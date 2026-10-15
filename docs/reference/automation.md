# Automation

## Request structured output

Use JSON instead of parsing terminal tables:

```sh
collar pr list -R ScottyLabs/collar --json number,title,headRefOid
collar project view -R ScottyLabs/collar --json
```

`--json` returns every supported field; `--json field1,field2` selects fields.
Field names are exact and comma-separated. An unsupported field fails with the
available names. List commands emit arrays and object views emit objects.
Canonical source results include `dataSource` when requested or when returning
all fields. The raw `api` command prints the server response rather than applying
this projection.

Structured results go to stdout. Errors and terminal prompts go to stderr.
Interactive pickers require terminal stdin and stderr; scripts should select
explicit repositories and resources. `--no-config` ignores saved settings and
logins for a reproducible run. It does not clear environment variables.

## Exit statuses

| Code | Meaning |
| --- | --- |
| `0` | The command succeeded |
| `1` | An operational error or a failed health/check result |
| `2` | Invalid command syntax or arguments |
| `130` | An interactive selection was cancelled |

Wrapped tools preserve their exit status, so devenv, prek, and secretspec can
return codes beyond this table. SIGPIPE uses normal Unix behavior when a
consumer closes a pipe.

`pr checks` fails for missing or unsuccessful statuses. `doctor` prints all
checks before returning failure. A successful write means the API accepted the
request; scheduled operations may not have finished.

## Use the API escape hatch

```sh
collar api user
collar api repos/ScottyLabs/collar/pulls
```

Endpoints are relative to the selected forge's `/api/v1/`. Absolute URLs are
rejected. GET is the default. Supplying fields implies POST unless `--method`
selects another method. `-f NAME=VALUE` sends a string; `-F NAME=VALUE` parses a
JSON value when possible. `--input PATH` or `--input -` accepts a JSON body.

The raw API command can write. Prefer a typed Collar command when one exists and
use explicit `--method GET` for reads in agent workflows. Repository
provisioning and settings still belong in governance, regardless of the raw API's
capabilities.

## Handle uncertain writes

Collar never retries API writes automatically. A timeout can happen after the
server applied a change. Inspect the resource before retrying to avoid duplicate
comments, reviews, or releases.

For PR reviews, inspect the diff and current head before submitting. Immediate
merges use an expected-head guard; scheduled merges may accept a later head.
See [pull request behavior](../guides/pull-requests.md#update-or-merge).
