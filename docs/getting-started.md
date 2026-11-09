# Installation and first steps

## Use the dev shell

Every ScottyLabs project's dev shell includes Collar. Install Lix by following kennel's [local development guide](https://docs.kennel.scottylabs.org/guides/local-development/#installing-nix), then in a project checkout:

```sh
nix develop
collar --version
```

If `collar` isn't found, make sure your project's kennel input is new enough (`nix flake update scottylabs`).

## Work on Collar

In Collar's own dev shell, the `collar` command runs the source checkout through `cargo run` and preserves your working directory:

```sh
git clone ssh://forgejo@git.cmu.dev/ScottyLabs/collar.git
cd collar
nix develop
collar --version
```

## Read a repository

Public reads do not require sign-in or a local clone:

```sh
collar repo view collar
collar pr list -R collar
```

Inside a project checkout, Collar uses `upstream` if present, otherwise `origin`. This makes work in a fork target the canonical repository when an upstream is configured. Bare names such as `collar` default to the `ScottyLabs` owner. Use `OWNER/REPO` to select another owner.

## Sign in

```sh
collar auth login
collar auth status
```

The first login opens Forgejo in your browser. Choose university SSO there. Ordinary commands silently refresh expired OAuth access tokens using the saved refresh token. `auth login` can also refresh an existing session without opening a browser. Use `collar auth login --web` to start a fresh browser sign-in; an existing Forgejo or university browser session may sign you in automatically. Collar verifies the credential and saves it in your operating system's credential store. Browser login runs on the same machine as the browser.

SSH authenticates Git clone, fetch, and push. PR reviews and other API writes use the separate credential from `collar auth login`; an SSH key alone does not authenticate API requests.

For headless sessions, see [environment tokens](reference/configuration.md#headless-authentication). For secrets access, use `collar auth bao login`; Forgejo and OpenBao logins are separate.

## Set up a coding agent

```sh
collar skill install
```

The CLI bundles the skill; no checkout or download is needed. See [agent setup](agents.md) for other clients, updates, and headless authentication.

## Review a pull request

```sh
collar pr view 42 -R ScottyLabs/collar
collar pr diff 42 -R ScottyLabs/collar
collar pr review 42 --comment --body 'Could you add a regression test?' -R ScottyLabs/collar
```

Review commands submit immediately. Use `--approve` or `--request-changes` when you intend those verdicts. See the [pull request guide](guides/pull-requests.md) for inline comments, checkout, and merging.
