# Collar development review

Status: In progress. The existing proof of concept does not satisfy the complete developer workflow below.

## Required workflows

| Workflow | Required behavior | Verification |
| --- | --- | --- |
| Onboarding | Explain prerequisites, configure canonical checkouts and host, authenticate, diagnose Git/SSH, Keycloak account linking, Bao login, and devenv readiness. | Isolated configuration and subprocess tests; public host probes; headless failure paths. |
| Daily forge work | Clone/view repositories; create, inspect, check out, edit, comment on, review, update, merge, and close PRs; work with issues, labels, and releases. | Typed Forgejo requests, local Git fixtures, pinned mutations, ambiguity and permission failures. |
| Authentication | Host-bound credentials, saved login/logout/status, explicit headless behavior, no token arguments or plaintext credential fallback. | Isolated credential-store tests, environment precedence, multi-host isolation, redacted errors. |
| Scripting | Stable JSON fields and exit codes, complete pagination, completions, relative raw API access with typed fields. | Empty and nonempty schemas, page boundaries, HTTP failures, parser tests. |
| Governance | Discover teams/projects/features/namespaces; invoke canonical validation; prepare reviewed self-membership and feature changes; distinguish declarations from applied access. | Current canonical schemas, nested project fixtures, TOML preservation, local branch/PR tests. |
| Kennel | Find previews and build/runtime log links, report deployment state for the current revision, wait with a timeout, check health when an authorized endpoint is configured, explain missing reports. | Canonical statuses/comment contract, historical and teardown fixtures, bounded polling, no invented endpoints. |
| Environment | Guide Bao login; invoke secretspec checks and secret management with explicit profiles; use devenv for project tasks. | Exact subprocess arguments, exit propagation, no captured or printed secret values. |
| Infrastructure | Discover and evaluate hosts/services, locate dashboards/docs, check/build Nix configurations, prepare Atlantis plans and invoke Colmena through established workflows. | Canonical flake/Atlantis contracts, Nix output fixtures, platform/bastion handling, failed tool execution. |
| Project context | Relate the current repository to its governance owner, secret namespace, features, preview, and infrastructure/configuration links without repeated flags. | Mixed repository/project slugs, ambiguous ownership, stale or unavailable checkout signals. |
| Documentation and packaging | All usage/design/review documentation stays in this crate; supported commands are discoverable; Nix/devenv entrypoints work. | Link/format checks, help snapshots where useful, package build and runtime smoke checks. |

The existing Fetch executable remains supported. Integrating its RFC commands is a follow-up to the core developer workflows rather than a reason to duplicate its implementation.

## Integration contracts

The canonical source is git.cmu.dev. Read-only GitHub mirrors can lag behind. This review checked the Forgejo 16.0.5 schema and canonical source heads on 2026-10-14:

- Governance: `7a38b2aac4211f8429ddd2108bf5216873e87fd1`.
- Kennel: `d7545b882543b05a85c4bdd7a05b60364bda9718`.
- Infrastructure: `31434c4e673f5c56f68ec2fea7c87be306d647a5`.

Governance owns identity, team membership, repositories, feature provisioning, and project secret namespaces. Changes must enter its review and Atlantis pipeline. Its schema/core validator remains authoritative.

Kennel currently exposes `/webhook`, `/metrics`, and `/deployments/{id}/health`. Deployment listing, build/log retrieval, redeploy, and rollback have no public user API. Existing commit statuses and marked comments can expose reports and links; they do not prove live health. Operational APIs need a separately reviewed authentication and authorization contract before adding direct mutations.

Infrastructure owns evaluated NixOS/nix-darwin configuration, Colmena host activation, Atlantis/OpenTofu resources, observability, bootstrap credentials, and Bao access. Collar should invoke these owners with exact arguments and propagate failures rather than becoming another provisioner.

## Correctness findings

- Forgejo commit status responses use status, not state. Fixtures must follow the deployed schema rather than repeat the client's model.
- Diff anchors must honor hunk counts, deleted files, quoted Git paths, and integer bounds. File-looking added content must remain content.
- A review must recheck the PR's open state as well as its head before submission.
- Branch lookup must establish complete results before asserting uniqueness.
- Git discovery must distinguish a missing remote from a failed Git command.
- JSON decoding errors must not echo server-provided credential values.
- The documented no-retry mutation contract must explicitly disable the HTTP client's retry policy.
- JSON projection must reject unknown fields even when a list is empty.
- Reported commit statuses alone cannot prove required checks or merge readiness.

## Delivery order

1. Correctness and request/output boundaries with regression tests.
1. Configuration, diagnostics, project context, and existing-tool integration.
1. Complete PR/issue/repository workflows, then releases and Actions.
1. Saved authentication and headless behavior.
1. Governance change preparation, Kennel operations through supported contracts, and infrastructure workflows.
1. Full workflow audit against this table, contribution hooks, packaging, and public read-only smoke checks. Remote mutations are verified with local fixtures unless the user requests a real operation.

The goal remains open until the requirements have implementation and verification evidence. Command names, passing unrelated tests, and raw API escape hatches do not establish complete workflow support.

## Verification evidence

The correctness foundation is covered by [diff parser tests](src/diff.rs), [PR review validation](src/pr.rs), and [CLI integration tests](tests/cli.rs). These exercise canonical status response fields, short-page pagination, empty-list field validation, malformed and quoted diffs, deleted files, changing/closed PRs, host credential isolation, redacted JSON failures, and broken Git remotes. A public Terrier commit-status response independently confirmed the deployed field names.

The workflow requirements above still need implementation and end-to-end verification. Public read checks do not establish authenticated write permissions or successful provisioning.

Nonsecret configuration now supports saved host and canonical checkout defaults, explicit file selection, environment/flag precedence, atomic writes, and reproducible no-config operation. Tests cover missing/corrupt files, redacted TOML errors, checkout discovery, relative paths, and credential isolation when overriding the saved host. Shell evaluation, live access checks, and the remaining workflows above are still required.

`project view` now joins the selected repository to its canonical governance owner, inherited membership, repository-specific features, and secret namespace. It validates both origin and organization before matching, rejects duplicate ownership, and provides source paths and the configured Keycloak account-linking URL. Tests cover differing repository/project slugs, case-insensitive forge names, unrelated forges/owners, missing declarations, and duplicate ownership. Live access and checkout freshness remain separate diagnostic requirements.

`doctor` now aggregates runnable tools, Git identity/SSH-agent state, devenv files, canonical project ownership, secretspec namespace/profile alignment, and clean checkout revisions against last-fetched upstreams. Online checks verify Forgejo identity and Bao token acceptance without resolving secrets or printing subprocess output; offline mode skips those requests. Account linking stays manual. Isolated checkout/tool fixtures cover successful offline inspection, online request contracts, unavailable/failing tools, dirty checkouts, namespace/profile errors, redacted TOML diagnostics, and subprocess timeouts. SSH authentication, live project permissions, shell evaluation, and infrastructure health still need their respective workflow evidence.

### Verified boundary

At implementation commit `c109cae`, the following checks passed on x86_64 Linux:

| Check | Evidence and scope |
| --- | --- |
| Focused Rust checks | 13 unit tests and 28 CLI integration tests; strict Clippy with `--all-targets -- -D warnings`. |
| Contribution hooks | All applicable generated commit hooks passed, including workspace Rust tests/lints, dependency usage, secret scanning, and formatting. |
| Crate documentation | Relative Markdown links resolve; formatting passed. |
| Nix package | `nix build .#collar --no-link --print-out-paths --accept-flake-config` completed successfully. |
| Packaged executable | Version, root/config/project/doctor help, and canonical governance project-context smoke checks passed. |
| Canonical context | `ScottyLabs/link-shortener` resolved to team `devops`, namespace `link-shortener`, with repository-specific features and declared-only output. |

These checks establish the current implementation and packaging boundary. They do not establish complete gh parity, saved login, project secret permissions, successful governance application, or deployment operations. The required-workflow table remains the completion gate.

### Existing-tool integration constraints

- New-machine Bao login must work before entering a project shell. Use the standalone Kennel flake login documented in the [local development guide](https://docs.kennel.scottylabs.org/guides/secrets.html#local-development).
- `secretspec check` can prompt to set missing secrets. An inspection command must use `--no-prompt`, or the value-free `--json`/`--explain` modes, and propagate missing-secret failures. Doctor currently never invokes resolution.
- `secretspec set NAME` prompts for the value when no value argument is supplied. A Collar integration must keep secret values out of command arguments, logs, and saved configuration.
- Devenv's `--profile` selects a Nix development profile. `--secretspec-profile` selects a secret profile. A wrapper must use the latter explicitly and preserve the user's access reason rather than inventing one.

The command contracts above were checked against the installed CLI help alongside Kennel's canonical documentation. They are requirements for the remaining environment commands, not claims that those commands are implemented.
