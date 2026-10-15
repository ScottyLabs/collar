# Troubleshooting

Start with `collar doctor` inside the affected project. Use `--offline` when
network access is unavailable, then follow the failed check's suggested action.

## API reads work, but a review asks for login

Public API reads do not require authentication. Writes need the credential from
`collar auth login` or a host-bound `COLLAR_TOKEN`. Git's SSH key does not
authenticate API requests.

## Login opens a browser but cannot save the credential

On Linux, unlock Secret Service and make sure the session bus is available.
Headless environments can use a token supplied through `COLLAR_TOKEN` and
`COLLAR_HOST`. Collar does not fall back to saving plaintext tokens.

If the browser helper itself fails, confirm `git-credential-oauth` is on `PATH`.
Use the Nix package or Collar's development shell to get the helper. Browser
login needs Forgejo's pre-registered helper application and a local browser for
its callback.

## A saved credential has expired

Run `collar auth login` again, then `collar auth status`. Other commands do not
start login automatically. For an environment token, update the runner's secret
and check the host binding.

## Repository or host selection fails

Run `git remote -v` and check `upstream` and `origin`, or pass `-R OWNER/REPO`.
An explicit `--host` must match the selected repository. Select the canonical
upstream when inspecting governance for a fork.

## The current branch has no matching PR

Pass the PR number or URL. Collar cannot infer a PR from an unpushed branch, and
multiple matching PRs need an explicit selector.

## PR checkout refuses to switch branches

Commit or stash tracked changes, untracked files, and changed submodules. Inspect
an existing `pr/NUMBER` branch before reusing its name; Collar will not reset it
to a different commit. A fetched head that differs from the inspected PR needs a
fresh inspection before checkout.

## Offline data is missing or corrupt

Run the read command once without `--offline` to download and verify the latest
snapshot. Offline mode does not download missing content. Online failures do not
silently fall back to cached data.

## A local wrapper reports a tool failure

Collar forwards devenv, prek, and secretspec output and exit status. Fix the
reported project configuration or tool error. Local development commands do not
need a governance clone or a project secret namespace merely to start.

## A write timed out

Inspect the affected PR, issue, release, or workflow before retrying. The server
may already have applied the request.
