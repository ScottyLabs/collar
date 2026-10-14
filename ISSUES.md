# Issues and labels

Collar provides `issue list`, `view`, `create`, `edit`, `comment`, `close`, and
`reopen`, plus `label list`, `create`, `edit`, and `delete`. Issue list reads
Forgejo's native issues endpoint with its `type=issues` filter and applies
`--limit` during pagination. It rejects responses containing pull requests.
Issue mutations first inspect the selected number and refuse pull requests.
Forgejo remains authoritative for permissions and validation.

Issue and comment bodies accept `--body TEXT` or `--body-file PATH`; `-` reads
stdin. The body is sent verbatim. Label colors use six hexadecimal digits,
with an optional leading `#`. Labels are edited and deleted by Forgejo label ID.
Commands support `--json` with the fields shown by their output; issue output
uses familiar `author`, `url`, and `isArchived` field names where applicable.

These commands currently do not support issue URLs, assigning users, milestones,
attachments, reactions, label assignment, or label name lookup.
