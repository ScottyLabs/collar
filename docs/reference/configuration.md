# Configuration and authentication

## Repository and host selection

Repository commands use the first available remote named `upstream`, then
`origin`. `-R [HOST/][OWNER/]REPO` selects a repository directly and works outside a
checkout. An omitted owner defaults to `ScottyLabs`: `-R terrier` selects
`ScottyLabs/terrier`. Both `REPO` and `OWNER/REPO` use the explicit host, saved host, or `git.cmu.dev`.
An inferred Git remote or explicit repository host supplies its own forge origin.
An explicit `--host` must agree with that origin.

```sh
collar pr list -R ScottyLabs/collar
collar repo view -R git.cmu.dev/ScottyLabs/collar
collar config set host git.cmu.dev
collar config get --json
collar config unset host
```

The host must be an HTTPS origin, without credentials, a path, or a query.
Loopback HTTP is supported for local development. Requests never follow redirects.

## Settings

Settings live in the operating system's user configuration directory, such as
`$XDG_CONFIG_HOME/collar/config.toml` on Linux. They store the default host and
nonsecret login-origin markers. Tokens are stored separately.

`--config PATH` or `COLLAR_CONFIG` selects a different settings file.
`--no-config` ignores saved defaults and saved logins. Missing explicit settings
files fail unless the command is creating settings or saving a login. Invalid
settings fail rather than being silently replaced.

## Browser sign-in

```sh
collar auth login
collar auth status
collar auth logout
```

Login opens Forgejo, where you can choose university SSO. The browser helper
`git-credential-oauth` uses Forgejo's registered application and loopback
callback. It must be installed on `PATH`, and the browser must run on the same
machine as Collar. The Nix package includes the helper.

Collar verifies the credential against `/api/v1/user` and stores access and
refresh tokens in the OS credential store. Linux needs Secret Service and a
session bus; macOS uses Keychain and Windows uses Credential Manager. These
platform integrations come from the
[keyring library](https://docs.rs/keyring/latest/keyring/v1/index.html); Linux is
the platform exercised with a real store.

If a token expires, run `auth login` again. Login can use the saved refresh token;
other commands do not refresh or open a browser implicitly. `auth logout`
removes the selected origin's saved credential and marker, but does not revoke
the server token or unset environment variables.

Git's SSH authentication, Forgejo API authentication, and OpenBao login are
separate. Account linking remains available through
[Keycloak](https://idp.scottylabs.org/realms/scottylabs/account/account-security/linked-accounts).

## Headless authentication

Use a personal access token from
[Forgejo application settings](https://git.cmu.dev/user/settings/applications)
with the permissions needed for the task. Provision it through the runner's
secret environment:

```sh
export COLLAR_HOST=git.cmu.dev
# COLLAR_TOKEN is supplied by the runner or a secret manager.
collar auth status
```

`COLLAR_TOKEN` takes precedence over a saved credential for its bound origin:
`COLLAR_HOST`, then the saved host, then `git.cmu.dev`. Changing `--host` alone
does not forward a token to another origin. Set both environment variables to
use an environment token on a different forge.

`auth login --token` prompts for a PAT; `auth login --with-token` reads it from
stdin explicitly. Never put a token in command arguments, source files, or logs.
There is no plaintext fallback when the OS store is unavailable.

## Environment variables

| Variable | Purpose |
| --- | --- |
| `COLLAR_HOST` | Forge origin and environment-token binding |
| `COLLAR_TOKEN` | API token for that bound origin |
| `COLLAR_CONFIG` | Alternate settings file |
| `COLLAR_CACHE_DIR` | Canonical source cache directory |
| `COLLAR_GOVERNANCE` | Optional local governance inspection override |
| `COLLAR_INFRASTRUCTURE` | Optional local infrastructure inspection override |
| `SECRETSPEC_PROFILE` | Profile for development and secret commands |
| `SECRETSPEC_PROVIDER` | Provider selection for development and secret commands |
| `SECRETSPEC_REASON` | Access reason forwarded to the underlying tool |
| `SECRETSPEC_SCOPE` | Scope for `secrets check` |
| `NO_COLOR` | Disable styled output |

## Source cache

Read-only project commands resolve the latest default-branch revision through
Forgejo. Snapshots live under the OS user cache directory. Each result identifies
its source in `dataSource`, including the revision and offline status.

`--offline` uses the last verified snapshot, without a source API request. It
fails when that snapshot is missing or corrupt. A failed online refresh does not
silently fall back to cached content. Local checkout overrides return their
working-tree path and no canonical revision.
