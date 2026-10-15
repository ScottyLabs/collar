# `collar secrets`

Global options apply to every command below; see the [command index](index.md).

```text
Check or update project secrets through secretspec

Usage: collar secrets <COMMAND>

Commands:
  check  Check required secrets without prompts or printing values
  set    Set a declared secret using a prompt or file/stdin input
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar secrets check --profile dev
  collar secrets set JWT_SECRET --profile dev
```

## `collar secrets check`

```text
Check required secrets without prompts or printing values

Usage: collar secrets check [OPTIONS]

Options:
      --profile <PROFILE>
          Secret profile; uses the underlying tool default when omitted

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason, forwarded unchanged to secretspec and devenv

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

      --scope <SCOPE>
          [env: SECRETSPEC_SCOPE]

      --json
          Emit secretspec's value-free JSON report

  -h, --help
          Print help
```

## `collar secrets set`

```text
Set a declared secret using a prompt or file/stdin input

Usage: collar secrets set [OPTIONS] <NAME>

Arguments:
  <NAME>


Options:
      --from-file <FROM_FILE>
          Read exact bytes from a file, or - for stdin; no value arguments

      --profile <PROFILE>
          Secret profile; uses the underlying tool default when omitted

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason, forwarded unchanged to secretspec and devenv

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

  -h, --help
          Print help
```
