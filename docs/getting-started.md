# Installation and first steps

## Install from source

Install [Nix](https://nixos.org/download/) and
[devenv](https://devenv.sh/getting-started/), then clone Collar:

```sh
git clone ssh://forgejo@git.cmu.dev/ScottyLabs/collar.git
cd collar
devenv shell
collar --version
```

The development shell provides the Rust toolchain and runtime helpers. Its
`collar` command runs the source checkout and preserves your working directory.

For an installed binary with Git and the browser-login helper included:

```sh
nix profile add .#collar
```

The Nix package declares Linux x86-64, Linux ARM64, and Apple Silicon macOS.
Linux x86-64 is the tested platform. Other systems have not received the same
end-to-end validation.

Cargo installation is also available from the repository root:

```sh
cargo install --path . --locked
```

Cargo installs Collar itself. Provide `git` and `git-credential-oauth` on `PATH`
for Git operations and browser login. Local development commands additionally
need devenv, secretspec, or OpenBao as appropriate. Use `collar doctor` inside a
project to inspect the full setup.

## Read a repository

Public reads do not require sign-in or a local clone:

```sh
collar repo view collar
collar pr list -R collar
```

Inside a project checkout, Collar uses `upstream` if present, otherwise `origin`.
This makes work in a fork target the canonical repository when an upstream is
configured. Bare names such as `collar` default to the `ScottyLabs` owner. Use
`OWNER/REPO` to select another owner.

## Sign in

```sh
collar auth login
collar auth status
```

The first login opens Forgejo in your browser. Choose university SSO there.
Ordinary commands silently refresh expired OAuth access tokens using the saved
refresh token. `auth login` can also refresh an existing session without opening
a browser. Use `collar auth login --web` to start a fresh browser sign-in; an
existing Forgejo or university browser session may sign you in automatically. Collar verifies the credential and saves it in your operating system's
credential store. Browser login runs on the same machine as the browser.

SSH authenticates Git clone, fetch, and push. PR reviews and other API writes
use the separate credential from `collar auth login`; an SSH key alone does not
authenticate API requests.

For headless sessions, see [environment tokens](reference/configuration.md#headless-authentication).
For secrets access, use `collar auth bao login`; Forgejo and OpenBao logins are
separate.

## Set up a coding agent

```sh
collar skill install
```

The CLI bundles the skill; no checkout or download is needed. See
[agent setup](agents.md) for other clients, updates, and headless authentication.

## Review a pull request

```sh
collar pr view 42 -R ScottyLabs/collar
collar pr diff 42 -R ScottyLabs/collar
collar pr review 42 --comment --body 'Could you add a regression test?' -R ScottyLabs/collar
```

Review commands submit immediately. Use `--approve` or `--request-changes` when
you intend those verdicts. See the [pull request guide](guides/pull-requests.md)
for inline comments, checkout, and merging.
