# Local development

These commands run in the current project's Git root. They do not fetch
governance or require secretspec configuration before starting a tool.

## Diagnose setup

```sh
collar doctor
collar doctor --offline
```

Doctor checks tools, Git identity, SSH-agent keys, devenv files, declared project
ownership, secretspec configuration, infrastructure sources, and authentication.
It prints the whole report before returning a failure status. Checks can pass,
fail, require a manual step, or be skipped.

Online checks include Forgejo and OpenBao token verification. Offline mode uses
cached canonical sources and skips network probes. Doctor never resolves secret
values or applies infrastructure. It checks configuration and access prerequisites;
it does not prove deployment health or access to every project secret.

Follow the suggested actions for failed checks. Account linking is manual:
[link your accounts in Keycloak](https://idp.scottylabs.org/realms/scottylabs/account/account-security/linked-accounts).

## Start a shell or process

```sh
collar dev shell
collar dev run -- cargo test
collar dev up
collar dev up --detach
collar dev task security:
collar dev down
```

Collar delegates to the project's devenv configuration and preserves tool output
and exit codes. Arguments after `dev run --` are passed literally, without a
shell expansion step. `dev shell` requires a terminal; scripts should use `dev run`.

## Run checks

```sh
collar check
collar check --all
collar check treefmt --files src/main.rs
```

`check` runs prek inside the development shell. It uses staged files by default;
`--all` selects all files. File paths for `--files` are relative to your current
directory or absolute and must belong to the project checkout.

## Authenticate to OpenBao

```sh
collar auth bao login
collar auth bao status
```

OpenBao login uses university OIDC and works before a project shell is available:
Nix supplies the login tool. Token printing is disabled. Status verifies the
token without printing its metadata; it does not check every project permission.

## Check or set secrets

```sh
collar secrets check --profile dev
collar secrets check --profile prod --scope build --json
collar secrets set JWT_SECRET --profile dev
```

Secretspec owns validation of profiles, providers, scopes, and declared secret
names. Checks do not prompt for missing values and use secretspec's value-free
report. Setting a value requires a terminal prompt or explicit file/stdin input:

```sh
collar secrets set JWT_SECRET --profile dev --from-file ./private-secret-file
```

Use `--from-file -` to read a value from stdin. Keep secret values out of command
arguments, shell history, logs, and agent conversations.

`--profile` honors `SECRETSPEC_PROFILE`; when neither is set, the underlying tool
uses its configured default. `--provider` selects a secretspec provider.
`--reason` or `SECRETSPEC_REASON` forwards an access reason unchanged. Provider
policy still applies. Secrets commands require secretspec 0.21 or newer.
