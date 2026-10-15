# Collar

Collar brings ScottyLabs development workflows into one CLI: pull requests,
issues, CI, previews, project ownership, documentation, and local development.
It uses Forgejo APIs for remote reads and calls the existing ScottyLabs tools
for local operations.

Check `collar --version` and use the help
shipped with your installed version when a command differs from this guide.

## Start here

1. [Install Collar and sign in](getting-started.md).
1. [Install the Collar skill for an agent](agents.md#install-the-skill).
1. [Inspect and review a pull request](guides/pull-requests.md).
1. [Check CI and previews](guides/actions-and-previews.md).
1. [Run development commands](guides/development.md).

## Find a task

| Task | Guide |
| --- | --- |
| Find ownership, services, documentation, or changes | [ScottyLabs projects](guides/scottylabs.md) |
| Manage issues, labels, releases, or forks | [Repository work](guides/repository-work.md) |
| Enable shell completions or use Nushell | [Shell completions](guides/completions.md) |
| Install or update the agent skill | [Agent setup](agents.md#install-the-skill) |
| Configure a host or understand authentication | [Configuration and authentication](reference/configuration.md) |
| Script commands and handle failures | [Automation](reference/automation.md) |
| Diagnose a problem | [Troubleshooting](troubleshooting.md) |
| Look up arguments and defaults | [Command reference](reference/commands/index.md) |

Run `collar --help` to see command groups and `collar pr review --help` to inspect
a specific command. `-h` gives brief help; `--help` includes extended descriptions.
