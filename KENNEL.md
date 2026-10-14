# Kennel reports in Collar

`kennel preview <PR>` reads Forgejo issue comments carrying the
`<!-- kennel-deployment -->` marker and extracts HTTPS deployment links from
their table. Each result identifies whether its "Last updated for commit"
prefix matches the selected PR head. The comment is still a historical report;
the prefix does not prove the service is currently running.

`kennel logs <PR>` prints the latest `kennel/build` and `kennel/deploy` commit
statuses for the PR head, including each report's target URL. Kennel commonly
uses these target URLs for build or deployment logs. Forgejo may retain statuses
after the corresponding service changes.

`kennel wait <PR>` pins the starting PR head and polls those two statuses for
up to 120 seconds. It succeeds only when both statuses report `success`, fails
on build/deployment failure, PR closure, head changes, or timeout, and accepts
`--timeout` (1-300 seconds) and `--interval` (1-30 seconds). Success means
Kennel reported deployment success for that commit. It does not query a live
health endpoint. The timeout bounds the polling window; an in-flight Forgejo
request can extend wall-clock time by its HTTP request timeout.

These commands use Forgejo's existing PR, issue-comment, and commit-status
endpoints. They do not require Kennel-specific API endpoints.
