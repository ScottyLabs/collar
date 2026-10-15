# RFC: Collar Developer CLI

- **Status:** Draft
- **Author(s):** @kritdass
- **Created:** 2026-10-14
- **Updated:** 2026-10-15

## Overview

Build Collar, an independent Rust CLI for git.cmu.dev with GitHub CLI command conventions and ScottyLabs governance, Kennel, and infrastructure workflows. Develop a bounded proof of concept on this branch before proposing production adoption.

## Motivation

Contributors currently move between Forgejo, governance TOML, Kennel status comments, and infrastructure configuration to understand a project. A common CLI can expose these relationships and provide familiar pull request review commands. Implement the client directly against Forgejo rather than depending on fj.

The implementation follows the [link-shortener reference project](https://git.cmu.dev/ScottyLabs/link-shortener): a standalone Rust package, a small entry point, focused modules, contextual errors, behavior tests, shared Kennel build helpers, and devenv scripts. Application OIDC and database setup are not requirements for this local CLI. Its user token is separate from deployed service credentials.

## Goals

- Use gh command names, arguments, review flags, and selected JSON fields for supported forge workflows.
- Review PRs with approval, requested changes, body text, and inline comments pinned to the inspected head commit.
- Discover project ownership and enabled features from latest canonical governance snapshots.
- Discover deployment statuses and preview links from existing Forgejo data.
- Inspect infrastructure configuration and delegate explicit operations to its existing tools.
- Fail explicitly on unavailable capabilities, invalid configuration, and failed requests.
- Keep the crate independent of Terrier server and domain types.

## Non-Goals

The POC does not implement every gh command or add Kennel server endpoints. Provisioning stays with governance and infrastructure tools. Repository provisioning, membership, and feature changes remain in the canonical governance workflow. GitHub-specific services remain outside this POC.

## Detailed Design

### Command contract

The POC implements these workflows:

| Group | Commands | Behavior |
| --- | --- | --- |
| Onboarding | doctor, project view/select | Aggregate local/login diagnostics and join canonical ownership, features, and secret namespace; explicit offline mode and optional terminal selection. |
| Configuration | config get/set/unset | Nonsecret host defaults; canonical sources resolve the latest revision automatically. |
| Forge | auth status, repo view/fork, pr create/checkout/edit/comment/close/reopen/merge/update-branch/list/view/browse/diff/checks/review, api | Direct Forgejo requests; explicit writes require a token. Terminal inspection stays read-only. |
| Governance | project list/view, service-access | Inspect canonical ownership and declared service access without a clone. |
| Kennel | status, preview, logs, wait | Inspect commit-bound checks and preview/log links; wait with bounded polling. |
| Infrastructure | hosts, service, check, build, atlantis-config | Discover canonical configuration and delegate explicit Nix builds. |
| Environment | auth bao login/status, secrets check/set, dev shell/run/up/down/task, check | Delegate to Bao, secretspec, devenv, and prek with native tool validation and original exit codes. |
| Changelog | view, updates, update | Read latest canonical generated summaries and announcements without a clone. |
| RFC | rfc | Complete Fetch search and draft creation within Collar. |
| Documentation | docs list/view/search | Read and search latest canonical organization and service guides, with explicit offline snapshots. |

Repository selection accepts -R [HOST/]OWNER/REPO and otherwise reads upstream then origin. Explicit host/repository disagreements are errors. PR selection accepts a positive number or a same-host PR URL; omitted selection resolves the current branch through the pull list and refuses ambiguity. JSON output projects explicit supported gh-style fields. Forgejo-native responses remain accessible through api.

Review submission supports --approve, --request-changes, --comment, --body, and --body-file, including stdin. Exactly one review verdict is required. Repeatable --inline-comment flags carry path, line, side, and body directly. Validate line anchors against the inspected diff, then submit one review with that head SHA as commit_id. Do not retry mutations or silently retarget a changed head. The POC never submits reviews as part of its own live verification.

PR creation uses an already-pushed head branch and the repository's default target branch unless explicitly selected. Authoring commands accept literal text or Markdown files/stdin; edits send only supplied fields and support explicitly clearing the body. Comments verify that the numeric target is a PR before using Forgejo's shared issue-comment endpoint. Close/reopen refuse merged PRs and already-requested states. These metadata/state endpoints do not provide atomic expected-head guards; server permissions and current state remain authoritative. Remote mutations are verified only against fixtures during development.

### API and authentication

Store only the host and nonsecret saved-login origin metadata in the platform user configuration directory. Credentials belong in the OS credential store, one entry per canonical origin. Verify before saving, never fall back to plaintext, and restore the previous entry if metadata persistence fails. Canonical source revisions are resolved automatically on every online read; local checkout overrides are explicit. Support explicit configuration files and a no-config mode for automation. Reject malformed or unknown settings and write atomically. Keep inferred Git remote hosts independent from the saved host default.

Default to git.cmu.dev. Accept COLLAR_TOKEN from the environment for authenticated operations, bound to COLLAR_HOST, the saved default host, or git.cmu.dev in that order; it overrides only that host's saved credential. Public reads without registered login do not require a credential store. An explicit alternate --host does not receive the default host's credential. No token is accepted on a command line or written into Collar configuration. Login opens the existing Forgejo browser authorization flow, including university SSO, and stores refresh credentials securely. Hidden token input and explicit bounded stdin remain available. Reject non-HTTPS hosts except loopback test servers, URL credentials, and redirects. Bound connect/request timeouts and pagination. Error messages include the operation and HTTP status without response bodies that may echo credentials. Typed response decoding must fail on malformed data.

Raw api requests are relative to the selected host's /api/v1 namespace. Match gh's string -f/--raw-field and typed -F/--field conventions, infer POST when fields are supplied, and permit an explicit method. Do not accept arbitrary absolute URLs that could receive the selected host's credentials.

### ScottyLabs data

Project context validates the selected repository against the governance organization and forge origin before matching ownership. Expose the repository-specific features, inherited ownership, namespace, and source paths together. Local diagnostics aggregate actionable failures without asserting that declarations establish live access or that account linking has been verified. Offline diagnostics must make no network requests or resolve secrets.

Governance inspection defaults to versioned canonical API snapshots and traverses team and nested project entries. Explicit checkout overrides support local edits. It reports declared ownership rather than asserting that remote access has been applied. It does not replace governance-core validation or Terraform generation. Parse the subset needed for discovery while preserving feature names and settings as data; reject malformed required identity fields and ambiguous repository ownership.

Infrastructure discovery reads modules/hosts and resolves service filenames beneath those host directories, from a canonical snapshot or explicit checkout. Results are configuration locations, not live service health. Do not parse Nix with regular expressions or claim filesystem discovery is evaluated flake output.

Kennel status combines commit statuses and Kennel-marked PR comments. Include the source commit and links; historical comments do not prove current service health. There is no deployment discovery API today. Do not read remote SQLite or require SSH administrator access.

### Modules

Use focused modules for argument parsing and orchestration, repository selection, the HTTP client, PR operations, governance discovery, and infrastructure discovery. Add shared abstractions only when multiple implemented commands need them. Use clap, reqwest, serde, serde_json, and TOML parsing with the workspace's conventions.

## Alternatives Considered

- An fj wrapper reduces implementation but does not provide control over gh compatibility and its release behavior.
- A gh fork carries GitHub REST/GraphQL assumptions and a Go implementation.
- Direct infrastructure and identity mutations create competing sources of truth. Retain governance PRs, Atlantis, Colmena, Bao, and secretspec as the owners of those operations.

## Compatibility and limits

This remains a Draft POC. The supported commands are documented in the [command reference](../reference/commands/index.md) and task guides. Forgejo fixtures cover the selected request/response contracts; compatibility with every gh command, flag, and exit code is not claimed.

Typed forge workflows include PR review and authoring, repository inspection, issues and labels, release metadata, workflow dispatch, and run/job inspection. Raw API access covers additional native endpoints. Release asset transfer, GitHub-only services, extension management, and new Kennel backend APIs are outside the agreed POC scope.

Read-only ScottyLabs discovery uses latest canonical snapshots without user-maintained clones. Explicit local checkouts are required for edits and tools that need full source trees. Governance provisioning, Atlantis, Colmena, Bao, and secretspec retain ownership of their workflows.

The minimum supported Forgejo version remains an adoption decision. The standalone package has no Terrier domain dependencies and keeps its design and user documentation in this repository.

### Expanded developer workflow

Fold Fetch's complete search and creation implementation into `collar rfc`, including its regression tests. Remove the separate crate and development-shell command. Resolve RFC directories from an explicit `--directory`, the active devenv root, or the current Git checkout rather than a compiled-in Terrier source path. Crate-local `RFC.md` files are searchable when their directory is selected.

Read-only governance and infrastructure discovery should not require user-maintained clones. Prefer versioned canonical API snapshots cached under Collar's user cache directory, with explicit refresh/offline behavior and source revision in output. Keep optional checkout overrides for local inspection, validation, and Nix evaluation. Never silently substitute cached data for a failed refresh. Changes still use canonical review pipelines.

Terminal interaction should help selection and inspection: an opt-in project picker and PR review navigator, with keyboard help and explicit write confirmation. Preserve noninteractive commands and JSON output; reject TUI mode without a terminal. Feature access must cover the current governance feature schema and link each feature to its supported inspection, configuration, and operational workflow.

### Environment tools

Expose Bao login/status, secretspec check/set, devenv shell/run/up/down/tasks, and repository hooks as typed commands. Let devenv and secretspec validate their own configuration; development commands must not require governance or secretspec preflight. Delegate validation and provisioning to the existing tools, preserve the user's access reason, inherit interactive I/O only for intentional interactive operations, and propagate tool exit codes. Secret checks use no-prompt/value-free reporting; secret updates accept a prompt or file/stdin input, never a secret value argument. Bootstrap Bao through Nix with `login -method=oidc -no-print` so a new machine can authenticate before entering the development shell without displaying its token. Stopping local processes must remain possible without governance or secret preflight.
