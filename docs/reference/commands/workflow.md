# `collar workflow`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect or dispatch existing workflows

Usage: collar workflow <COMMAND>

Commands:
  list      List workflow files in .forgejo/workflows and .github/workflows
  view      View workflow file metadata returned by Forgejo
  dispatch  Dispatch an existing workflow with optional NAME=VALUE inputs
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar workflow dispatch ci.yml --ref main --field mode=fast
```

## `collar workflow list`

```text
List workflow files in .forgejo/workflows and .github/workflows

Usage: collar workflow list [OPTIONS]

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar workflow view`

```text
View workflow file metadata returned by Forgejo

Usage: collar workflow view [OPTIONS] <WORKFLOW>

Arguments:
  <WORKFLOW>
          Path or filename under .forgejo/workflows or .github/workflows

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar workflow dispatch`

```text
Dispatch an existing workflow with optional NAME=VALUE inputs

Usage: collar workflow dispatch [OPTIONS] --ref <REF> <WORKFLOW>

Arguments:
  <WORKFLOW>
          Workflow filename or path under .forgejo/workflows or .github/workflows

Options:
      --ref <REF>
          Git ref to dispatch

  -f, --field <NAME=VALUE>
          Workflow input as NAME=VALUE; repeat for multiple inputs

      --return-run-info
          Include the created run information in Forgejo's response

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```
