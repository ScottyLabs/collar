# `collar status`

Global options apply to every command below; see the [command index](index.md).

```text
Show your review requests, authored PRs, and assigned issues across the forge

Usage: collar status [OPTIONS]

Options:
      --owner <OWNER>
          Filter repositories by owner or organization

      --limit <LIMIT>
          Maximum recently updated items per category

          [default: 10]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar status
  collar status --owner ScottyLabs
  collar status --json reviewRequests

Requires sign-in. Shows up to --limit recently updated open items in each category. This is a host-wide inbox; use --owner to narrow it, rather than -R. CI checks remain available through pr checks and actions commands.
```
