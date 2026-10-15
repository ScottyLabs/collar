# `collar api`

Global options apply to every command below; see the [command index](index.md).

```text
Send a request to this forge's REST API

Usage: collar api [OPTIONS] <ENDPOINT>

Arguments:
  <ENDPOINT>


Options:
  -X, --method <METHOD>


  -f, --raw-field <RAW_FIELDS>
          Add a string field; fields imply POST unless --method is given

  -F, --field <FIELDS>
          Add a typed JSON field; @file and @- read a string from a file or stdin

      --input <INPUT>
          Read a JSON request body from a file, or - for stdin

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar api user --method GET
  collar api repos/ScottyLabs/collar/pulls

Endpoints are relative to /api/v1 on the selected forge. GET is the default; fields imply POST unless --method is set. This command can write and requires the same authorization as the underlying operation.
```
