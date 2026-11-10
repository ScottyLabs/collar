# `collar kennel`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect nixbot builds, Kennel deployments, and previews

Usage: collar kennel <COMMAND>

Commands:
  preview  Show the preview URLs Kennel serves for a PR
  logs     Print nixbot's statuses and failure logs and Kennel's deploy outcome for a PR head
  wait     Wait for the nixbot build and Kennel deployment on the current PR head
  status   Show nixbot's checks and Kennel's deploy and deployments for a PR
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar kennel preview 42
  collar kennel wait 42
```

## `collar kennel preview`

```text
Show the preview URLs Kennel serves for a PR

Usage: collar kennel preview [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]


Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar kennel logs`

```text
Print nixbot's statuses and failure logs and Kennel's deploy outcome for a PR head

Usage: collar kennel logs [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]


Options:
      --lines
          Read deployment log lines through authenticated Grafana instead of the report

      --since <SINCE>
          Minutes of deployment logs to query, up to the 30-day retention

          [default: 60]

      --tail <TAIL>
          Maximum log lines (per failed check in report mode)

          [default: 50]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar kennel wait`

```text
Wait for the nixbot build and Kennel deployment on the current PR head

Usage: collar kennel wait [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]


Options:
      --timeout <TIMEOUT>
          [default: 120]

      --interval <INTERVAL>
          [default: 2]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar kennel status`

```text
Show nixbot's checks and Kennel's deploy and deployments for a PR

Usage: collar kennel status [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
