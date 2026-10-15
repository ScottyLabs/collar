# Contributing

Collar is a standalone Rust CLI and a draft proof of concept. Keep changes
focused on useful ScottyLabs workflows and test the behavior users will see.

## Development

Use `devenv shell` from the repository root, or prefix a command with
`devenv shell --`. The shared ScottyLabs module supplies Rust, formatting, and
commit hooks. Declare additional tools in `devenv.nix`; do not edit generated
hook configuration.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
prek run --all-files
```

Run focused tests while developing, then the required hooks before committing.
Use fixtures for remote writes; do not post comments, submit reviews, or change
live resources as a test. Keep warnings at zero. Dependency changes use Cargo's
commands and update the lockfile.

## Command design

Use the current checkout or explicit `-R` when possible. Remote reads should use
APIs rather than requiring maintained clones. Delegate to existing ScottyLabs
tools without adding unrelated prerequisites. Accept short input directly;
optional files and stdin can support longer text or secret input.

Help is part of the interface. Give commands a clear summary, describe options
that need explanation, and include examples for common tasks. Writes should be
obvious and must not retry automatically. Preserve JSON output and tool exit
statuses. Governance owns repository provisioning, membership, and features.

## Documentation

User documentation lives in `docs/`. The governance `docs` feature publishes that
directory through the ScottyLabs documentation hub. Preview the same content:

```sh
docs
```

The `docs` shell helper runs `mdbook serve`. `mdbook build` builds locally, and
`nix build .#docs` checks the packaged documentation. The root README stays short;
put task guides and explanations in the documentation. Agent-use guidance belongs
in `docs/agents.md`; repository maintenance instructions belong in `AGENTS.md`.

Command reference pages are generated from Clap's help definitions. After changing
commands or help, regenerate them inside the development shell:

```sh
COLLAR_UPDATE_DOCS=1 cargo test command_reference_is_current
```

The regular test run checks that generated pages match the CLI. Review the output
and update task guides when behavior changes. Verify local links and examples;
do not document a proposed command as shipped.

## Commits and review

Use atomic conventional commits with short subjects and no message body. Keep a
linear history by rebasing; squash correction commits before review. Do not add
authorship trailers or attribution notices for automated tools.

Update tests and documentation with user-facing changes. Describe the resulting
behavior and relevant validation in the PR. Leave the [Collar RFC](docs/design/collar.md)
as Draft until it has been accepted through review.

Contributions are licensed under [AGPL-3.0-or-later](LICENSE).
