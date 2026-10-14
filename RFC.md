# RFC: Collar Developer CLI

- **Status:** Draft
- **Author(s):** Pending
- **Created:** 2026-10-14
- **Updated:** 2026-10-14

## Overview

Build Collar, an independent Rust CLI for git.cmu.dev with GitHub CLI command conventions and ScottyLabs governance, Kennel, and infrastructure workflows. Develop a bounded proof of concept on this branch before proposing production adoption.

## Motivation

Contributors currently move between Forgejo, governance TOML, Kennel status comments, and infrastructure configuration to understand a project. A common CLI can expose these relationships and provide familiar pull request review commands. Implement the client directly against Forgejo rather than depending on fj.

The implementation follows the [link-shortener reference project](https://git.cmu.dev/ScottyLabs/link-shortener): workspace crates, a small entry point, focused modules, contextual errors, behavior tests, shared Kennel build helpers, and devenv scripts. Application OIDC and database setup are not requirements for this local CLI. Its user token is separate from deployed service credentials.

## Goals

- Use gh command names, arguments, review flags, and selected JSON fields for supported forge workflows.
- Review PRs with approval, requested changes, body text, and inline comments pinned to the inspected head commit.
- Discover project ownership and enabled features from a governance checkout.
- Discover deployment statuses and preview links from existing Forgejo data.
- Inspect infrastructure hosts and configuration locations without deploying changes.
- Fail explicitly on unavailable capabilities, invalid configuration, and failed requests.
- Keep the crate independent of Terrier server and domain types.

## Non-Goals

The POC does not implement every gh command, persist credentials, provision resources, apply infrastructure, mutate governance, or add Kennel server endpoints. These are later phases, not successful placeholder commands. GitHub-specific services require separate product and API decisions.

## Detailed Design

### Command contract

The POC implements these workflows:

| Group | Commands | Behavior |
| --- | --- | --- |
| Configuration | config get/set/unset | Atomic nonsecret user defaults; explicit flags/environment override saved paths and host. |
| Forge | auth status, repo view, pr list/view/diff/checks/review, api | Direct Forgejo requests; explicit writes require a token. |
| Governance | project list/view | Read team and project ownership, repositories, features, and secret namespace from TOML. |
| Kennel | status | Report kennel/build and kennel/deploy commit statuses and marked preview comments. |
| Infrastructure | hosts, service | Read host directories and locate service configuration files in a checkout. |

Repository selection accepts -R [HOST/]OWNER/REPO and otherwise reads upstream then origin. Explicit host/repository disagreements are errors. PR selection accepts a positive number or a same-host PR URL; omitted selection resolves the current branch through the pull list and refuses ambiguity. JSON output projects explicit supported gh-style fields. Forgejo-native responses remain accessible through api.

Review submission supports --approve, --request-changes, --comment, --body, and --body-file, including stdin. Exactly one review verdict is required. An optional JSON comments file carries path, line, side, and body. Validate line anchors against the inspected diff, then submit one review with that head SHA as commit_id. Do not retry mutations or silently retarget a changed head. The POC never submits reviews as part of its own live verification.

### API and authentication

Store only host and canonical checkout paths in the platform user configuration directory. Support explicit configuration files and a no-config mode for automation. Reject malformed or unknown settings and write atomically. Keep inferred Git remote hosts independent from the saved host default.

Default to git.cmu.dev. Accept COLLAR_TOKEN from the environment for authenticated operations, bound to COLLAR_HOST, the saved default host, or git.cmu.dev in that order; public reads work without credentials. An explicit alternate --host does not receive the default host's credential. No token is accepted on a command line or written to a file. Reject non-HTTPS hosts except loopback test servers, URL credentials, and redirects. Bound connect/request timeouts and pagination. Error messages include the operation and HTTP status without response bodies that may echo credentials. Typed response decoding must fail on malformed data.

Raw api requests are relative to the selected host's /api/v1 namespace. Match gh's string -f/--raw-field and typed -F/--field conventions, infer POST when fields are supplied, and permit an explicit method. Do not accept arbitrary absolute URLs that could receive the selected host's credentials.

### ScottyLabs data

Governance inspection reads an explicitly selected checkout and traverses team and nested project entries. It reports declared ownership rather than asserting that remote access has been applied. It does not replace governance-core validation or Terraform generation. Parse the subset needed for discovery while preserving feature names and settings as data; reject malformed required identity fields and ambiguous repository ownership.

Infrastructure discovery reads modules/hosts and resolves service filenames beneath those host directories. Results are configuration locations, not live service health. Do not parse Nix with regular expressions or claim filesystem discovery is evaluated flake output.

Kennel status combines commit statuses and Kennel-marked PR comments. Include the source commit and links; historical comments do not prove current service health. There is no deployment discovery API today. Do not read remote SQLite or require SSH administrator access.

### Modules

Use focused modules for argument parsing and orchestration, repository selection, the HTTP client, PR operations, governance discovery, and infrastructure discovery. Add shared abstractions only when multiple implemented commands need them. Use clap, reqwest, serde, serde_json, and TOML parsing with the workspace's conventions.

## Alternatives Considered

- An fj wrapper reduces implementation but does not provide control over gh compatibility and its release behavior.
- A gh fork carries GitHub REST/GraphQL assumptions and a Go implementation.
- Direct infrastructure and identity mutations create competing sources of truth. Retain governance PRs, Atlantis, Colmena, Bao, and secretspec as the owners of those operations.

## Open Questions

- Choose the minimum supported Forgejo version and capability contract for later phases.
- Design keyring-backed login and multi-host credential storage.
- Define authenticated Kennel list/log/redeploy APIs before adding those commands.
- Decide the long-term repository home after validating the POC in Terrier's workspace.
- Move Fetch into collar rfc in a later change, retaining the fetch executable during migration.

## Implementation Phases

1. Record this design, inspect existing RFCs/issues, and create an isolated branch.
1. Initialize the crate with cargo init and add dependencies with cargo add.
1. Implement the HTTP/repository/output boundaries and meaningful error-path tests.
1. Implement PR inspection, diff-anchored reviews, and mock-server payload tests.
1. Implement governance, Kennel, and infrastructure discovery with representative fixtures.
1. Document the supported command subset and add a development-shell entrypoint.
1. Run focused tests, clippy, formatting, applicable hooks, and the Nix package build. Check crate documentation links and perform public read-only smoke checks; record blockers accurately.
1. Review the POC before acceptance. Expand parity by command family with a tested compatibility inventory: issues/repositories/releases, Actions and secrets, scripting/extensions, then ScottyLabs mutations through existing review pipelines.

### Parity roadmap

| Family | POC | Follow-up work |
| --- | --- | --- |
| auth | Token verification; credentials bound to one host | Keyring login, multi-account switching, refresh, Git credential helper, SSH/GPG keys. |
| repo | View and Git remote discovery | Clone, create, fork, edit, archive, delete, sync, collaborators, deploy keys, mirrors. Governance-owned resources must use its PR workflow. |
| pr | List, view, diff, reported checks, commit-pinned review with inline comments | Create, checkout, edit, comment, ready, update branch, merge, close, reopen, status, revert; required-check and approval evaluation. |
| issue, label, release | Raw REST access | Typed lifecycle commands, search/filter parity, release assets, labels, milestones. |
| run, workflow, secret, variable | Raw REST access | Typed commands and capability detection. Run deletion is supported by the server; Forgejo rerun and workflow state management need server API work. |
| api and scripting | Simple typed/string fields, explicit methods, bounded typed lists, selected JSON fields | Nested fields, placeholders, API pagination, jq, templates, gh exit codes, completions, aliases, extensions. |
| search, projects, discussions | Not implemented | Use available Forgejo search APIs. Code/commit search and project/discussion APIs need backend decisions. |
| GitHub services | Not implemented | Codespaces, Copilot, attestations, and hosted agent tasks require separate services. |
| governance | Project ownership, features, namespace discovery | Reuse the existing validator; generate reviewed membership/feature PRs; track Atlantis application. |
| Kennel | Current-head commit statuses and historical preview comments | Authenticated project/build/deployment discovery and logs first; design redeploy/rollback semantics before mutations. |
| infrastructure | Host and service source discovery | Evaluated Nix inventory and diagnostics; preserve Atlantis for resource changes and Colmena for host activation. |
| RFCs | Existing fetch command retained | Incorporate Fetch as collar rfc with a compatibility entrypoint. |

Add a command family's request, response, failure-path, flag, JSON, and exit-code tests before claiming compatibility. A command name alone is not parity. The POC's runtime exit code is 1 on failure; full gh exit-code compatibility remains follow-up work.
