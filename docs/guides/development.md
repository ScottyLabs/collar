# Local development

These commands run in the current project's Git root. They do not fetch
governance or require secretspec configuration before starting a tool.

## Diagnose setup

```sh
collar doctor
collar doctor --offline
```

Doctor checks tools, Git identity, SSH-agent keys, the project flake, declared project
ownership, secretspec configuration, infrastructure sources, and authentication.
It prints the whole report before returning a failure status. Checks can pass,
fail, require a manual step, or be skipped.

Online checks include Forgejo and OpenBao token verification. Offline mode uses
cached canonical sources and skips network probes. Doctor never resolves secret
values or applies infrastructure. It checks configuration and access prerequisites;
it does not prove deployment health or access to every project secret.

Follow the suggested actions for failed checks. Doctor can't verify that your accounts are linked in Keycloak or that your team membership has been applied, so it reports that check as manual. Linking accounts is the first step of governance's [joining a team](https://git.cmu.dev/ScottyLabs/governance#joining-a-team).

## Start a shell or process

```sh
collar dev shell
collar dev run -- cargo test
collar dev up
collar dev down
```

Collar runs commands through `nix develop` in the project root and preserves tool output
and exit codes. Arguments after `dev run --` are passed literally, without a
shell expansion step. `dev shell` requires a terminal; scripts should use `dev run`.
`dev up` starts the project's local processes in the background with `pitchfork start --local`, and `dev down` stops them with `pitchfork stop --local`.
Kennel's [local development guide](https://docs.kennel.scottylabs.org/guides/local-development/) describes the shell, the local services, and the processes a project declares.

## Run checks

```sh
collar check
collar check --all
collar check treefmt --files src/main.rs
```

`check` runs prek inside the development shell. It uses staged files by default;
`--all` selects all files. File paths for `--files` are relative to your current
directory or absolute and must belong to the project checkout.
The hooks are the project's, described in Kennel's [git hooks](https://docs.kennel.scottylabs.org/guides/local-development/#git-hooks) section.

## Authenticate to OpenBao

```sh
collar auth bao login
collar auth bao status
```

OpenBao login uses university OIDC and works before a project shell is available:
Nix supplies the login tool. Token printing is disabled. Status verifies the
token without printing its metadata; it does not check every project permission.
Kennel's [secrets guide](https://docs.kennel.scottylabs.org/guides/secrets/) explains how projects declare secrets and which profiles exist.

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
