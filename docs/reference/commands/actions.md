# `collar actions`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect Forgejo Actions runs

Usage: collar actions <COMMAND>

Commands:
  list    List workflow runs
  view    View one workflow run
  jobs    List jobs for a workflow run
  watch   Wait for a workflow run; fail if it does not succeed
  logs    Print job logs for a run, optionally only failed jobs
  delete  Delete a completed workflow run
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar actions list
  collar actions jobs 123
```

## `collar actions list`

```text
List workflow runs

Usage: collar actions list [OPTIONS]

Options:
      --limit <LIMIT>
          Maximum number of runs to return

          [default: 50]

      --status <STATUS>
          Filter by Forgejo Actions status

      --ref <REF>
          Filter by git ref, such as refs/heads/main

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar actions view`

```text
View one workflow run

Usage: collar actions view [OPTIONS] <ID>

Arguments:
  <ID>
          Forgejo workflow run ID

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar actions jobs`

```text
List jobs for a workflow run

Usage: collar actions jobs [OPTIONS] <ID>

Arguments:
  <ID>
          Forgejo workflow run ID

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar actions watch`

```text
Wait for a workflow run; fail if it does not succeed

Usage: collar actions watch [OPTIONS] <ID>

Arguments:
  <ID>
          Forgejo workflow run ID

Options:
      --interval <INTERVAL>
          Seconds between polls

          [default: 5]

      --timeout <TIMEOUT>
          Maximum seconds to wait, excluding an in-flight request

          [default: 600]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar actions logs`

```text
Print job logs for a run, optionally only failed jobs

Usage: collar actions logs [OPTIONS] <ID>

Arguments:
  <ID>
          Forgejo workflow run ID

Options:
      --failed
          Only include jobs with failure status

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar actions delete`

```text
Delete a completed workflow run

Usage: collar actions delete [OPTIONS] <ID>

Arguments:
  <ID>
          Forgejo workflow run ID

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
