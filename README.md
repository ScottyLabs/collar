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
collar config set governance ../governance
collar config set infrastructure ../infrastructure
collar config set host git.cmu.dev
collar config get --json
collar config unset host
```

Settings live in the platform's user configuration directory, such as `$XDG_CONFIG_HOME/collar/config.toml` on Linux. Use `--config PATH` or `COLLAR_CONFIG` for a different file, or `--no-config` to ignore saved settings. An explicitly selected missing file is an error except when creating it with `config set`. Invalid settings fail before running a command; they are never silently replaced. Writes replace the file atomically.

Only the default host and governance/infrastructure checkout paths are saved. Checkout flags and environment variables override saved paths. `config set` checks the checkout layout and saves an absolute path; manually entered relative paths resolve from the configuration file's directory. A saved host supplies the default for API requests and `OWNER/REPO` selections. An explicit repository host or inferred Git remote takes precedence; an explicit `--host` still checks for disagreements.

## Authentication

Public reads work without login. Reviews and raw API writes require a personal access token from [Forgejo application settings](https://git.cmu.dev/user/settings/applications). Choose scopes appropriate to the operation; PR reviews require repository access.

Read a token into the environment without putting it in shell history:

```bash
read -rs -p 'Forgejo token: ' COLLAR_TOKEN
echo
export COLLAR_TOKEN
collar auth status
```

Collar does not save the token or accept it as a command-line argument. `COLLAR_TOKEN` belongs to `COLLAR_HOST`, or the saved default host, or git.cmu.dev in that order. To use a different authenticated forge, set both variables. `--host other.example` alone never sends the git.cmu.dev credential there. Connections require HTTPS, except loopback HTTP for local tests. Redirects are refused.

This token is separate from OpenBao login and application OIDC credentials. Collar does not change your devenv secrets or Keycloak account links.

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

Save a governance checkout first, or supply `--checkout PATH` or `COLLAR_GOVERNANCE`. Collar matches the selected repository against the forge and organization in `data/org.toml`, then reports the owning project, inherited leads and members, repository-specific features, secret namespace, and source files. For a fork, select the canonical upstream with `-R`.

The secret namespace is the governance slug, which can differ from the repository name. Use it as `[project].name` in `secretspec.toml`. The account-linking URL comes from the organization's Keycloak configuration. These are local declarations; neither applied access nor deployment health is verified, and the checkout is not automatically refreshed.

## Inspect ScottyLabs projects

Clone the canonical [governance repository](https://git.cmu.dev/ScottyLabs/governance). Its GitHub mirror may lag behind.

```bash
collar governance project list --checkout ../governance
collar governance project view terrier --checkout ../governance --json
```

Save a governance default with `config set governance PATH`, or set `COLLAR_GOVERNANCE`, to avoid repeating `--checkout`. Discovery includes nested projects, inherited leads and members, enabled feature settings, and the secret namespace. The namespace is the project or team slug, which can differ from the repository name. Team-owned repositories alongside nested projects have no namespace in the current governance loader; Collar reports `null` for that case.

These commands inspect declared configuration. They do not validate the entire governance schema, verify applied access, change memberships, or run OpenTofu. Use the governance repository's existing validator and PR/Atlantis workflow for changes.

Inspect deployment reports on a PR:

```bash
collar kennel status 42 -R ScottyLabs/terrier --json
```

The report includes the head SHA, latest `kennel/build` and `kennel/deploy` commit statuses, and comments containing Kennel's deployment marker. Comments may describe earlier revisions or teardown. Neither comments nor commit statuses prove live service health. No reports is an explicit error.

Inspect an [infrastructure checkout](https://git.cmu.dev/ScottyLabs/infrastructure):

```bash
collar infra hosts --checkout ../infrastructure
collar infra service kennel --checkout ../infrastructure --json
```

Save an infrastructure default with `config set infrastructure PATH`, or set `COLLAR_INFRASTRUCTURE`, to avoid repeating `--checkout`. Host discovery reads `modules/hosts` and checks each configuration and role file. Service discovery finds `<service>.nix` and `<service>/default.nix` under host directories. These are source locations, not evaluated Nix configuration or live health. Collar does not deploy hosts or apply Atlantis plans.

## Scripting and API requests

Inspection commands accept `--json` for all fields or `--json field1,field2` for selected top-level fields. PR output uses names such as `number`, `title`, `url`, `headRefName`, `headRefOid`, `baseRefName`, `author`, and `isDraft`. Use external `jq` for filtering.

```bash
collar api version
collar api repos/ScottyLabs/terrier/pulls --method GET -F limit=5
```

`api` targets `/api/v1` on the selected forge. `-f/--raw-field` adds a string and `-F/--field` adds a typed JSON value; `-F key=@file` reads a string from a file. Fields or `--input FILE` imply POST unless `--method` is given. An explicit GET sends fields as query parameters. API writes take effect immediately and require a token.

Absolute URLs, traversal, nested field names, duplicate fields, and arbitrary headers are not supported. The POC has no GraphQL, built-in jq, output templates, API pagination flag, keyring login, or extension manager. Typed list commands paginate with a 100-page bound and fail if completeness cannot be established within it. Responses are limited to 8 MiB.

Success exits with code 0; runtime errors exit with code 1; argument errors use clap's code 2. These codes are not yet full gh compatibility. Unsupported commands fail through argument parsing rather than reporting placeholder success.

Fetch remains available for RFC search. A future `collar rfc` command can incorporate it without removing the existing executable in this POC.

## Development

Run checks in the repository development shell:

```bash
cargo test -p collar
cargo clippy -p collar --all-targets -- -D warnings
```

Tests use local HTTP fixtures and temporary checkouts. Review payloads are verified without remote writes.

The [design and parity roadmap](RFC.md) live alongside the implementation.

The [development review](REVIEW.md) defines the complete ScottyLabs workflows and tracks integration constraints and remaining work.
