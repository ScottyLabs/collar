# Pull requests

## Find and inspect a PR

```sh
collar pr list
collar pr view 42
collar pr view 42 --comments
collar pr diff 42
collar pr checks 42
```

PR selectors accept a number, a URL in the selected repository, or a branch
name. Omit the selector inside a checkout to find the open PR for the current
branch. If more than one PR matches, select a number explicitly. Pass `-R OWNER/REPO` for remote work without a clone.

`pr view --comments` includes discussion comments, submitted reviews, and inline
review comments. Use `--json` to read the same conversation as structured data.

Filter lists without a local checkout:

```sh
collar pr list -R ScottyLabs/collar --author @me
collar pr list -R ScottyLabs/collar --base main --head feature/health
```

`@me` resolves the signed-in Forgejo account and requires authentication. Other
author logins and branch filters are sent directly to Forgejo.

`pr checks` reports commit statuses and fails when none exist or any are not
successful. It does not determine whether every required check has reported;
Forgejo enforces merge requirements.

For a terminal picker, use `collar pr browse`. Type to filter, choose a PR, then
inspect details, its diff, or checks. Escape cancels. This is read-only and
requires terminal input and output.

## Request reviewers

```sh
collar pr reviewers 42
collar pr reviewers 42 --add anish kritdass
collar pr reviewers 42 --remove anish
```

With no changes, this lists requested users and teams. `--add` and `--remove`
write immediately, require sign-in and an open PR, and cannot be combined.
Usernames are supplied directly; no files or local clones are needed. Team
requests are displayed but are not modified by these options. An uncertain
failure after a write requires inspecting the list before retrying.

## Submit a review

```sh
collar pr review 42 --approve
collar pr review 42 --request-changes --body 'Please cover the timeout case.'
collar pr review 42 --comment --body 'The migration looks good.'
```

Choose exactly one verdict. Comments and change requests need text or inline
comments. A review writes immediately and requires sign-in.

Add an inline comment directly with `PATH LINE SIDE BODY`; repeat the option for
more comments:

```sh
collar pr review 42 --request-changes \
  --inline-comment src/main.rs 12 new 'Return an error here instead of panicking.'
```

Use `new` for the PR's version and `old` for the base version. Lines must occur
in the diff. For renamed files, use the new path on either side. Collar checks
line anchors and verifies the PR's head before submitting one review pinned to
that commit. If the head changes, inspect the new diff before reviewing again.

For longer text, `--body-file review.md` reads a Markdown file and `--body-file -`
reads stdin. A file is optional; use `--body` for short messages.

## Create and update a PR

Push the branch with Git first. Then:

```sh
collar pr create --title 'Handle preview timeouts' --body 'Return an actionable error.'
collar pr edit 42 --title 'Report preview timeouts'
collar pr comment 42 --body 'Ready for another look.'
collar pr close 42
collar pr reopen 42
```

Creation defaults to the current branch and the repository's default target
branch. `--head owner:branch` selects a fork branch; `--base main` selects a target.
Collar does not push commits. An edit sends only supplied fields; `--body ''`
clears the description.

## Check out a PR

```sh
collar pr checkout 42
```

This is a local Git operation. Collar fetches the PR head through `upstream`,
then `origin`, and switches to `pr/42`. Use `--branch NAME` or `--remote NAME` to
override those choices. Git handles SSH or HTTPS authentication.

Commit or stash local changes first, including untracked files and changed
submodules. Collar verifies the fetched commit before switching branches and
never resets an existing branch to a different commit.

## Update or merge

```sh
collar pr update-branch 42
collar pr merge 42 --squash
```

`update-branch` merges the target into the PR branch; `--rebase` requests a
rebase. Its optional `--match-head-commit SHA` checks the head before submission,
but Forgejo does not make that check atomic with the update.

Merge requires one of `--merge`, `--squash`, or `--rebase`. Immediate merging
checks the inspected head on the server. Use `--delete-branch` to request source
branch deletion and `--match-head-commit SHA` to require a specific head.

`--auto` schedules a merge when server checks pass. It can merge a later head
and cannot be combined with `--match-head-commit`. A successful response confirms
the request was accepted; inspect the PR to determine whether it has merged.

If a write fails after submission, inspect the PR before retrying. Collar does
not retry writes automatically.
