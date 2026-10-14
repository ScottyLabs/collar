# Releases

Collar's `release` command lists, views, creates, edits, and deletes Forgejo releases for the selected repository. Read operations use the repository release API; mutations require a Collar token.

Tags are positional. Create accepts a title and literal Markdown through `--notes` or `--notes-file` (use `-` to read stdin). Edit accepts the same content options, plus `--publish` to clear draft and `--stable` to clear prerelease. `--draft` and `--prerelease` set those flags. Release output includes asset names and Forgejo-provided download links; Collar does not upload or download asset bytes.

Examples:

```sh
collar -R ScottyLabs/terrier release list
collar -R ScottyLabs/terrier release view v1.2.0 --json
collar -R ScottyLabs/terrier release create v1.2.0 --title 'Terrier 1.2.0' --notes-file notes.md
collar -R ScottyLabs/terrier release edit v1.2.0 --publish --stable
collar -R ScottyLabs/terrier release delete v1.2.0
```
