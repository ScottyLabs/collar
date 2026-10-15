# `collar governance`

Global options apply to every command below; see the [command index](index.md).

```text
Discover declared ownership and service access without a clone

Usage: collar governance <COMMAND>

Commands:
  service-access  Show declared service access and the platform entry points
  project         List governed projects or inspect one by name
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar governance project list
  collar governance project view collar
```

## `collar governance service-access`

```text
Show declared service access and the platform entry points

Usage: collar governance service-access [OPTIONS]

Options:
      --checkout <CHECKOUT>
          Read a supplied governance checkout instead of the canonical snapshot

          [env: COLLAR_GOVERNANCE]

      --offline
          Read the cached canonical governance snapshot without refreshing it

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar governance project`

```text
List governed projects or inspect one by name

Usage: collar governance project <COMMAND>

Commands:
  list  List repositories with their owning projects and teams
  view  Inspect a project selected by slug or repository name
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help
```

### `collar governance project list`

```text
List repositories with their owning projects and teams

Usage: collar governance project list [OPTIONS]

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

### `collar governance project view`

```text
Inspect a project selected by slug or repository name

Usage: collar governance project view [OPTIONS] <NAME>

Arguments:
  <NAME>
          Project slug or repository name

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
