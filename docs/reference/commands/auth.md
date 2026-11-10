# `collar auth`

Global options apply to every command below; see the [command index](index.md).

```text
Sign in to Forgejo or OpenBao and check saved credentials

Usage: collar auth <COMMAND>

Commands:
  login   Sign in through your browser and save the verified credential
  logout  Revoke this host's SSO session and remove its saved credentials
  bao     Sign in to OpenBao for project secrets; separate from Forgejo login
  status  Verify Forgejo access and Grafana access for a saved SSO session
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar auth login
  collar auth status
  collar auth bao login
```

## `collar auth login`

```text
Sign in through your browser and save the verified credential

Usage: collar auth login [OPTIONS]

Options:
      --with-token
          Read a personal access token from stdin instead of opening a browser

      --token
          Use a hidden personal-access-token prompt instead of browser SSO

      --web
          Start a fresh browser sign-in instead of reusing the saved SSO session

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar auth login
  collar auth login --token

Browser sign-in is the default; uses Keycloak and the Forgejo broker. Git SSH authentication is separate. Headless sessions can use COLLAR_HOST and COLLAR_TOKEN supplied through a secret environment.
```

## `collar auth logout`

```text
Revoke this host's SSO session and remove its saved credentials

Usage: collar auth logout

Options:
  -h, --help
          Print help
```

## `collar auth bao`

```text
Sign in to OpenBao for project secrets; separate from Forgejo login

Usage: collar auth bao <COMMAND>

Commands:
  login   Authenticate with ScottyLabs OIDC before entering a project shell
  status  Verify the current token without displaying it
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help
```

### `collar auth bao login`

```text
Authenticate with ScottyLabs OIDC before entering a project shell

Usage: collar auth bao login

Options:
  -h, --help
          Print help
```

### `collar auth bao status`

```text
Verify the current token without displaying it

Usage: collar auth bao status

Options:
  -h, --help
          Print help
```

## `collar auth status`

```text
Verify Forgejo access and Grafana access for a saved SSO session

Usage: collar auth status

Options:
  -h, --help
          Print help
```
