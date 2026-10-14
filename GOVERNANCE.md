# Governance workflows

Collar reads canonical governance snapshots for project and service-access views. Commands that run the governance repository's own CLI require a full working checkout:

```sh
git clone https://git.cmu.dev/ScottyLabs/governance.git
cd governance
collar governance validate --checkout .
collar governance run --checkout . -- schema
collar governance run --checkout . -- projects
```

`governance validate` runs the canonical `cargo run -p governance -- validate` command directly. The validator checks local governance data and also checks configured Slack, Discord, and Keycloak identities through their APIs, so those checks require the corresponding credentials in the environment.

`governance run` runs any canonical governance CLI command as `devenv shell -- cargo run -p governance -- ...` from the verified checkout. Its arguments are passed literally. The upstream CLI includes local commands such as `schema`, `projects`, and `resolve-identities`, plus service operations such as Slack membership changes and PostHog invitations. Review command behavior before invoking a service operation; Collar does not add a dry run or permission check around upstream commands.

For a governance change, use a branch and the repository's normal review process:

```sh
git switch -c governance/update-project
# edit data/teams/*.toml or other governance sources
collar governance validate --checkout .
git add data/teams/
git commit -m "feat(governance): update project access"
git push -u origin HEAD
collar pr create --title "feat(governance): update project access"
```

Review the pull request and its Atlantis plan. Apply through the repository's approved Atlantis workflow after review; Collar does not apply OpenTofu changes.
