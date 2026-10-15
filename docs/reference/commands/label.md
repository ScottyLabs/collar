# `collar label`

Global options apply to every command below; see the [command index](index.md).

```text
Manage repository labels

Usage: collar label <COMMAND>

Commands:
  list    List repository labels
  create  Create a repository label
  edit    Edit a repository label by ID
  delete  Delete a repository label by ID
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar label list
  collar label create --name bug --color ff0000
```

## `collar label list`

```text
List repository labels

Usage: collar label list [OPTIONS]

Options:
      --limit <LIMIT>
          [default: 30]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar label create`

```text
Create a repository label

Usage: collar label create [OPTIONS] --name <NAME> --color <COLOR>

Options:
      --name <NAME>


      --color <COLOR>


      --description <DESCRIPTION>


      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar label edit`

```text
Edit a repository label by ID

Usage: collar label edit [OPTIONS] <ID>

Arguments:
  <ID>


Options:
      --name <NAME>


      --color <COLOR>


      --description <DESCRIPTION>


      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar label delete`

```text
Delete a repository label by ID

Usage: collar label delete <ID>

Arguments:
  <ID>


Options:
  -h, --help
          Print help
```
