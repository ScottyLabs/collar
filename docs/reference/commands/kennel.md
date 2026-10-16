# `collar kennel`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect Kennel deployment reports and retry branch builds

Usage: collar kennel <COMMAND>

Commands:
  retry    Retry a Kennel branch build through its verified Forgejo webhook
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

## `collar kennel retry`

```text
Retry a Kennel branch build through its verified Forgejo webhook

Usage: collar kennel retry [OPTIONS] [BRANCH]

Arguments:
  [BRANCH]
          Branch to retry; omitted selects the repository's default branch

Options:
      --pr <PR>
          PR selector; currently rejected because push tests cannot retry previews

      --hook <HOOK>
          Select a Kennel webhook when more than one is configured

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar kennel retry
  collar kennel retry staging
  collar kennel retry dev --hook 12

Omitting BRANCH selects the repository's default branch. Kennel push retries support main, staging, and dev. PR previews require a pull-request event and cannot be retried by Forgejo's push-test endpoint. Requires repository webhook administration permission. An accepted trigger is not proof of delivery or build start; never repeat an uncertain write automatically.
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
