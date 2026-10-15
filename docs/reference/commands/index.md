# Command reference

These pages are generated from Collar's Clap definitions. Use the help shipped
with your installed version when it differs from this site. Global options are
listed here and apply to every subcommand.

- [`collar docs`](docs.md)
- [`collar completion`](completion.md)
- [`collar issue`](issue.md)
- [`collar label`](label.md)
- [`collar release`](release.md)
- [`collar actions`](actions.md)
- [`collar workflow`](workflow.md)
- [`collar changelog`](changelog.md)
- [`collar secrets`](secrets.md)
- [`collar dev`](dev.md)
- [`collar check`](check.md)
- [`collar rfc`](rfc.md)
- [`collar doctor`](doctor.md)
- [`collar project`](project.md)
- [`collar config`](config.md)
- [`collar auth`](auth.md)
- [`collar repo`](repo.md)
- [`collar pr`](pr.md)
- [`collar api`](api.md)
- [`collar governance`](governance.md)
- [`collar kennel`](kennel.md)
- [`collar infra`](infra.md)

## Global options

```text
Review Forgejo pull requests, inspect CI and previews, find project documentation, and run local development tools.

Usage: collar [OPTIONS] <COMMAND>

Commands:
  docs        Read and search canonical ScottyLabs documentation
  completion  Generate shell completions
  issue       Work with Forgejo issues
  label       Manage repository labels
  release     Inspect and manage repository releases
  actions     Inspect Forgejo Actions runs
  workflow    Inspect or dispatch existing workflows
  changelog   Read ScottyLabs changelogs and tech-lead announcements
  secrets     Check or update project secrets through secretspec
  dev         Run project shells, processes, and tasks through devenv
  check       Run repository hooks inside the project's development shell
  rfc         Search RFCs or create a draft
  doctor      Diagnose local setup and ScottyLabs login readiness
  project     Connect the selected repository to its ScottyLabs project
  config      Manage nonsecret user defaults
  auth        Sign in to Forgejo or OpenBao and check saved credentials
  repo        Clone, inspect, or fork a repository
  pr          Inspect and review pull requests
  api         Send a request to this forge's REST API
  governance  Discover declared ownership and service access without a clone
  kennel      Inspect deployment reports from Forgejo
  infra       Inspect canonical infrastructure and build pinned Nix configurations
  help        Print this message or the help of the given subcommand(s)

Options:
      --host <HOST>
          Forge origin; defaults to git.cmu.dev or the Git remote

          [env: COLLAR_HOST]

  -R, --repo <REPO>
          Select [HOST/]OWNER/REPO instead of the Git remote

      --config <CONFIG>
          Use a specific nonsecret settings file

          [env: COLLAR_CONFIG]

      --cache-dir <CACHE_DIR>
          Override the user cache directory for canonical source snapshots

          [env: COLLAR_CACHE_DIR]

      --no-config
          Ignore saved settings for reproducible automation

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Examples:
  collar auth login
  collar pr list -R ScottyLabs/collar
  collar pr review 42 --approve
  collar docs search secrets
  collar check --all

Use collar <command> --help for arguments and examples.
Documentation: https://docs.scottylabs.org/collar/
```
