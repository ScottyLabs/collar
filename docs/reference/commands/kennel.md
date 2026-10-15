# `collar kennel`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect deployment reports from Forgejo

Usage: collar kennel <COMMAND>

Commands:
  preview  Find preview links reported in marked Kennel PR comments
  logs     Print Kennel build and deploy report links for a PR head
  wait     Wait for Kennel build and deployment statuses on the current PR head
  status   Show build/deploy reports for a PR and historical preview comments
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
Find preview links reported in marked Kennel PR comments

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
Print Kennel build and deploy report links for a PR head

Usage: collar kennel logs [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]


Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar kennel wait`

```text
Wait for Kennel build and deployment statuses on the current PR head

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
Show build/deploy reports for a PR and historical preview comments

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
