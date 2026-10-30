# Collar SSO

- **Status:** Proposal

## Overview

Extend `collar auth login` to authenticate with Forgejo and Grafana. Use a public
Keycloak client for Grafana. Forgejo can use direct OAuth or Keycloak broker
retrieval as described below. Store separate credentials for each service.

Read deployment logs through Grafana; no Kennel API is needed. Builds run
through nixbot and are outside this proposal.

## Authentication flow

1. The user runs `collar auth login`.
1. Collar opens Keycloak with client ID `collar`, scopes `openid profile email`,
   and PKCE S256. The user signs in if needed.
1. Keycloak returns a code to `http://127.0.0.1:<port>/callback`. Collar validates
   state, exchanges the code, checks Grafana access with `GET /api/user`, and
   saves the access and refresh tokens.
1. Obtain the Forgejo credential using either option below.
1. Report status for both services. A failure for one does not discard the
   other's credentials.

## Forgejo authentication options

### Direct OAuth

Retain `git-credential-oauth`. Forgejo requires approval for every fresh
public-client authorization, including on a new machine. Saved refresh tokens
avoid repeating it on the same machine. This needs no additional service.

### Keycloak broker

Infrastructure already declares a confidential Forgejo connector, `cmu-dev`,
with stored tokens. Link and approve Forgejo once through Keycloak; later Collar
logins can retrieve its token across machines. Revoked grants require relinking.

Infrastructure pins Keycloak 26.8.1; verify the running version.
Its [V1 API](https://github.com/keycloak/keycloak/blob/26.8.1/services/src/main/java/org/keycloak/services/resources/IdentityBrokerService.java)
accepts public clients with `broker/read-token`, but that role also exposes the
user's other linked providers' stored tokens. Do not grant it to Collar.
[V1 is deprecated; V2 requires confidential clients](https://www.keycloak.org/docs/latest/server_development/#retrieving-external-idp-tokens).

For V2, infrastructure must:

- Enable `identity-brokering-api:v2` and provision a confidential `collar-broker`
  client. Enable external-token retrieval with an allowlist containing only
  `cmu-dev`; keep its secret server-side.
- Provide an authenticated token relay. Add `collar-broker` to Collar's token
  audience. The relay submits that user token as the `token` form field to
  `POST /realms/scottylabs/broker/cmu-dev/token`, authenticating as `collar-broker`.
  Return only the Forgejo access token and expiry to Collar.
- Keep refresh tokens in Keycloak. Fix or upgrade V2 stored-token refresh before
  rollout: [26.8.1's V2 path](https://github.com/keycloak/keycloak/blob/26.8.1/services/src/main/java/org/keycloak/broker/oidc/AbstractOAuth2IdentityProvider.java)
  returns the stored access token without V1's refresh handling. Test expired
  access tokens and rotated refresh tokens. Missing/revoked grants require
  explicit relinking.

The relay must validate the user token, accept only client `collar`, and expose
only `cmu-dev`. This option avoids per-machine consent but adds a server component
and requires working V2 refresh. Neither option is selected yet.

## Governance

Register Collar under DevOps and enable `oidc_client.public`. Extend
[`OidcClientFeature`](https://git.cmu.dev/ScottyLabs/governance/src/branch/main/crates/governance-schema/src/team.rs)
with optional `redirect_uris` and `audiences` lists, preserving defaults. Update
the generator, schema, tests, and documentation.

For Collar, override the shared web callback with `http://127.0.0.1/callback`.
The CLI binds an available loopback port before opening the browser. Keycloak
[matches loopback callbacks independently of the port](https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/utils/RedirectUtils.java).
Use client ID `collar` for the installed CLI.

Add `grafana` to access-token audiences; include `collar-broker` for the broker
option. Retain full-path `groups`, `sub`, `email`, and `preferred_username`. Use
normal refresh sessions without offline access or realm timeout changes.

## Infrastructure

In [`observability/grafana.nix`](https://git.cmu.dev/ScottyLabs/infrastructure/src/branch/main/modules/hosts/scottylabs/infra-01/observability/grafana.nix),
enable JWT authentication with:

| Setting | Value |
| --- | --- |
| `header_name` | `X-JWT-Assertion` |
| `jwk_set_url` | `https://idp.scottylabs.org/realms/scottylabs/protocol/openid-connect/certs` |
| `expect_claims` | `{"iss":"https://idp.scottylabs.org/realms/scottylabs","aud":"grafana","azp":"collar"}` |
| `username_claim`, `email_claim` | `email` |
| `auto_sign_up`, `role_attribute_strict`, `allow_assign_grafana_admin` | `true` |
| `url_login` | `false` |

Copy the existing role expression: `/projects/devops/admins` maps to
`GrafanaAdmin`; everyone else gets `Viewer`. Set browser OAuth's
`login_attribute_path` to `email` as well, making the existing email fallback
explicit. Verify both methods use the same account.
[Grafana JWT authentication](https://grafana.com/docs/grafana/latest/setup-grafana/configure-access/configure-authentication/jwt/)
validates tokens; Collar handles refresh and sends the raw access token in the
configured header. Keep browser OAuth, its client, and existing status links.

## Collar and Kennel

Use a five-minute callback timeout. Store credentials in the OS credential store
by service, issuer, and client. Extend status/logout to both services; revoke
Collar's Keycloak session on logout, leaving shared account links intact. Preserve
Forgejo token input and headless authentication. Refresh failures require login.

Read `project`, `branch`, and full `commit` from the current `kennel/deploy`
status URL. Validate the revision and Grafana origin; never infer the project
from the repository name. Kennel needs no report changes.

Query `GET /api/datasources/proxy/uid/loki/loki/api/v1/query_range` on Grafana.
Use `{project="..."} | branch="..." | commit="..."`, escaping filter values.
`project` is a Loki label; `branch` and `commit` are structured metadata. Limit
queries to the existing 30-day retention, 1,000 lines, and a bounded response.
Preserve JSON report output and offer `--links` for browser access.

Access follows Grafana's existing permissions, including access across projects.
Project-level isolation would require server-side authorization.

## Implementation

1. Select the Forgejo option. Provision the Keycloak client and Grafana JWT
   settings; deploy the relay and V2 permissions only for the broker option.
1. Implement Collar authentication and deployment-log reads with fixtures.
1. Verify SSO reuse, account matching, role mapping, refresh/logout, and rejection
   of invalid tokens. For the broker option, also test cross-user denial,
   provider restrictions, refresh rotation, and missing/revoked links. Verify a
   bounded deployment-log read.
