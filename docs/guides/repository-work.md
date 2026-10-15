# Repository work

## Clone or fork

```sh
collar repo clone ScottyLabs/collar
collar repo view collar
collar repo fork collar
```

Bare names such as `terrier` resolve to `ScottyLabs/terrier`. Specify
`OWNER/REPO` for another owner.

Omit the repository for `repo view`, `repo fork`, or `project view` to use the
current checkout. `-R` remains available for commands such as `pr view 42`, where
the positional argument identifies a different resource.

Cloning uses SSH by default; `--https` selects HTTPS. Forking creates a copy in
your account without changing the source repository. `--organization` and
`--name` select a destination organization and name, subject to Forgejo permissions.

ScottyLabs repository settings and provisioning are declared in governance.
Collar does not expose repository creation, metadata editing, archiving,
deletion, or mirror administration.

## Work with issues

```sh
collar issue list
collar issue view 12
collar issue create --title 'Preview times out' --body 'Steps to reproduce...'
collar issue comment 12 --body 'Reproduced on the current branch.'
collar issue close 12
collar issue reopen 12
```

Issue lists omit pull requests. `issue edit` changes supplied title/body fields;
omitted fields remain unchanged. An empty body clears it. Long descriptions can
use `--body-file PATH` or `--body-file -` for stdin.

## Maintain labels

```sh
collar label list
collar label create --name bug --color ff0000
collar label edit 7 --description 'Unexpected behavior'
collar label delete 7
```

Editing and deleting use the label ID from `label list`. Label colors use six
hexadecimal digits, optionally prefixed by `#`.

## Publish release metadata

```sh
collar release list
collar release view v0.1.0
collar release create v0.1.0 --title 'Version 0.1.0' --notes 'Release notes.' --draft
collar release edit v0.1.0 --publish
```

The Git tag must already exist. These commands manage release metadata, not Git
tags or uploaded assets. `--notes-file PATH` or `--notes-file -` supports longer
notes. `release delete TAG` deletes the release while leaving the tag in place.

All writes require sign-in and take effect immediately. Forgejo enforces
permissions. Inspect the affected resource after an uncertain failure before
retrying.

## Search or draft an RFC

Inside a project with an `rfcs/` directory:

```sh
collar rfc authentication
collar rfc --status all deployment
collar rfc --new 'Improve preview health checks'
```

Search defaults to accepted RFCs. Terms must match within a section or its title;
title matches rank first. `--directory PATH` selects a different RFC directory.
Creation writes the next numbered draft and prints its path. It does not commit,
push, or open a PR.
