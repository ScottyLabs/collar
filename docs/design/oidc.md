# Collar SSO

Status: Implemented locally; platform rollout pending

## Authentication

Use Keycloak for `collar auth login`, Grafana access, and Forgejo token retrieval through a server-side broker. This follows [the broker decision](https://git.cmu.dev/ScottyLabs/collar/pulls/9#issuecomment-21419). Machines retain a Keycloak session and short-lived Forgejo access tokens; Forgejo refresh tokens stay in Keycloak. Do not add a direct Forgejo OAuth fallback.

Use the `openidconnect` protocol library, as the ScottyLabs Rust web applications do through `axum-oidc`. Discover the provider and signing keys before consuming a code or refresh grant. Keep the native callback and OS credential storage in Collar.

1. Bind an available loopback port and open Keycloak with public client `collar`, scopes `openid profile email`, and PKCE S256.
1. Validate callback state, exchange the authorization code, and save the Keycloak session in the OS credential store. Use a five-minute callback timeout.
1. Request Forgejo access through the broker. If `cmu-dev` is missing or revoked, open Keycloak's account-linking flow during login and require successful linking before reporting Forgejo access.
1. Verify Grafana access with `GET /api/user` and report access to each service.

Use normal refresh sessions. The realm's configured limits are 24 hours idle and seven days maximum; retain those limits and omit `offline_access`. Refresh failures require login. Credential entries remain bound to the service, issuer, and client. Logout revokes Collar's Keycloak session and clears its local credentials while preserving shared account links. Preserve explicit host-bound token input for headless Forgejo access.

## Broker

Infrastructure already declares the Forgejo connector `cmu-dev`. Enable `identity-brokering-api:v2` and provision confidential client `collar-broker` with external-token retrieval allowed only for `cmu-dev`. Keep its client secret server-side. Do not grant the broad V1 `broker/read-token` role to Collar.

The relay validates the user token's signature, issuer, expiry, audience `collar-broker`, and authorized party `collar`. Derive the user from its subject; accept neither caller-supplied user IDs nor provider selection. Submit the user token to `POST /realms/scottylabs/broker/cmu-dev/token` as the `token` form field, authenticating as `collar-broker`. Return only the Forgejo access token and expiry.

V2 stored-token refresh must work before rollout. The pinned Keycloak 26.8.2 OIDC provider already refreshes expired tokens, but its [refresh handler](https://github.com/keycloak/keycloak/blob/26.8.2/services/src/main/java/org/keycloak/broker/oidc/OIDCIdentityProvider.java#L222-L356) can discard refresh state on a transient upstream error. The [upstream fix](https://github.com/keycloak/keycloak/pull/53256) remains a rollout prerequisite. Nix currently packages a compiled release ZIP; adding a Java-source patch to that package does not modify its classes. Use a verified source-built backport or a release containing the fix before enabling the broker. No placeholder patch or fake dependency hash is included.

Verify expired access tokens, rotated refresh tokens, revoked grants, wrong-client/provider requests, and cross-user denial. A transient Forgejo outage must preserve the stored refresh token and return an unavailable result rather than force account relinking.

## Governance

Update Collar's existing governance project registration as public OIDC client `collar`, preserving its current ownership. Extend [`OidcClientFeature`](https://git.cmu.dev/ScottyLabs/governance/src/branch/main/crates/governance-schema/src/team.rs) with optional `redirect_uris` and `audiences`, preserving defaults for existing clients. Update the generator, schema, documentation, and tests.

Set the callback to `http://127.0.0.1/callback`; Keycloak matches loopback callbacks independently of the bound port. Include audiences `grafana` and `collar-broker`, retaining full-path `groups`, `sub`, `email`, and `preferred_username`.

## Grafana

Enable JWT authentication in [`observability/grafana.nix`](https://git.cmu.dev/ScottyLabs/infrastructure/src/branch/main/modules/hosts/scottylabs/infra-01/observability/grafana.nix):

| Setting | Value |
| -- | -- |
| `header_name` | `X-JWT-Assertion` |
| `jwk_set_url` | `https://idp.scottylabs.org/realms/scottylabs/protocol/openid-connect/certs` |
| `expect_claims` | `{"iss":"https://idp.scottylabs.org/realms/scottylabs","aud":"grafana","azp":"collar"}` |
| `username_claim`, `email_claim` | `email` |
| `auto_sign_up`, `role_attribute_strict`, `allow_assign_grafana_admin` | `true` |
| `url_login` | `false` |

Retain the role mapping: `/projects/devops/admins` maps to `GrafanaAdmin`; everyone else gets `Viewer`. Make browser OAuth's `login_attribute_path` explicitly `email` and verify both methods resolve to the same account. Keep browser OAuth and its client.

Collar refreshes its Keycloak token and sends the raw access token in the configured JWT header. Grafana validates it. Access follows Grafana's existing permissions, including access across projects; this proposal does not establish project-level isolation.

## Deployment logs

Reuse Collar's typed Kennel branch-state client for `GET /projects/{project}/branches/{branch}`. Take the canonical project and branch from the response and select the full commit from the matching request or deployment. Check that it matches the requested PR head and recheck the head before reporting results. Missing state or a revision mismatch is an error; do not parse `kennel/deploy` status URLs or guess identifiers as a fallback.

Read deployment log lines through Grafana's `GET /api/datasources/proxy/uid/loki/loki/api/v1/query_range`, using `{project="..."} | branch="..." | commit="..."` with escaped filter values. `project` is a Loki label; `branch` and `commit` are structured metadata. Bound the time range to retention, limit results to 1,000 lines, and cap response size. Keep the shipped JSON report contract and browser links. Builds remain nixbot's responsibility.

Kennel already supplies deployment identity. No new Kennel log endpoint or authentication layer is needed. The earlier [Kennel log proposal](https://git.cmu.dev/ScottyLabs/collar/src/branch/feat/kennel-log-access/docs/design/kennel-log-access.md) is superseded by this approach.

## Delivery

1. Provision the governance client, broker, Keycloak refresh fix, and Grafana JWT configuration through their owning repositories.
1. Implement Collar login, refresh, logout, and bounded deployment-log reads with fixtures.
1. Verify SSO reuse, linking, account matching, role mapping, invalid-token rejection, session expiry/revocation, and broker refresh and isolation. Verify branch-state mismatches and PR head changes without live writes.

The local implementation is on the existing `feat/oidc-auth` PR branch. Anish owns the governance and infrastructure prerequisites; this change modifies only Collar. None of these changes are deployed by editing the repositories. The broker stays disabled until the Keycloak dependency above is verified, the client and audiences are provisioned, and the infrastructure Collar input is updated to a version containing `collar-broker`.

## Walkthrough

After those prerequisites are deployed, run `collar auth login --web`, complete university sign-in and any account-linking action, then run `collar auth status`. Select a PR with a matching Kennel request and run `collar kennel logs NUMBER --lines --since 60 --tail 20`. An expired access token should refresh without a browser; logout should revoke the Keycloak session and clear local credentials. Test refresh rotation, provider outages, and cross-user denial with fixtures rather than live writes.

The repository tests also execute the loopback callback against a local signed-token fixture, verifying PKCE, state, nonce, and noise-request handling. That fixture is not a live university sign-in.
