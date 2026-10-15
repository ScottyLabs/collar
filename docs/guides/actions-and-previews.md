# CI and previews

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

Kennel commands inspect commit statuses and marked PR comments. Omit the PR
number inside a checkout to use the current branch's PR.

`preview` finds preview URLs reported by Kennel. `logs` prints build and deploy
report links. `wait` polls checks for the inspected PR head with a timeout;
inspect `collar kennel wait --help` for polling options.

Report links and historical comments are not live service-health checks.
Check the reported commit when a PR has received new pushes.
