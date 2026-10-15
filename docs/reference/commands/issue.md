# `collar issue`

Global options apply to every command below; see the [command index](index.md).

```text
Work with Forgejo issues

Usage: collar issue <COMMAND>

Commands:
  list     List issues (pull requests are omitted)
  view     View an issue by number
  create   Create an issue
  edit     Edit an issue
  comment  Add a comment to an issue
  close    Close an issue
  reopen   Reopen an issue
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar issue list
  collar issue create --title 'Preview times out' --body 'Steps to reproduce...'
```

## `collar issue list`

```text
List issues (pull requests are omitted)

Usage: collar issue list [OPTIONS]

Options:
      --state <STATE>
          [default: open]
          [possible values: open, closed, all]

      --limit <LIMIT>
          [default: 30]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar issue view`

```text
View an issue by number

Usage: collar issue view [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>


Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar issue create`

```text
Create an issue

Usage: collar issue create [OPTIONS] --title <TITLE>

Options:
      --title <TITLE>


      --body <BODY>
          Exact body text

      --body-file <PATH>
          Read exact body text from a file, or stdin when set to -

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar issue edit`

```text
Edit an issue

Usage: collar issue edit [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>


Options:
      --title <TITLE>


      --body <BODY>
          Exact body text

      --body-file <PATH>
          Read exact body text from a file, or stdin when set to -

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar issue comment`

```text
Add a comment to an issue

Usage: collar issue comment [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>


Options:
      --body <BODY>
          Exact body text

      --body-file <PATH>
          Read exact body text from a file, or stdin when set to -

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar issue close`

```text
Close an issue

Usage: collar issue close [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>


Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar issue reopen`

```text
Reopen an issue

Usage: collar issue reopen [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>


Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
