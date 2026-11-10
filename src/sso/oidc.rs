use std::io::Read;

use anyhow::{Context, Result, ensure};
use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, IssuerUrl, Nonce, NonceVerifier, OAuth2TokenResponse,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse,
    core::{
        CoreAuthenticationFlow, CoreClient, CoreJwsSigningAlgorithm, CoreProviderMetadata,
        CoreTokenResponse, CoreTokenType,
    },
};
use reqwest::blocking::Client;
use url::Url;

use super::{CLIENT_ID, Endpoints, LIMIT, Session, now, secret, secure_url};

type Provider = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

pub(super) struct Oidc {
    provider: Provider,
    http: Client,
    issuer: Url,
}

impl Oidc {
    pub(super) fn discover(endpoints: &Endpoints) -> Result<Self> {
        let http = super::client()?;
        // Discovery includes signing keys, before an authorization or refresh grant is consumed.
        let metadata = CoreProviderMetadata::discover(
            &IssuerUrl::new(endpoints.issuer.to_string())?,
            &|request| send(&http, &endpoints.issuer, request),
        )
        .map_err(|_| anyhow::anyhow!("SSO provider discovery unavailable"))?;
        let authorization = metadata.authorization_endpoint().url();
        secure_url(authorization.as_str())?;
        ensure!(
            authorization.origin() == endpoints.issuer.origin(),
            "SSO authorization endpoint belongs to another host"
        );
        ensure!(
            metadata.token_endpoint().is_some(),
            "SSO provider has no token endpoint"
        );
        Ok(Self {
            provider: CoreClient::from_provider_metadata(
                metadata,
                ClientId::new(CLIENT_ID.into()),
                None,
            ),
            http,
            issuer: endpoints.issuer.clone(),
        })
    }

    pub(super) fn with_redirect(mut self, redirect: &Url) -> Result<Self> {
        self.provider = self
            .provider
            .set_redirect_uri(RedirectUrl::new(redirect.to_string())?);
        Ok(self)
    }

    pub(super) fn authorization(&self, link: bool) -> (Url, CsrfToken, Nonce, PkceCodeVerifier) {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut request = self
            .provider
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .add_scope(Scope::new("email".into()))
            .set_pkce_challenge(challenge);
        if link {
            request = request.add_extra_param("kc_action", "idp_link:cmu-dev");
        }
        let (url, state, nonce) = request.url();
        (url, state, nonce, verifier)
    }

    pub(super) fn redeem(
        &self,
        endpoints: &Endpoints,
        code: String,
        verifier: PkceCodeVerifier,
        nonce: &Nonce,
    ) -> Result<Session> {
        let requested_at = now()?;
        let response = self
            .provider
            .exchange_code(AuthorizationCode::new(code))?
            .set_pkce_verifier(verifier)
            .request(&|request| send(&self.http, &self.issuer, request))
            .map_err(|_| anyhow::anyhow!("SSO exchange could not be confirmed; sign in again"))?;
        self.session(endpoints, response, nonce, None, requested_at)
    }

    pub(super) fn refresh(&self, endpoints: &Endpoints, previous: &Session) -> Result<Session> {
        let requested_at = now()?;
        let response = self
            .provider
            .exchange_refresh_token(&RefreshToken::new(previous.refresh_token.clone()))?
            .request(&|request| send(&self.http, &self.issuer, request))
            .map_err(|_| anyhow::anyhow!("SSO refresh could not be confirmed; sign in again"))?;
        self.session(
            endpoints,
            response,
            &Nonce::new(previous.nonce.clone()),
            Some(previous),
            requested_at,
        )
    }

    fn session(
        &self,
        endpoints: &Endpoints,
        response: CoreTokenResponse,
        nonce: &Nonce,
        previous: Option<&Session>,
        requested_at: u64,
    ) -> Result<Session> {
        ensure!(
            response.token_type() == &CoreTokenType::Bearer,
            "unsupported SSO token type"
        );
        let token = response
            .id_token()
            .context("SSO response has no ID token")?;
        let verifier = self
            .provider
            .id_token_verifier()
            .set_allowed_algs([CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256]);
        let claims = token
            .claims(&verifier, |received: Option<&Nonce>| {
                if previous.is_some() && received.is_none() {
                    return Ok(());
                }
                nonce.verify(received)
            })
            .map_err(|_| anyhow::anyhow!("identity token verification failed"))?;
        ensure!(
            !claims.subject().as_str().is_empty()
                && claims
                    .authorized_party()
                    .is_some_and(|party| party.as_str() == CLIENT_ID),
            "identity token is not issued to Collar"
        );
        let auth_time = claims
            .auth_time()
            .map(|time| u64::try_from(time.timestamp()))
            .transpose()
            .context("invalid authentication time")?;
        if let Some(previous) = previous {
            ensure!(
                claims.subject().as_str() == previous.subject,
                "refreshed session belongs to another user"
            );
            ensure!(
                auth_time.is_none() || auth_time == previous.auth_time,
                "refresh changed the original authentication time"
            );
        }
        if let Some(expected) = claims.access_token_hash() {
            let actual = AccessTokenHash::from_token(
                response.access_token(),
                token.signing_alg()?,
                token.signing_key(&verifier)?,
            )?;
            ensure!(&actual == expected, "identity access token hash mismatch");
        }
        let access_token = response.access_token().secret();
        let refresh_token = response
            .refresh_token()
            .context("SSO response has no refresh token")?
            .secret();
        secret(access_token)?;
        secret(refresh_token)?;
        let expires_in = response
            .expires_in()
            .context("SSO response has no token expiry")?
            .as_secs();
        ensure!(expires_in > 0, "invalid identity token expiry");
        Ok(Session {
            issuer: endpoints.issuer.to_string(),
            client_id: CLIENT_ID.into(),
            broker: endpoints.broker.to_string(),
            grafana: endpoints.grafana.to_string(),
            subject: claims.subject().to_string(),
            nonce: nonce.secret().clone(),
            auth_time: previous.map_or(auth_time, |session| session.auth_time),
            access_token: access_token.clone(),
            refresh_token: refresh_token.clone(),
            expires_at: requested_at
                .checked_add(expires_in)
                .context("invalid identity token expiry")?,
        })
    }
}

// Reuse Collar's bounded blocking client through the library's supported transport API.
fn send(http: &Client, issuer: &Url, request: HttpRequest) -> Result<HttpResponse, std::io::Error> {
    let result = (|| -> Result<HttpResponse> {
        let url = secure_url(&request.uri().to_string())?;
        ensure!(
            url.origin() == issuer.origin(),
            "SSO endpoint belongs to another host"
        );
        let (parts, body) = request.into_parts();
        let response = http
            .request(parts.method, url)
            .headers(parts.headers)
            .body(body)
            .send()?;
        let mut builder = openidconnect::http::Response::builder().status(response.status());
        if let Some(headers) = builder.headers_mut() {
            *headers = response.headers().clone();
        }
        let mut bytes = Vec::new();
        response.take(LIMIT + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() as u64 <= LIMIT, "SSO response exceeds 8 MiB");
        Ok(builder.body(bytes)?)
    })();
    result.map_err(|_| std::io::Error::other("SSO HTTP request failed"))
}
