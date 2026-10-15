# `collar doctor`

Global options apply to every command below; see the [command index](index.md).

```text
Diagnose local setup and ScottyLabs login readiness

Usage: collar doctor [OPTIONS]

Options:
      --offline
          Skip forge and Bao requests; never resolve secrets

      --governance <GOVERNANCE>
          [env: COLLAR_GOVERNANCE]

      --infrastructure <INFRASTRUCTURE>
          [env: COLLAR_INFRASTRUCTURE]

      --profile <PROFILE>
          Secretspec profile to inspect without reading its secret values

          [default: dev]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar doctor
  collar doctor --offline --json

Prints the full setup report before returning failure. Offline mode skips network probes and reads cached sources. Secret values are never resolved.
```
