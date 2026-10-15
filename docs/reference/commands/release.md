# `collar release`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect and manage repository releases

Usage: collar release <COMMAND>

Commands:
  list    List releases, newest first
  view    View one release by tag
  create  Create a release for an existing tag
  edit    Edit a release by tag
  delete  Delete a release by tag
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar release list
  collar release view v0.1.0
```

## `collar release list`

```text
List releases, newest first

Usage: collar release list [OPTIONS]

Options:
      --limit <LIMIT>
          [default: 30]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar release view`

```text
View one release by tag

Usage: collar release view [OPTIONS] <TAG>

Arguments:
  <TAG>


Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar release create`

```text
Create a release for an existing tag

Usage: collar release create [OPTIONS] <TAG>

Arguments:
  <TAG>


Options:
      --title <TITLE>
          Release title; defaults to the tag on creation

      --notes <NOTES>
          Release notes as Markdown text

      --notes-file <NOTES_FILE>
          Read Markdown notes from a file, or - for stdin

      --draft
          Save as a draft without publishing

      --prerelease
          Mark the release as a prerelease

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar release edit`

```text
Edit a release by tag

Usage: collar release edit [OPTIONS] <TAG>

Arguments:
  <TAG>


Options:
      --title <TITLE>
          Release title; defaults to the tag on creation

      --notes <NOTES>
          Release notes as Markdown text

      --notes-file <NOTES_FILE>
          Read Markdown notes from a file, or - for stdin

      --draft
          Convert to a draft

      --publish
          Publish a draft release

      --prerelease
          Mark the release as a prerelease

      --stable
          Remove the prerelease flag

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar release delete`

```text
Delete a release by tag

Usage: collar release delete <TAG>

Arguments:
  <TAG>


Options:
  -h, --help
          Print help
```
