# Service documentation

Collar lists, reads, and searches ScottyLabs organization guides by default, with the Kennel, infrastructure, and governance documentation available through `--service`. It uses verified source snapshots shared with other collar commands; `--offline` reads the last verified snapshot, and `--checkout PATH` reads a local checkout.

```sh
collar docs list
collar docs view index
collar docs search "deployment preview"
collar kennel docs list
collar kennel docs view guides/features
collar docs view concepts/secrets --service infrastructure --offline
```

ScottyLabs topics come from the `org-docs/scottylabs/` section of the documentation repository. Kennel topics come from `sites/docs/src/` and its repository README. Infrastructure topics come from `docs/src/`; governance exposes its repository README. Generated `SUMMARY.md` files are excluded. JSON output includes the topic and repository-relative path; views include the Markdown body, and each result carries its source metadata.

`collar kennel docs` always selects Kennel, even when another `--service` is supplied. `collar docs` defaults to the organization guides.
