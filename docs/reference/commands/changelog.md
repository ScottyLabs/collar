# `collar changelog`

Global options apply to every command below; see the [command index](index.md).

```text
Read ScottyLabs changelogs and tech-lead announcements

Usage: collar changelog <COMMAND>

Commands:
  view     Read the canonical generated summary for a DevOps repository
  updates  List numbered tech-lead announcements, newest first
  update   Read a numbered tech-lead announcement
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar changelog view kennel
  collar changelog updates
```

## `collar changelog view`

```text
Read the canonical generated summary for a DevOps repository

Usage: collar changelog view [OPTIONS] <REPOSITORY>

Arguments:
  <REPOSITORY>
          [possible values: kennel, governance, infrastructure]

Options:
      --checkout <CHECKOUT>
          Inspect local changes instead of the latest canonical snapshot

      --offline
          Read the last verified snapshot without network access

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar changelog updates`

```text
List numbered tech-lead announcements, newest first

Usage: collar changelog updates [OPTIONS]

Options:
      --checkout <CHECKOUT>
          Inspect local changes instead of the latest canonical snapshot

      --offline
          Read the last verified snapshot without network access

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar changelog update`

```text
Read a numbered tech-lead announcement

Usage: collar changelog update [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>


Options:
      --checkout <CHECKOUT>
          Inspect local changes instead of the latest canonical snapshot

      --offline
          Read the last verified snapshot without network access

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
