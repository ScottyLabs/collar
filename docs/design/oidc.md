# Collar SSO

- **Status:** Proposal

## Overview

Extend `collar auth login` to authenticate with Forgejo and Grafana. Use a public
Keycloak client for Grafana and retain `git-credential-oauth` for Forgejo. Store
separate credentials for each service.

This lets Collar read deployment logs through Grafana without adding a Kennel
API. Current builds run through nixbot; build-log access is separate work.

## Authentication flow

1. The user runs `collar auth login`.
1. Collar opens Keycloak with client ID `collar`, scopes `openid profile email`,
   and PKCE S256. The user signs in if needed.
1. Keycloak returns a code to `http://127.0.0.1:<port>/callback`. Collar validates
   state, exchanges the code, checks Grafana access with `GET /api/user`, and
   saves the access and refresh tokens.
1. If Forgejo login is needed, Collar starts its existing OAuth flow. Forgejo's
   linked Keycloak login should reuse the browser session, avoiding another
   credential entry. The user may still need to approve Collar's access in
   Forgejo on first use: Keycloak sign-in does not grant Forgejo OAuth consent.
   Verify session reuse after deployment.
1. Collar reports status for both services. A failure for one service does not
   discard the other's credentials.

One login command can involve two browser flows. Implementing Forgejo OAuth
directly in Collar instead of using `git-credential-oauth` would not remove the
consent step.

## Governance

Register Collar under DevOps and enable `oidc_client.public`. Extend
[`OidcClientFeature`](https://git.cmu.dev/ScottyLabs/governance/src/branch/main/crates/governance-schema/src/team.rs)
with optional `redirect_uris` and `audiences` lists. Keep existing defaults when
omitted; update the generator, schema, tests, and feature documentation.

For Collar, override the shared web callback with `http://127.0.0.1/callback`.
The CLI binds an available loopback port before opening the browser. Keycloak
[matches loopback callbacks independently of the port](https://github.com/keycloak/keycloak/blob/main/services/src/main/java/org/keycloak/protocol/oidc/utils/RedirectUtils.java).
Use client ID `collar` for the installed CLI; staging/dev clients are not needed
for normal use.

Set `audiences: [grafana]`. Generate an audience mapper on the client that adds
`grafana` to access tokens only. Retain the existing full-path `groups` mapper
and include `sub`, `email`, and `preferred_username`. Use normal Keycloak refresh
sessions, without requesting offline access or changing realm session limits.

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
explicit. Verify both methods resolve to the same account before rollout.
[Grafana JWT authentication](https://grafana.com/docs/grafana/latest/setup-grafana/configure-access/configure-authentication/jwt/)
validates tokens; Collar handles refresh and sends the raw access token in the
configured header. Keep browser OAuth, its client, and existing status links.
No Forgejo configuration change is required.

## Collar and Kennel

Implement the Keycloak flow with state validation, PKCE, and a five-minute
callback timeout. Store credentials in the OS credential store by service,
issuer, and client. Refresh before expiry; require explicit login when refresh
fails. Extend status/logout to both services, revoking the Keycloak refresh
session on logout. Preserve Forgejo token input and headless authentication.

For logs, read the current commit's `kennel/deploy` status. Its Grafana URL
already carries `project`, `branch`, and full `commit` filters. Validate these
against the selected revision and configured Grafana origin; never infer the
project from the repository name. Kennel needs no report changes.

Query `GET /api/datasources/proxy/uid/loki/loki/api/v1/query_range` on Grafana.
Use `{project="..."} | branch="..." | commit="..."`, escaping filter values.
`project` is a Loki label; `branch` and `commit` are structured metadata. Limit
queries to the existing 30-day retention, 1,000 lines, and a bounded response.
Preserve JSON report output and offer `--links` for browser access.

Access follows Grafana's existing permissions, including access across projects.
Project-level isolation would require server-side authorization.

## Implementation

1. Merge governance support and provision the client, then deploy Grafana JWT
   authentication.
1. Implement Collar authentication and deployment-log reads with fixtures.
1. Verify SSO reuse, account matching, role mapping, refresh/logout, and rejection
   of wrong-issuer, wrong-audience, wrong-client, and expired tokens. Confirm a
   bounded deployment-log read without changing live resources.
