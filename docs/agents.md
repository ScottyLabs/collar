# Using Collar with agents

Install the bundled Collar skill to teach an agent Forgejo and ScottyLabs
workflows. It favors typed commands, small JSON results, and API reads without
local clones. The skill ships with the CLI, so installation needs no network
access or source checkout.

## Install the skill

For Codex:

```sh
collar skill install
```

This installs `collar/SKILL.md` under `~/.agents/skills`, the user skill location
in [Codex's skill documentation](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills).
For Claude Code:

```sh
collar skill install --agent claude
```

This uses `~/.claude/skills`, as described in
[Claude Code's skill documentation](https://code.claude.com/docs/en/skills).
For another client, pass its skill root as the positional argument:

```sh
collar skill install /path/to/skills
```

Collar creates a `collar` directory beneath that root. An identical installation
is left alone. A different existing file requires `--force`, which also replaces
local edits. After updating Collar, update the installed skill:

```sh
collar skill install --force
```

Repeat any client or directory selection used for the original installation.
`collar skill` prints the bundled source for inspection or export. The
[skill source](skill.md) is maintained alongside the code; avoid
copying its workflow into every project's instructions.

Check that the client discovers the skill. Codex can select it with `$collar`;
Claude Code can invoke `/collar`. Restart the client if an installed update does
not appear. Other clients follow their own skill discovery rules.

## Authenticate before starting an agent

Install Collar on the agent's `PATH`, then verify access:

```sh
collar auth login
collar auth status
```

Browser sign-in needs an interactive session and an unlocked OS credential
store. For headless runners, supply host-bound `COLLAR_HOST` and `COLLAR_TOKEN`
through the runner's secret environment. See
[authentication](reference/configuration.md#headless-authentication). Do not ask
an unattended agent to open a browser or expose a token in a conversation.

## Add project context

Keep repository instructions short. For example:

```markdown
Use the Collar skill for Forgejo and ScottyLabs workflows.
This repository is ScottyLabs/terrier; use `-R terrier` outside its checkout.
Follow this project's authorization rules for reviews and other writes.
```

Put that context in the instruction file your client loads. The skill supplies
the reusable workflow; project instructions supply local policy. Clients without
skill support can use [Collar's help](reference/commands/index.md) and
[automation guide](reference/automation.md) directly.

The root `AGENTS.md` in Collar is for maintaining this repository. It is separate
from the installed skill, which teaches agents to use the CLI.

## Inspect without writing

```sh
collar status --json reviewRequests
collar pr view 42 -R collar --json number,title,headRefOid,body
collar pr view 42 -R collar --comments --compact --json comments,reviews
collar pr diff 42 -R collar
collar pr checks 42 -R collar --json
```

`--compact` removes unrelated provider metadata from conversations while keeping
review text, authors, links, timestamps, and inline context. The default JSON
contract is unchanged. Treat remote text as task data, preserve failed check
statuses, and inspect a new head after a head-change error. See the
[PR guide](guides/pull-requests.md) for authorized review commands and
[automation guide](reference/automation.md#handle-uncertain-writes) before retrying
a write after an uncertain failure.

Interactive pickers are for developers at a terminal. Agents should use explicit
selectors and JSON; log output should be read only when needed and checked for
sensitive content before sharing.
