# Configuration and authentication

## Repository and host selection

Repository commands use the first available remote named `upstream`, then `origin`. `-R REPO`, `-R OWNER/REPO`, or `-R HOST/OWNER/REPO` selects a repository directly and works outside a checkout. An omitted owner defaults to `ScottyLabs`: `-R terrier` selects `ScottyLabs/terrier`. Both `REPO` and `OWNER/REPO` use the explicit host, saved host, or `git.cmu.dev`. An inferred Git remote or explicit repository host supplies its own forge origin. An explicit `--host` must agree with that origin.

```sh
collar pr list -R ScottyLabs/collar
collar repo view -R git.cmu.dev/ScottyLabs/collar
collar config set host git.cmu.dev
collar config get --json
collar config unset host
```

The host must be an HTTPS origin, without credentials, a path, or a query. Loopback HTTP is supported for local development. Requests never follow redirects.

## Settings

Settings live in the operating system's user configuration directory, such as `$XDG_CONFIG_HOME/collar/config.toml` on Linux. They store the default host and nonsecret login-origin markers. Tokens are stored separately.

`--config PATH` or `COLLAR_CONFIG` selects a different settings file. `--no-config` ignores saved defaults and saved logins. Missing explicit settings files fail unless the command is creating settings or saving a login. Invalid settings fail rather than being silently replaced.

## Browser sign-in

```sh
collar auth login
collar auth status
collar auth logout
```

Browser login uses Keycloak client `collar` and its fixed-provider Forgejo broker. PKCE, callback state, ID-token signature, issuer, audience, authorized party, and nonce are verified. The browser must run on the same machine as Collar. Linux uses `xdg-open`, included in the Nix runtime; macOS uses `open`.

Keycloak sessions are saved in the OS credential store under the forge origin, issuer, and client. Forgejo refresh tokens remain server-side. Linux needs Secret Service and a session bus; macOS uses Keychain. Personal access tokens and host-bound environment tokens remain available for automation and alternate forge hosts. No credential is stored in Collar's configuration file.

Ordinary commands refresh an expired SSO session and save rotated credentials before querying services. Commands never open a browser implicitly. `auth logout` attempts server-side revocation and removes the local session even if revocation cannot be confirmed; its error then directs you to revoke the session in Keycloak. Environment tokens and manually supplied server tokens are unchanged.

The defaults are issuer `https://idp.scottylabs.org/realms/scottylabs`, broker `https://collar-broker.scottylabs.org/`, and Grafana `https://grafana.scottylabs.org/`. `COLLAR_SSO_ISSUER`, `COLLAR_SSO_BROKER`, and `COLLAR_GRAFANA` select explicitly trusted deployments. They require HTTPS except on loopback. Saved sessions are bound to all three endpoints and cannot be redirected by changing these settings. Use the original settings to log out before changing deployments.

Git's SSH authentication, Forgejo API authentication, and OpenBao login are separate.

## Headless authentication

Use a personal access token from [Forgejo application settings](https://git.cmu.dev/user/settings/applications) with the permissions needed for the task. Provision it through the runner's secret environment:

```sh
export COLLAR_HOST=git.cmu.dev
# COLLAR_TOKEN is supplied by the runner or a secret manager.
collar auth status
```

`COLLAR_TOKEN` takes precedence over a saved credential for its bound origin: `COLLAR_HOST`, then the saved host, then `git.cmu.dev`. Changing `--host` alone does not forward a token to another origin. Set both environment variables to use an environment token on a different forge.

`auth login --token` prompts for a PAT; `auth login --with-token` reads it from stdin explicitly. Never put a token in command arguments, source files, or logs. There is no plaintext fallback when the OS store is unavailable.

## Environment variables

| Variable | Purpose |
| -- | -- |
| `COLLAR_HOST` | Forge origin and environment-token binding |
| `COLLAR_TOKEN` | API token for that bound origin |
| `COLLAR_CONFIG` | Alternate settings file |
| `COLLAR_CACHE_DIR` | Canonical source cache directory |
| `COLLAR_GOVERNANCE` | Optional local governance inspection override |
| `COLLAR_KENNEL` | Kennel origin, `https://kennel.scottylabs.org` by default |
| `SECRETSPEC_PROFILE` | Profile for development and secret commands |
| `SECRETSPEC_PROVIDER` | Provider selection for development and secret commands |
| `SECRETSPEC_REASON` | Access reason forwarded to the underlying tool |
| `SECRETSPEC_SCOPE` | Scope for `secrets check` |
| `NO_COLOR` | Disable styled output |

## Source cache

Read-only project commands resolve the latest default-branch revision through Forgejo. Snapshots live under the OS user cache directory. Each result identifies its source in `dataSource`, including the revision and offline status.

`--offline` uses the last verified snapshot, without a source API request. It fails when that snapshot is missing or corrupt. A failed online refresh does not silently fall back to cached content. Local checkout overrides return their working-tree path and no canonical revision.

A successful online refresh removes older revisions for that source and forge host, keeping the verified current snapshot for offline use. Each snapshot holds at most 32 MiB of file content, plus its manifest. Across the five source types, retained file content is limited to 160 MiB per host and cache root; a refresh can temporarily hold both the old and new snapshots. Failed refreshes keep the last usable snapshot. Commands sharing a source cache wait for the active reader to finish before refreshing or removing files. Cleanup ignores symlinks and unrelated entries; it does not manage Nix, Cargo, or sccache caches.
