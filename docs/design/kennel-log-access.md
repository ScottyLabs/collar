# Authenticated Kennel log access

Status: Proposal

Collar should retrieve Kennel logs without asking developers to create Grafana
service-account tokens, learn Loki queries, or maintain a checkout. Kennel already
knows the project, build ID, branch, and commit, and can query Loki over its
internal network. The current Grafana links remain useful browser entry points.

## User experience

```sh
collar kennel logs 42
collar kennel logs 42 --build
collar kennel logs 42 --follow
collar kennel logs 42 --links
```

The default reads deployment log lines for the selected PR head. `--build` reads
build logs; `--follow` streams new lines; `--links` preserves the browser workflow.
The existing `--json` historical-report contract must remain available. Decide on
a separate structured log mode before implementation rather than changing that
contract silently.

Collar uses the PR head and Kennel's reported identifiers to resolve a build or
deployment. It must not guess a project from a repository name. Fail clearly if
the requested revision has no report or if the PR head changes during resolution.

## Kennel API needed

Add authenticated, read-only log retrieval for builds and deployments. Suggested
routes are `GET /builds/{id}/logs` and `GET /deployments/{id}/logs`. A lookup by
canonical repository and full commit SHA is also needed when Collar only has
Forgejo commit statuses. That lookup can instead be avoided if Kennel publishes
stable build/deployment IDs in machine-readable status metadata.

Responses should identify the project, build/deployment, and full commit SHA.
Support bounded time ranges, line limits, and a continuation cursor. Return
structured timestamps, labels/metadata, and log lines; cap response size. Optional
following needs a documented streaming protocol, disconnect handling, and an
idle limit. Retention expiry and missing resources should have distinguishable
responses. A log read must never trigger a build, deployment, or restart.

Kennel constructs the Loki query from its own stored identity and metadata. Do
not expose arbitrary LogQL or permit callers to choose another project's label
filters. Kennel's current labels are `project`, with per-line `build_id`, `branch`,
and `commit` metadata. Credentials for internal Loki access stay server-side.

## Authentication dependency

The current Kennel HTTP routes cover webhooks, metrics, and deployment health;
there is no user-authenticated API to reuse. Implement a supported university/
ScottyLabs SSO flow for CLI callers, then verify issuer, audience, expiry, and
project access at Kennel. Forgejo's access token is not automatically a Kennel
credential. Reuse an existing identity integration where possible; do not accept
a token for the wrong service or rely on repository visibility to authorize logs.

The CLI flow must have a headless path for authorized automation and an ergonomic
interactive login for developers. The precise identity/token exchange and
project-access mapping require agreement with the infrastructure owners. A shared
Grafana token distributed to developers is not the intended solution.

## Delivery

Implement authentication and authorization in Kennel/infrastructure first,
including cross-project denial tests and bounded read fixtures. Agree on the API
contract and identifier discovery. Then implement Collar's client with fixture
responses, default log reads, and preserved tool/CLI exit behavior. Keep Grafana
links available during migration. Never use live writes to validate this work.

This proposal does not enable token-based logcli access or change shipped log
semantics. The implementation waits for the authenticated backend contract.
