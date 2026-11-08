# Collar

Collar is the ScottyLabs development CLI. Use it to review Forgejo pull requests,
inspect CI and Kennel previews, find project documentation, and run local
development tools.

## Get started

From this repository, enter the ScottyLabs development environment:

```sh
nix develop
collar --help
collar repo view collar
```

To install the CLI from this checkout:

```sh
nix profile add .#collar
```

See [installation and first steps](https://docs.collar.scottylabs.org/getting-started/) for Cargo installation,
authentication, and requirements.

For coding agents, start with `collar skill install` and the
[agent setup guide](https://docs.collar.scottylabs.org/agents/).

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

- [User guide](https://docs.collar.scottylabs.org)
- [Command reference](https://docs.collar.scottylabs.org/reference/commands/), generated from CLI help
- [Install the agent skill](https://docs.collar.scottylabs.org/agents/#install-the-skill)
- [Contributing](CONTRIBUTING.md)

Kennel builds the `docs/` directory with Zensical and deploys it as the `docs` site in `flake.nix`.

## License

[AGPL-3.0-or-later](LICENSE).
