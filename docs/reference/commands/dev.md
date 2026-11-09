# `collar dev`

Global options apply to every command below; see the [command index](index.md).

```text
Run project shells, commands, and local processes through nix develop

Usage: collar dev <COMMAND>

Commands:
  shell    Enter an interactive project shell
  run      Run a command inside the project shell, preserving literal arguments
  up       Start local processes and wait for readiness
  down     Stop local processes in reverse dependency order
  restart  Restart local processes through Pitchfork
  status   List this project's processes or inspect one process
  logs     Read a local process's logs
  help     Print this message or the help of the given subcommand(s)

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
          Secret profile; the shared project shell defaults to dev

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
          Secret profile; the shared project shell defaults to dev

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
Start local processes through Pitchfork, waiting for readiness and dependencies. Omit names to start every project process. Shell secret options do not override profiles selected by process scripts.

Usage: collar dev up [OPTIONS] [NAMES]...

Arguments:
  [NAMES]...
          Project process names; omit to select every local process

Options:
      --profile <PROFILE>
          Secret profile; the shared project shell defaults to dev

          [env: SECRETSPEC_PROFILE]

      --reason <REASON>
          Audit reason forwarded unchanged to secretspec

          [env: SECRETSPEC_REASON]

      --provider <PROVIDER>
          Select a provider alias declared in secretspec.toml

          [env: SECRETSPEC_PROVIDER]

  -h, --help
          Print help (see a summary with '-h')
```

## `collar dev down`

```text
Stop local processes in reverse dependency order

Usage: collar dev down [NAMES]...

Arguments:
  [NAMES]...
          Project process names; omit to select every local process

Options:
  -h, --help
          Print help
```

## `collar dev restart`

```text
Restart local processes through Pitchfork

Usage: collar dev restart [OPTIONS] [NAMES]...

Arguments:
  [NAMES]...
          Project process names; omit to select every local process

Options:
      --profile <PROFILE>
          Secret profile; the shared project shell defaults to dev

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

## `collar dev status`

```text
List this project's processes or inspect one process

Usage: collar dev status [OPTIONS] [NAME]

Arguments:
  [NAME]


Options:
      --json
          Emit Pitchfork's native JSON

  -h, --help
          Print help
```

## `collar dev logs`

```text
Read a local process's logs

Usage: collar dev logs [OPTIONS] <NAME>

Arguments:
  <NAME>


Options:
      --tail <TAIL>
          Maximum log lines to read before following, if requested

          [default: 50]

      --follow
          Follow new output until interrupted

      --json
          Emit Pitchfork's native JSON

  -h, --help
          Print help
```
