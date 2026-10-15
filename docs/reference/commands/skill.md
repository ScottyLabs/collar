# `collar skill`

Global options apply to every command below; see the [command index](index.md).

```text
Show or install Collar's bundled agent skill

Usage: collar skill [COMMAND]

Commands:
  install  Install the bundled skill, or replace it explicitly after updating Collar
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Examples:
  collar skill
  collar skill install
  collar skill install --agent claude
  collar skill install --force

The skill ships with this binary and needs no network access. Installation defaults to Codex's user skill directory. Reinstall after updating Collar; --force explicitly replaces a different installed version.
```

## `collar skill install`

```text
Install the bundled skill, or replace it explicitly after updating Collar

Usage: collar skill install [OPTIONS] [DIRECTORY]

Arguments:
  [DIRECTORY]
          Skill root directory; Collar creates a collar/SKILL.md beneath it

Options:
      --agent <AGENT>
          Client whose user skill directory to use; defaults to codex

          [possible values: codex, claude]

      --force
          Replace a different installed skill, including local edits

  -h, --help
          Print help
```
