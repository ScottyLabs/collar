# Collar

Collar is a Rust CLI that connects Forgejo pull requests with ScottyLabs governance, Kennel reports, and infrastructure configuration. It is a proof of concept with a defined command subset. It does not yet implement every GitHub CLI command or flag.

## Run Collar

Inside the development shell:

```bash
collar --help
collar repo view ScottyLabs/terrier
collar pr list -R ScottyLabs/terrier --json number,title,headRefOid
```

Outside the shell, prefix commands with `devenv shell --`. You can also build with `cargo build -p collar`, install with `cargo install --path crates/collar`, or run `nix run .#collar -- --help` on x86_64 Linux.

`-R [HOST/]OWNER/REPO` selects a repository. Without it, Collar reads the `upstream` remote, then `origin`. `--host` or `COLLAR_HOST` selects a forge; the default is git.cmu.dev. A remote or repository that disagrees with the selected host is an error.

## Save defaults

```bash
collar config set host git.cmu.dev
collar config get --json
collar config unset host
```

Settings live in the platform's user configuration directory, such as `$XDG_CONFIG_HOME/collar/config.toml` on Linux. Use `--config PATH` or `COLLAR_CONFIG` for a different file, or `--no-config` to ignore saved settings and logins. An explicitly selected missing file is an error except when creating it with `config set` or `auth login`. Invalid settings fail before running a command; they are never silently replaced. Writes replace the file atomically.

Settings contain the default forge host and nonsecret saved-login host metadata. Normal governance and infrastructure inspection always resolves the latest canonical default-branch revision, so no repository path or revision needs configuration. Explicit local checkout flags and environment variables support deliberate inspection of local changes. A saved host supplies the default for API requests and `OWNER/REPO` selections. An explicit repository host or inferred Git remote takes precedence; an explicit `--host` still checks for disagreements.

## Diagnose setup

```bash
collar doctor
collar doctor --offline --json
collar doctor --profile preview
```

Run inside a project checkout. Governance and infrastructure inspection uses canonical API snapshots by default; `--governance PATH` and `--infrastructure PATH` override them with local working trees. Diagnostics check runnable tools, Git identity and SSH-agent keys, devenv files, canonical project ownership, and secretspec namespace/profile declarations. Snapshot diagnostics report their pinned revisions. Local checkout diagnostics check for clean Git roots matching their last-fetched upstreams. Doctor does not fetch or update checkouts; matching a local tracking branch does not establish remote freshness.

Online mode checks forge reachability and your host-bound token, then verifies your Bao token against ScottyLabs OpenBao. Tool output is suppressed, tool and Bao probes have time limits, and secrets are never resolved. `--offline` skips all network requests and uses existing source snapshots, or explicit local checkouts. Checks report `pass`, `fail`, `manual`, or `skipped`; failures produce exit code 1 after printing the full report. Account linking remains a manual check even when all automated checks pass. An SSH-agent key does not prove forge SSH access; a Bao token does not prove access to a project's secrets. Doctor checks configuration presence, without evaluating devenv or infrastructure or asserting deployment health.

If a new machine cannot enter the shell because Bao login is missing, use the standalone login from [Kennel's local development guide](https://docs.kennel.scottylabs.org/guides/secrets.html#local-development):

```bash
nix run git+https://git.cmu.dev/ScottyLabs/kennel#login
```

## Authentication

Public reads work without login. Reviews and raw API writes require a personal access token from [Forgejo application settings](https://git.cmu.dev/user/settings/applications). Choose scopes appropriate to the operation; PR reviews require repository access.

```bash
collar auth login
collar auth status
collar auth logout
```

`auth login` signs in through Forgejo in your browser and verifies the resulting OAuth credential against the selected forge. Collar stores access and refresh tokens together in the OS credential store; running `auth login` again refreshes the credential. An expired access token never triggers a browser prompt during another command. Use `auth login --token` for a hidden personal access token prompt, or `auth login --with-token` to explicitly read stdin, including input from a password manager. Tokens are never accepted as command-line arguments, and stdin input is bounded to 64 KiB. See [browser sign-in details](SSO.md). Linux requires an available Secret Service and session bus; macOS uses Keychain and Windows uses Credential Manager through the [keyring library](https://docs.rs/keyring/latest/keyring/v1/index.html). Only Linux has been exercised here with a real isolated store. Unlocking the OS store may require its own interaction. Store failures are explicit, with no plaintext fallback. Config-saving failures attempt to restore the previous credential and report any recovery failure.

Each canonical forge origin has one saved credential, shared across configuration files. A configuration's nonsecret `logins` list identifies which origins to use; `--no-config` disables saved-login lookup. Public reads with no registered login do not open the OS store. A registered but missing or inaccessible credential is an error. `auth logout` removes the selected origin's stored credential and this configuration's marker; it does not revoke the server token or unset environment variables. Other configuration files using the same credential must log in again.

Read a token into the environment without putting it in shell history:

```bash
read -rs -p 'Forgejo token: ' COLLAR_TOKEN
echo
export COLLAR_TOKEN
collar auth status
```

`COLLAR_TOKEN` takes precedence over a saved credential only for its bound host: `COLLAR_HOST`, or the saved default host, or git.cmu.dev in that order. To use a different environment-authenticated forge, set both variables. `--host other.example` alone never sends the git.cmu.dev credential there; it can use that other origin's own registered saved credential. Tokens are never stored in Collar's config or accepted as command-line arguments. Connections require HTTPS, except loopback HTTP for local tests. Redirects are refused.

This token is separate from OpenBao login and application OIDC credentials. Collar does not change your devenv secrets or Keycloak account links.

## Environment and repository checks

```bash
collar auth bao login
collar auth bao status
collar secrets check --profile dev --json --reason 'Check local setup'
collar secrets check --profile prod --scope build --reason 'Check build requirements'
collar secrets set JWT_SECRET --profile dev
collar secrets set JWT_SECRET --profile dev --from-file ./private-secret-file
collar dev shell
collar dev run -- cargo test --workspace
collar dev up --detach
collar dev task security:
collar dev down
collar check --all
collar check treefmt --files crates/collar/src/main.rs
```

Bao login works before a project shell is available: Nix supplies OpenBao, then OIDC login stores the token in Bao's configured helper with token printing disabled. `auth bao status` suppresses token metadata and checks token acceptance against ScottyLabs OpenBao; it does not establish project permissions.

Secret and development commands resolve the selected repository's latest canonical governance project, then validate the local `secretspec.toml` namespace and selected profile before starting a tool. `--provider` must name a provider declared in that file. Use `--governance PATH` or `COLLAR_GOVERNANCE` for deliberate local inspection. `--offline-source` uses the last verified governance snapshot; it controls source discovery only, and the invoked tools can still access the network.

Secret checks never prompt to set missing values. They emit secretspec's value-free explanation or native JSON report and propagate failure codes. Set accepts declared keys from the chosen profile or its default profile. Without `--from-file`, a terminal prompt is required; `--from-file -` explicitly accepts exact secret bytes from stdin. Collar never accepts a secret value argument or saves secret values. Secretspec owns provider access and updates. These commands require secretspec 0.21 or newer.

Use `--reason` or `SECRETSPEC_REASON` to supply an access reason. Collar forwards it unchanged and supplies its caller metadata; it never invents a reason or bypasses the provider's access policy. `--profile` selects the secret profile and honors `SECRETSPEC_PROFILE`, defaulting to `dev`. Devenv receives `--secretspec-profile`, not its separate Nix development-profile flag, and runs from the project root. `dev run -- COMMAND ...` preserves literal argument boundaries. `dev down` stops processes without requiring governance or secrets preflight.

`check` runs `prek run` inside the project development shell, using staged files by default. Supply a hook identifier, `--all`, or `--files` for existing files inside the checkout. File paths are relative to the calling directory or absolute. Tool execution inherits normal terminal output and preserves failure exit codes. Installed tools and project configuration remain authoritative for their own workflows.

## Review pull requests

```bash
collar pr view 42
collar pr diff 42
collar pr checks 42
collar pr review 42 --approve
collar pr review 42 --request-changes --body 'Please add a failure-path test.'
collar pr review 42 --comment --body-file review.md
```

PR commands accept a number, a PR URL belonging to the selected repository, or a branch name. Omit the selection to find the open PR for your current branch. Ambiguous branch matches require a number.

```sh
collar pr create --title 'Add preview health check' --head feature/health --body-file description.md
collar pr edit 42 --title 'Check preview health' --body-file description.md
collar pr comment 42 --body 'Ready for review.'
collar pr close 42
collar pr reopen 42
collar pr checkout 42
collar pr merge 42 --squash --delete-branch
collar pr update-branch 42 --rebase
```

Creation uses an already-pushed branch; omitting `--head` reads the current Git branch, and omitting `--base` uses the forge repository's default branch. It does not push commits. Edit supports `--title`, `--base`, and body text/files; omitted fields remain unchanged, while `--body ''` clears the description. `--body-file -` reads stdin. Comments require nonempty text. Close/reopen refuse merged PRs and an already-requested state. These commands write immediately, require a host-bound token, support JSON output, and never retry a mutation automatically. The server enforces permissions and validates the current state.

`checkout` fetches `refs/pull/NUMBER/head` through the configured `upstream` remote, then `origin`, or an explicit `--remote NAME`. Its configured URL must match the selected forge repository. Git handles transport authentication and configured URL rewrites; Collar does not pass its API token to Git. The worktree must be clean, including untracked files and submodule changes. Collar verifies the fetched commit against the inspected PR head before switching to `pr/NUMBER`, or `--branch NAME`. An existing branch is reused only at that same commit; it is never reset. Ignored files are protected from overwrite. Fetches update only Collar's private `refs/collar/pulls/NUMBER` reference, without fetching tags or writing FETCH_HEAD. A failed verification can leave fetched objects and that private reference, but does not switch branches. Closed PRs can also be inspected this way if Forgejo retains their pull ref.

Merge requires exactly one of `--merge`, `--squash`, or `--rebase`. Immediate merge sends the inspected SHA as Forgejo's expected head, with force merging disabled. `--match-head-commit SHA` additionally requires that specific head before submission. `--delete-branch` asks Forgejo to delete the source branch after merging. `--auto` requests scheduled merging when server checks succeed; Forgejo's scheduled path does not enforce the expected SHA, so it cannot be combined with `--match-head-commit` and can merge a later head. Server permissions, required reviews, and branch protections remain authoritative; reported statuses alone do not prove readiness.

`update-branch` merges the target branch into the PR head, or rebases with `--rebase`. Its optional `--match-head-commit` is a preflight check: Forgejo's update endpoint has no atomic expected-head guard. Both commands require an open, unmerged PR. JSON reports request acceptance and the inspected `headRefOid`; it does not claim that a scheduled merge completed or give the new branch-update SHA. Inspect the PR again for current state.

`collar pr browse` provides an opt-in terminal picker and read-only details, diff, and commit-status navigation. `collar project select` picks a governed repository and prints its project context. Type to filter, use arrow keys to move, and Enter to select. Escape cancels with exit code 130. Both require terminal stdin and stderr; prompts use stderr so `--json` output can be redirected. With `pr browse --json`, selecting a PR emits its current metadata and exits. Reported commit statuses do not establish required-check coverage or merge readiness.

See the [governance feature catalog](FEATURES.md) for managed services, secret names, and access workflows.

## ScottyLabs changelog

```sh
collar changelog view kennel
collar changelog view infrastructure --json
collar changelog updates
collar changelog update 17
collar changelog new kennel-sqlite --checkout /path/to/changelog
```

Read commands resolve the latest default-branch revision of `ScottyLabs/changelog` and use the same verified snapshots and explicit `--offline` behavior as governance and infrastructure inspection. `view` reads the repository's generated Kennel, infrastructure, or governance summary; `updates` lists numbered tech-lead announcements newest first, and `update` reads one by number. JSON fields are `path`, `number`, `title`, `body`, and `dataSource`. `--checkout PATH` allows deliberate local inspection.

`new` requires an explicit working checkout and invokes its existing `new-update.sh` with a validated lowercase slug. The helper creates the numbered announcement template and prints its path. Fill in the title, affected users, change instructions, verification, and references following that repository's `AGENTS.md`. Review the entry before publishing: new announcements pushed to its main branch notify tech leads on Discord. Collar does not commit or publish the entry. Generated summaries remain owned by the repository's hourly git-cliff workflow.

Exactly one of `--approve`, `--request-changes`, or `--comment` is required. `--body-file -` reads stdin. Comments and requested changes require text or inline comments.

For inline comments, create a JSON file:

```json
[
  {
    "path": "src/main.rs",
    "line": 12,
    "side": "new",
    "body": "Could this error include the operation that failed?"
  }
]
```

```bash
collar pr review 42 --request-changes --comments-file comments.json
```

`side` is `new` for the PR's file or `old` for the base version. Lines must occur in a complete, valid diff hunk. Quoted UTF-8 Git filenames and old-side comments on fully deleted files are supported. For renamed files, use the new path on either side. Collar validates anchors, rechecks the PR's head and open state, and submits all comments in one review pinned to that commit. It never retries a write automatically. If a connection fails during submission, inspect the PR before retrying.

`pr checks` reports the latest commit status for each context. It exits unsuccessfully when no statuses exist or any reported status is not successful. It does not verify required branch protection or prove that every required check has reported.

## Project context

```bash
collar project view
collar project view -R ScottyLabs/terrier --json
```

No governance checkout is required. Use `--checkout PATH`, `COLLAR_GOVERNANCE`, to inspect local changes instead. Collar matches the selected repository against the forge and organization in `data/org.toml`, then reports the owning project, inherited leads and members, repository-specific features, secret namespace, and source files. For a fork, select the canonical upstream with `-R`.

The secret namespace is the governance slug, which can differ from the repository name. Use it as `[project].name` in `secretspec.toml`. The account-linking URL comes from the organization's Keycloak configuration. These are declared settings; neither applied access nor deployment health is verified. Local checkout overrides are never automatically refreshed.

## Inspect ScottyLabs projects

Collar reads the canonical [governance repository](https://git.cmu.dev/ScottyLabs/governance) through Forgejo. Its GitHub mirror may lag behind.

```bash
collar governance project list
collar governance project view terrier --json
collar governance project list --offline
```

Use `--checkout PATH` or `COLLAR_GOVERNANCE` only when inspecting local governance changes. Discovery includes nested projects, inherited leads and members, enabled feature settings, and the secret namespace. The namespace is the project or team slug, which can differ from the repository name. Team-owned repositories alongside nested projects have no namespace in the current governance loader; Collar reports `null` for that case.

These commands inspect declared configuration. They do not validate the entire governance schema, verify applied access, change memberships, or run OpenTofu. Use the governance repository's existing validator and PR/Atlantis workflow for changes.

Inspect deployment reports on a PR:

```bash
collar kennel status 42 -R ScottyLabs/terrier --json
```

The report includes the head SHA, latest `kennel/build` and `kennel/deploy` commit statuses, and comments containing Kennel's deployment marker. Comments may describe earlier revisions or teardown. Neither comments nor commit statuses prove live service health. No reports is an explicit error.

Inspect canonical [infrastructure configuration](https://git.cmu.dev/ScottyLabs/infrastructure):

```bash
collar infra hosts
collar infra service kennel --json
collar infra hosts --offline
```

Use `--checkout PATH` or `COLLAR_INFRASTRUCTURE` only when inspecting local infrastructure changes. Host discovery reads `modules/hosts` and checks each configuration and role file. Service discovery finds `<service>.nix` and `<service>/default.nix` under host directories. These are source locations, not evaluated Nix configuration or live health. Collar does not deploy hosts or apply Atlantis plans.

## Canonical source snapshots

Governance, project-context, and infrastructure inspection require no manually maintained clones. Online commands resolve the canonical repository's default branch, pin its full commit ID, and fetch the required source files through Forgejo REST endpoints, using host-bound credentials when available. Tree pages and file sizes are checked before publishing a complete snapshot. Subsequent calls check the branch again and reuse an intact snapshot when its revision is unchanged.

Snapshots live under the platform user cache directory. `--cache-dir PATH` or `COLLAR_CACHE_DIR` selects a different location. `--offline` explicitly reads the last complete snapshot without network requests; missing or corrupt snapshots fail. A failed online refresh never silently falls back to cached data. Each result's `dataSource` identifies the kind, host/repository, revision, download time, and offline mode. Human output prints the source before its results.

Explicit checkout flags and environment variables select deliberate local inspection instead of snapshots. Repository paths are not saved in Collar settings. Checkout results identify their working-tree path and use a null revision because local edits may differ from HEAD. Snapshots contain the governance organization/team files or infrastructure flake/host/role Nix sources needed for inspection. Editing, canonical validation, and Nix evaluation use full working checkouts. Cached files are not deployment health or proof that declared access has been applied.

Refreshes are bounded to 100 tree pages, 500 selected files, 8 MiB per file, and 32 MiB per snapshot. Traversal, symlinks, duplicate paths, partial tree pages, and inconsistent file sizes are refused. Immutable revisions remain cached; remove unwanted revision directories from the user cache when needed. File sizes and Git blob hashes are verified on download and on every cached read. Modified content fails validation.

## Scripting and API requests

Inspection commands accept `--json` for all fields or `--json field1,field2` for selected top-level fields. PR output uses names such as `number`, `title`, `url`, `headRefName`, `headRefOid`, `baseRefName`, `author`, and `isDraft`. Use external `jq` for filtering.

```bash
collar api version
collar api repos/ScottyLabs/terrier/pulls --method GET -F limit=5
```

`api` targets `/api/v1` on the selected forge. `-f/--raw-field` adds a string and `-F/--field` adds a typed JSON value; `-F key=@file` reads a string from a file. Fields or `--input FILE` imply POST unless `--method` is given. An explicit GET sends fields as query parameters. API writes take effect immediately and require a token.

Absolute URLs, traversal, nested field names, duplicate fields, and arbitrary headers are not supported. The POC has no GraphQL, built-in jq, output templates, API pagination flag, keyring login, or extension manager. Typed list commands paginate with a 100-page bound and fail if completeness cannot be established within it. Responses are limited to 8 MiB.

Success exits with code 0; runtime errors exit with code 1; delegated tool commands preserve their failure exit codes; argument errors use clap's code 2. These codes are not yet full gh compatibility. Unsupported commands fail through argument parsing rather than reporting placeholder success.

## RFC search and creation

```bash
collar rfc session redis
collar rfc --status draft saml
collar rfc --status all
collar rfc --new "Rate Limiting"
collar rfc --directory crates/collar --status all
```

Fetch is fully incorporated into Collar. The separate crate and `fetch` shell command are removed. Search defaults to accepted RFCs, ignores case, and requires all query words in a section or its title. Title matches rank first, followed by the number of matching sections and then filename order. Results include excerpts, source lines, heading anchors, updated dates, and declared RFC relationships. Missing status metadata produces a warning. Piped output is plain text.

Without a query, list matching RFCs. `--new` creates a numbered Draft with the RFC template and UTC dates, refuses invalid titles, and never overwrites an existing file. Search and creation cannot be combined.

Use `--directory PATH` to select the document directory. Otherwise Collar uses `DEVENV_ROOT/rfcs` or the current Git checkout's `rfcs` directory. Numbered RFCs and standalone `RFC.md` files are searchable; ordinary README files are excluded. There is no compiled-in Terrier path or search index. Outside the development shell, use `devenv shell -- collar rfc ...`; tests run with `cargo test -p collar --test rfc`.

## Development

Run checks in the repository development shell:

```bash
cargo test -p collar
cargo clippy -p collar --all-targets -- -D warnings
```

Tests use local HTTP fixtures and temporary checkouts. Review payloads are verified without remote writes.

The [design and parity roadmap](RFC.md) live alongside the implementation.

The [development review](REVIEW.md) defines the complete ScottyLabs workflows and tracks integration constraints and remaining work.
