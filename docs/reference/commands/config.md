# `collar config`

Global options apply to every command below; see the [command index](index.md).

```text
Manage nonsecret user defaults

Usage: collar config <COMMAND>

Commands:
  get    Show saved nonsecret settings and their file location
  set    Save the default forge host
  unset  Remove a saved default
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar config get
  collar config set host git.cmu.dev
```

## `collar config get`

```text
Show saved nonsecret settings and their file location

Usage: collar config get [OPTIONS]

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar config set`

```text
Save the default forge host

Usage: collar config set <KEY> <VALUE>

Arguments:
  <KEY>
          [possible values: host]

  <VALUE>


Options:
  -h, --help
          Print help
```

## `collar config unset`

```text
Remove a saved default

Usage: collar config unset <KEY>

Arguments:
  <KEY>
          [possible values: host]

Options:
  -h, --help
          Print help
```
