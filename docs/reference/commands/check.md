# `collar check`

Global options apply to every command below; see the [command index](index.md).

```text
Run repository hooks inside the project's development shell

Usage: collar check [OPTIONS] [HOOK]

Arguments:
  [HOOK]
          Run only this hook

Options:
      --all
          Run hooks for all files instead of only staged files

      --files <FILES>...
          Existing files relative to the calling directory, or absolute paths

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
          Print help (see a summary with '-h')

Examples:
  collar check
  collar check --all
  collar check treefmt --files src/main.rs

Runs prek in the project development shell. No governance or secrets preflight is required; tool output and exit status are preserved.
```
