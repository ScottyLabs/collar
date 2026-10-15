# `collar docs`

Global options apply to every command below; see the [command index](index.md).

```text
Read and search canonical ScottyLabs documentation

Usage: collar docs <COMMAND>

Commands:
  list    List available Markdown topics
  view    Read a Markdown topic
  search  Search Markdown topics for a literal phrase
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar docs search secrets
  collar docs list --service kennel
```

## `collar docs list`

```text
List available Markdown topics

Usage: collar docs list [OPTIONS]

Options:
      --service <SERVICE>
          Documentation service to read

          [default: scottylabs]
          [possible values: scottylabs, kennel, infrastructure, governance]

      --checkout <CHECKOUT>
          Read a local service checkout instead of a canonical snapshot

      --offline
          Read the last verified canonical snapshot without network access

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar docs view`

```text
Read a Markdown topic

Usage: collar docs view [OPTIONS] <TOPIC>

Arguments:
  <TOPIC>


Options:
      --service <SERVICE>
          Documentation service to read

          [default: scottylabs]
          [possible values: scottylabs, kennel, infrastructure, governance]

      --checkout <CHECKOUT>
          Read a local service checkout instead of a canonical snapshot

      --offline
          Read the last verified canonical snapshot without network access

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar docs search`

```text
Search Markdown topics for a literal phrase

Usage: collar docs search [OPTIONS] <QUERY>

Arguments:
  <QUERY>


Options:
      --service <SERVICE>
          Documentation service to read

          [default: scottylabs]
          [possible values: scottylabs, kennel, infrastructure, governance]

      --checkout <CHECKOUT>
          Read a local service checkout instead of a canonical snapshot

      --offline
          Read the last verified canonical snapshot without network access

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
