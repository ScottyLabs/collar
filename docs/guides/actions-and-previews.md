# CI and previews

## Builds and deployments

| Task | Command |
| --- | --- |
| Show every check on a PR | `collar pr checks NUMBER` |
| Wait for the build and deployment | `collar kennel wait NUMBER` |
| Show why a nixbot check failed | `collar kennel logs NUMBER` |
| Show why an Actions job failed | `collar actions list`, then `collar actions logs RUN --failed` |
| Find the preview URLs | `collar kennel preview NUMBER` |

Kennel's [deploying](https://docs.kennel.scottylabs.org/guides/deploying/#4-push), [CI](https://docs.kennel.scottylabs.org/guides/ci/), and [preview](https://docs.kennel.scottylabs.org/guides/previews/) guides cover what runs on each push.

## Inspect Actions

```sh
collar actions list
collar actions list --ref refs/heads/main
collar actions view 123
collar actions jobs 123
collar actions watch 123
collar actions logs 123 --failed
```

Use the run ID shown by `actions list`. `--status` and `--limit` narrow the list.
Run and job reads use Forgejo's API; no local clone is required when `-R` selects
the repository.

`watch` prints progress on stderr and waits up to 600 seconds by default, polling
every five seconds. `--timeout` and `--interval` adjust those limits. It returns
failure for an unsuccessful run or a timeout. With `--json`, stdout contains only
the final result; a blocked run keeps waiting for manual approval until timeout.

`logs` prints job logs directly, using each job's reported attempt. `--failed`
selects failed jobs. `--json` returns job IDs, attempts, and log text. Each log
response is limited to 8 MiB. Treat CI logs as potentially sensitive before
sharing them or placing them in an agent's context.

## Dispatch a workflow

```sh
collar workflow list
collar workflow view ci.yml
collar workflow dispatch ci.yml --ref main --field mode=fast
```

Repeat `--field NAME=VALUE` for more inputs. Values are strings; `--field note=`
sends an empty value. The workflow must already exist and support dispatch.
`--return-run-info` requests details of the created run from Forgejo.

`actions delete ID` deletes a completed run and requires sign-in. It is an
explicit write, not a way to stop a running job.

## Find a Kennel preview

```sh
collar kennel status 42
collar kennel preview 42
collar kennel logs 42
collar kennel wait 42
```

These commands read nixbot's commit statuses and Kennel's [HTTP API](https://docs.kennel.scottylabs.org/reference/architecture/#http-api). Omit the PR number inside a checkout to use the current branch's PR.

`status` shows nixbot's checks, Kennel's deploy of the PR, and each deployment with its URL and live health. `preview` lists only the deployments. `logs` prints nixbot's statuses, each failed check's evaluation error or the last `--tail` lines of its build log, and the stage where a failed Kennel deploy stopped. `wait` polls until nixbot has built the PR head and Kennel has deployed it, with a timeout; see `collar kennel wait --help` for polling options.

A deployment that runs an older commit than the PR head is flagged, which happens while a new push is still building.
