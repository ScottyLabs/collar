# Collar

Collar is the ScottyLabs development CLI. Use it to review Forgejo pull requests,
inspect CI and Kennel previews, find project documentation, and run local
development tools.

## Get started

From this repository, enter the ScottyLabs development environment:

```sh
devenv shell
collar --help
collar repo view collar
```

To install the CLI from this checkout:

```sh
nix profile add .#collar
```

See [installation and first steps](docs/getting-started.md) for Cargo installation,
authentication, and requirements.

For coding agents, start with `collar skill install` and the
[agent setup guide](docs/agents.md).

## Everyday commands

```sh
collar auth login
collar pr list
collar pr review 42 --approve
collar kennel preview 42
collar docs search secrets
collar check --all
```

Repository commands use the current checkout's `upstream` remote, then `origin`.
Pass `-R collar` to work without a checkout; bare names default to `ScottyLabs`. Sign-in is required for
private reads and writes; public reads work without it.

## Documentation

- [User guide](docs/index.md)
- [Command reference](docs/reference/commands/index.md), generated from CLI help
- [Install the agent skill](docs/agents.md#install-the-skill)
- [Contributing](CONTRIBUTING.md)

The [ScottyLabs documentation hub](https://docs.scottylabs.org) publishes the
`docs/` directory when the repository's governance `docs` feature is enabled.
All documentation source stays in this repository.

## License

[AGPL-3.0-or-later](LICENSE).
