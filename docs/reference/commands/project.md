# `collar project`

Global options apply to every command below; see the [command index](index.md).

```text
Connect the selected repository to its ScottyLabs project

Usage: collar project <COMMAND>

Commands:
  select  Pick a project repository interactively; emits its context
  view    Show declared ownership, features, and the secret namespace
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar project view -R ScottyLabs/collar
  collar project select
```

## `collar project select`

```text
Pick a project repository interactively; emits its context

Usage: collar project select [OPTIONS]

Options:
      --checkout <CHECKOUT>
          Inspect local governance changes instead of the latest canonical revision

          [env: COLLAR_GOVERNANCE]

      --offline


      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar project view`

```text
Show declared ownership, features, and the secret namespace

Usage: collar project view [OPTIONS]

Options:
      --checkout <CHECKOUT>
          Inspect local governance changes instead of the latest canonical revision

          [env: COLLAR_GOVERNANCE]

      --offline
          Read an existing snapshot without refreshing it

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
