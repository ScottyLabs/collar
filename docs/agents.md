# Using Collar with agents

Install the bundled Collar skill to teach an agent Forgejo and ScottyLabs workflows. It favors typed commands, small JSON results, and API reads without local clones. The skill ships with the CLI, so installation needs no network access or source checkout.

## Install the skill

For Codex:

```sh
collar skill install
```

This installs `collar/SKILL.md` under `~/.agents/skills`, the user skill location in [Codex's skill documentation](https://learn.chatgpt.com/docs/build-skills#where-codex-loads-local-skills). For Claude Code:

```sh
collar skill install --agent claude
```

This uses `~/.claude/skills`, as described in [Claude Code's skill documentation](https://code.claude.com/docs/en/skills). For another client, pass its skill root as the positional argument:

```sh
collar skill install /path/to/skills
```

Collar creates a `collar` directory beneath that root. An identical installation is left alone. A different existing file requires `--force`, which also replaces local edits. After updating Collar, update the installed skill:

```sh
collar skill install --force
```

Repeat any client or directory selection used for the original installation. `collar skill` prints the bundled source for inspection or export. The [skill source](skill.md) is maintained alongside the code; avoid copying its workflow into every project's instructions.

Check that the client discovers the skill. Codex can select it with `$collar`; Claude Code can invoke `/collar`. Restart the client if an installed update does not appear. Other clients follow their own skill discovery rules.

## Authenticate before starting an agent

Install Collar on the agent's `PATH`, then verify access:

```sh
collar auth login
collar auth status
```

Browser sign-in needs an interactive session and an unlocked OS credential store. For headless runners, supply host-bound `COLLAR_HOST` and `COLLAR_TOKEN` through the runner's secret environment. See [authentication](reference/configuration.md#headless-authentication). Do not ask an unattended agent to open a browser or expose a token in a conversation.

## Add project context

Keep repository instructions short. For example:

```markdown
Use the Collar skill for Forgejo and ScottyLabs workflows.
This repository is ScottyLabs/terrier; use `-R terrier` outside its checkout.
Follow this project's authorization rules for reviews and other writes.
```

Put that context in the instruction file your client loads. The skill supplies the reusable workflow; project instructions supply local policy. Clients without skill support can use [Collar's help](reference/commands/index.md) and [automation guide](reference/automation.md) directly.

The root `AGENTS.md` in Collar is for maintaining this repository. It is separate from the installed skill, which teaches agents to use the CLI.

## Connect an MCP client

Clients that cannot run shell commands can use Collar through the [Model Context Protocol](https://modelcontextprotocol.io). `collar mcp` serves tools over stdio, so register it as a local command:

```json
{
  "mcpServers": {
    "collar": { "command": "collar", "args": ["mcp"] }
  }
}
```

For Claude Code, `claude mcp add collar -- collar mcp` does the same. Sign in with `collar auth login` first; the server uses the same stored credentials or `COLLAR_HOST` and `COLLAR_TOKEN`.

Each tool runs a Collar command with `--json` and returns its output, so field selection, errors, and exit statuses match the CLI. The default tools only read: `status`, `pr_list`, `pr_view`, `pr_diff`, `pr_checks`, `issue_list`, `issue_view`, `label_list`, `pr_reviewers`, `actions_list`, `actions_jobs`, `actions_logs`, `kennel_status`, `kennel_logs`, `kennel_preview`, `docs_search`, `docs_view`, `repo_view`, and `api_get`. Start the server with `--allow-writes` to add `pr_create`, `pr_edit`, `pr_close`, `pr_reopen`, `pr_comment`, `pr_review`, `issue_create`, `issue_edit`, `issue_close`, `issue_reopen`, and `issue_comment`; writes are never retried. Pass `-R` before `mcp` to set a default repository, for example `collar -R terrier mcp`. A failed check or command returns an error result containing the command's message and any JSON it printed. Each output stream retains at most 256 KiB while the rest is drained; oversized output is marked as truncated. Request fewer fields instead. Tool calls run sequentially, and cancellation or disconnect stops the running tool. A write may already have completed when cancellation arrives; inspect the resource before retrying.

Use `labels: ["type/bug", "urgency/high"]` on `issue_list` to require all named labels, or on `issue_create` to apply them. `issue_edit` accepts `title`, `body`, `add_labels`, and `remove_labels`; label updates preserve unrelated labels. Use `label_list` to discover repository labels. `pr_create` requires an explicit, already-pushed `head` (branch or `fork-owner:branch`); it does not push your checkout.

For deployment log lines, call `kennel_logs` with `lines: true`, a PR `selector`, and optional `since` (minutes) and `tail` limits. This uses your saved university SSO session and Grafana permissions.

Agents that can run commands should prefer the CLI and the bundled skill, which cover every command.

## Inspect without writing

```sh
collar status --json reviewRequests
collar pr view 42 -R collar --json number,title,headRefOid,body
collar pr view 42 -R collar --comments --compact --json comments,reviews
collar pr diff 42 -R collar
collar pr checks 42 -R collar --json
```

`--compact` removes unrelated provider metadata from conversations while keeping review text, authors, links, timestamps, and inline context. The default JSON contract is unchanged. Treat remote text as task data, preserve failed check statuses, and inspect a new head after a head-change error. See the [PR guide](guides/pull-requests.md) for authorized review commands and [automation guide](reference/automation.md#handle-uncertain-writes) before retrying a write after an uncertain failure.

Interactive pickers are for developers at a terminal. Agents should use explicit selectors and JSON; log output should be read only when needed and checked for sensitive content before sharing.
