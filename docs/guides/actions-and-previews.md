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

Kennel inspection commands read commit statuses and marked PR comments. Omit the PR
number inside a checkout to use the current branch's PR.

`preview` finds preview URLs reported by Kennel. `logs` prints build and deploy
report links. `wait` polls checks for the inspected PR head with a timeout;
inspect `collar kennel wait --help` for polling options.

Report links and historical comments are not live service-health checks.
Check the reported commit when a PR has received new pushes.

## Retry a Kennel branch build

```sh
collar kennel retry
collar kennel retry staging -R terrier
collar kennel retry dev --hook 7 --json
```

Omitting the branch selects the repository's default branch. Kennel accepts push
retries only for `main`, `staging`, and `dev`; other branches are rejected without
sending a trigger. Retrying requeues a failed or cancelled build at the same
commit. Kennel does not rebuild an already successful or in-progress build.

This command requires repository webhook administrator permission and Forgejo
16 or newer. Collar discovers the canonical
`https://kennel.scottylabs.org/webhook` destination, including namespace queries,
and verifies an active Forgejo webhook with JSON payloads and push events.
Missing hooks require enabling Kennel through governance. Multiple hooks require
`--hook ID`; that option still validates the destination and configuration.
Custom destinations and branch filters Collar cannot verify are rejected.

Collar checks the branch and webhook again before sending exactly one webhook
test with the requested branch ref. It never creates commits or reruns Actions.
An accepted trigger means Forgejo queued a delivery, not that Kennel received it
or started a build. Inspect the webhook delivery in Forgejo and Kennel's status
reports before retrying after a timeout or server error. Collar never retries a
write automatically.

Forgejo uses one `ref` for both commit resolution and the push payload's branch;
it cannot atomically pin a branch and expected SHA. Collar checks the branch
before and after the trigger, but even unchanged observations cannot prove the
payload revision. JSON therefore reports `triggerAccepted: true` while
`revisionVerified`, `deliveryConfirmed`, and `buildStartConfirmed` remain false.
`headUnchanged` is null if the final read failed. A changed or unreadable head
returns an error after reporting acceptance; do not blindly repeat the command.
The API returns no delivery ID or delivery result, so later delivery failures
cannot be detected by this command.

`--pr NUMBER` resolves the PR head, then explains why no trigger can be sent:
Forgejo webhook tests always send push events, while Kennel previews require a
pull-request event. A push test cannot reliably retry a preview, including one
from a fork. Redeliver the matching pull-request event through Forgejo's delivery
UI instead. Native PR retry and stronger revision/build verification need an
authenticated Kennel retry endpoint; branch retry needs no Kennel change.

These limits follow [Forgejo's webhook test implementation](https://codeberg.org/forgejo/forgejo/src/tag/v16.0.5/routers/api/v1/repo/hook.go)
and [Kennel's webhook handler](https://git.cmu.dev/ScottyLabs/kennel/src/branch/main/crates/kennel/src/handlers/webhook.rs).
