# `collar rfc`

Global options apply to every command below; see the [command index](index.md).

```text
Search RFCs or create a draft

Usage: collar rfc [OPTIONS] [QUERY]...

Arguments:
  [QUERY]...
          Words to search for

Options:
      --directory <DIRECTORY>
          Directory containing RFC Markdown files

      --status <STATUS>
          Status to include, or 'all' for every status

          [default: accepted]

      --new <TITLE>
          Create a new Draft RFC with this title instead of searching, and print its path

  -h, --help
          Print help

All query words must match within a section or its RFC title.
With no query, list RFCs. Search ignores case.
Matches are ranked by relevance (title hits, then most matching sections) before falling back to filename order.
```
