# `collar dev`

Global options apply to every command below; see the [command index](index.md).

```text
Run project shells, commands, and local processes through nix develop

Usage: collar dev <COMMAND>

Commands:
  shell  Enter an interactive project shell
  run    Run a command inside the project shell, preserving literal arguments
  up     Start the project's local processes in the background with pitchfork
  down   Stop the project's local processes
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar dev shell
  collar dev run -- cargo test
  collar dev up
```

## `collar dev shell`

```text
Enter an interactive project shell

Usage: collar dev shell [OPTIONS]

Options:
      --profile <PROFILE>
          Secret profile; uses the underlying tool default when omitted

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason forwarded unchanged to secretspec

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

  -h, --help
          Print help
```

## `collar dev run`

```text
Run a command inside the project shell, preserving literal arguments

Usage: collar dev run [OPTIONS] <COMMAND>...

Arguments:
  <COMMAND>...


Options:
      --profile <PROFILE>
          Secret profile; uses the underlying tool default when omitted

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason forwarded unchanged to secretspec

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

  -h, --help
          Print help
```

## `collar dev up`

```text
Start the project's local processes in the background with pitchfork

Usage: collar dev up [OPTIONS]

Options:
      --profile <PROFILE>
          Secret profile; uses the underlying tool default when omitted

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason forwarded unchanged to secretspec

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

  -h, --help
          Print help
```

## `collar dev down`

```text
Stop the project's local processes

Usage: collar dev down [OPTIONS]

Options:
      --profile <PROFILE>
          Secret profile; uses the underlying tool default when omitted

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason forwarded unchanged to secretspec

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

  -h, --help
          Print help
```
