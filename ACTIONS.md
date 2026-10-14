# Forgejo Actions commands

Collar reads workflow runs from Forgejo's Actions API. `actions list` accepts `--limit`, `--status`, and `--ref`; `actions view ID` and `actions jobs ID` inspect a run. `actions delete ID` requires a token and Forgejo only permits deletion after a run completes.

`workflow list` reads `.forgejo/workflows` and `.github/workflows` through the repository contents API. `workflow view FILE` returns Forgejo's contents response, including its encoding and metadata. Dispatch a workflow with `workflow dispatch FILE --ref REF`; optional inputs come from a JSON object whose values must all be strings. The request uses Forgejo's `ref`, `inputs`, and `return_run_info` fields. Mutations require `COLLAR_TOKEN` or `collar auth login`.

Forgejo 16.0.5's API does not provide dedicated workflow-list or workflow-view endpoints. It also returns job log content from its log endpoint rather than an official log URL. Collar does not construct a browser URL for logs. The HTTP client caps response bodies at 8 MiB, so Collar does not download job logs.
