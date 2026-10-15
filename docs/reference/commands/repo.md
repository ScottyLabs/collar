# `collar repo`

Global options apply to every command below; see the [command index](index.md).

```text
Clone, inspect, or fork a repository

Usage: collar repo <COMMAND>

Commands:
  fork   Fork the selected repository without changing its settings
  clone  Clone using the repository's SSH URL and your existing Git authentication
  view   View a repository
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar repo view -R ScottyLabs/collar
  collar repo fork -R ScottyLabs/collar
```

## `collar repo fork`

```text
Fork the selected repository without changing its settings

Usage: collar repo fork [OPTIONS] [REPOSITORY]

Arguments:
  [REPOSITORY]
          Repository name or [HOST/]OWNER/REPO; defaults to the current repository

Options:
      --organization <ORGANIZATION>
          Fork into this organization instead of your account

      --name <NAME>
          Name of the fork; defaults to the source repository name

  -h, --help
          Print help
```

## `collar repo clone`

```text
Clone using the repository's SSH URL and your existing Git authentication

Usage: collar repo clone [OPTIONS] <REPOSITORY> [DIRECTORY]

Arguments:
  <REPOSITORY>


  [DIRECTORY]


Options:
      --https
          Use HTTPS instead of the default SSH transport

  -h, --help
          Print help
```

## `collar repo view`

```text
View a repository

Usage: collar repo view [OPTIONS] [REPOSITORY]

Arguments:
  [REPOSITORY]
          Repository name or [HOST/]OWNER/REPO; defaults to the current repository

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
