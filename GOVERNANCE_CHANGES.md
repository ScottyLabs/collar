# Governance changes

Governance changes edit TOML in an existing writable governance checkout. Collar identifies the authenticated Forgejo user for membership changes, edits only that user's membership, preserves unrelated TOML formatting and comments, and runs the canonical governance validator before reporting success. A validation failure restores the original file.

```sh
collar governance changes join --checkout ../governance --team devops
collar governance changes leave --checkout ../governance --project terrier/web
collar governance changes enable --checkout ../governance --feature sentry
collar governance changes enable --checkout ../governance --feature mobile_app --config app-store.json
collar governance changes disable --checkout ../governance --feature kennel
```

Team selectors are exact team slugs. Project selectors contain the team slug and each nested project slug, separated by `/`. Feature changes use the repository selected by `-R` or the current Git remote, then resolve its canonical project and source file. Feature configuration accepts a TOML file or a JSON object; omit it to enable a feature with an empty table and its canonical defaults.

These commands make a local patch only. Review the diff, commit it in the governance checkout, and submit it through the governance pull request workflow. Only team leads may modify another person's membership; these commands do not accept a username override.
