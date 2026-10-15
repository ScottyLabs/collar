# `collar pr`

Global options apply to every command below; see the [command index](index.md).

```text
Inspect and review pull requests

Usage: collar pr <COMMAND>

Commands:
  checkout       Fetch a verified PR head and switch to a local branch
  merge          Merge the inspected PR head using Forgejo protection checks
  update-branch  Merge or rebase the target branch into the PR branch
  create         Create a PR from an already-pushed branch
  edit           Edit a PR's title, body, or target branch
  comment        Add a discussion comment to a PR
  close          Close a PR without merging it
  reopen         Reopen a closed, unmerged PR
  browse         Select a PR interactively, then inspect its details, diff, or checks
  list           List pull requests
  view           View a PR selected by number, URL, or branch (default: current branch)
  diff           Print a PR's unified diff
  checks         Show commit statuses; fail when none are reported or any are not successful
  review         Submit a review, optionally including inline comments
  reviewers      List, request, or cancel reviewers for a PR
  help           Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar pr list
  collar pr view 42
  collar pr review 42 --approve
```

## `collar pr checkout`

```text
Fetch a verified PR head and switch to a local branch

Usage: collar pr checkout [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
  -b, --branch <BRANCH>
          Local branch name; defaults to pr/NUMBER

      --remote <REMOTE>
          Configured Git remote; defaults to upstream, then origin

  -h, --help
          Print help
```

## `collar pr merge`

```text
Merge the inspected PR head using Forgejo protection checks

Usage: collar pr merge [OPTIONS] <--merge|--squash|--rebase> [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --merge
          Preserve commits with a merge commit

      --squash
          Combine the PR into a single commit

      --rebase
          Rebase the PR commits instead of creating a merge commit

      --match-head-commit <MATCH_HEAD_COMMIT>
          Refuse if the current head differs from this commit

      --delete-branch
          Ask Forgejo to delete the source branch after merging

      --auto
          Ask Forgejo to merge when its checks succeed

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr update-branch`

```text
Merge or rebase the target branch into the PR branch

Usage: collar pr update-branch [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --rebase
          Rebase the PR commits instead of creating a merge commit

      --match-head-commit <MATCH_HEAD_COMMIT>
          Check the current head before submission; the update API has no atomic guard

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr create`

```text
Create a PR from an already-pushed branch

Usage: collar pr create [OPTIONS] --title <TITLE>

Options:
  -t, --title <TITLE>
          Pull request title

  -H, --head <HEAD>
          Branch or fork-owner:branch; defaults to the current Git branch

  -B, --base <BASE>
          Target branch; defaults to the forge repository's default branch

  -b, --body <BODY>
          Markdown text for the description, discussion comment, or review

  -F, --body-file <BODY_FILE>
          Read Markdown from a file, or - for stdin

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr edit`

```text
Edit a PR's title, body, or target branch

Usage: collar pr edit [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
  -t, --title <TITLE>
          Pull request title

  -B, --base <BASE>


  -b, --body <BODY>
          Markdown text for the description, discussion comment, or review

  -F, --body-file <BODY_FILE>
          Read Markdown from a file, or - for stdin

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr comment`

```text
Add a discussion comment to a PR

Usage: collar pr comment [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
  -b, --body <BODY>
          Markdown text for the description, discussion comment, or review

  -F, --body-file <BODY_FILE>
          Read Markdown from a file, or - for stdin

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr close`

```text
Close a PR without merging it

Usage: collar pr close [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr reopen`

```text
Reopen a closed, unmerged PR

Usage: collar pr reopen [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr browse`

```text
Select a PR interactively, then inspect its details, diff, or checks

Usage: collar pr browse [OPTIONS]

Options:
  -s, --state <STATE>
          [default: open]
          [possible values: open, closed, all]

  -L, --limit <LIMIT>
          [default: 30]

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr list`

```text
List pull requests

Usage: collar pr list [OPTIONS]

Options:
  -s, --state <STATE>
          [default: open]
          [possible values: open, closed, all]

  -L, --limit <LIMIT>
          [default: 30]

      --author <AUTHOR>
          Filter by author login; @me uses the authenticated account

      --base <BASE>
          Filter by target branch

      --head <HEAD>
          Filter by source branch

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr view`

```text
View a PR selected by number, URL, or branch (default: current branch)

Usage: collar pr view [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --comments
          Include discussion, reviews, and inline review comments

      --compact
          Omit provider metadata from conversation JSON

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr diff`

```text
Print a PR's unified diff

Usage: collar pr diff [SELECTOR]

Arguments:
  [SELECTOR]


Options:
  -h, --help
          Print help
```

## `collar pr checks`

```text
Show commit statuses; fail when none are reported or any are not successful

Usage: collar pr checks [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help
```

## `collar pr review`

```text
Submit a review, optionally including inline comments

Usage: collar pr review [OPTIONS] <--approve|--request-changes|--comment> [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, same-repository URL, or branch; defaults to the current branch

Options:
  -a, --approve
          Submit an approval

  -r, --request-changes
          Submit a change request

  -c, --comment
          Submit a review comment without approving or requesting changes

  -b, --body <BODY>
          Markdown text for the description, discussion comment, or review

  -F, --body-file <BODY_FILE>
          Read review text from a file, or - for stdin

      --inline-comment <PATH> <LINE> <SIDE> <BODY>
          Add an inline comment; repeat for multiple comments. SIDE is old or new

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar pr review 42 --approve
  collar pr review 42 --request-changes --body 'Cover the timeout case.'
  collar pr review 42 --comment --inline-comment src/main.rs 12 new 'Handle this error.'

SELECTOR is a PR number, same-repository URL, or branch; omitted selects the current branch. Reviews submit immediately and are pinned to the inspected head.
```

## `collar pr reviewers`

```text
List, request, or cancel reviewers for a PR

Usage: collar pr reviewers [OPTIONS] [SELECTOR]

Arguments:
  [SELECTOR]
          PR number, URL, or branch; defaults to the current branch

Options:
      --add <ADD>...
          Request reviews from these usernames

      --remove <REMOVE>...
          Cancel review requests for these usernames

      --json [<FIELDS>]
          Output selected JSON fields (comma-separated); omit fields for all

  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar pr reviewers 42
  collar pr reviewers 42 --add anish kritdass
  collar pr reviewers 42 --remove anish

With no changes, lists the requested users and teams. Changes require sign-in and an open PR; --add and --remove cannot be combined.
```
