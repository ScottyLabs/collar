# Rust library

Collar's CLI and library use the same authenticated client, response cache, and PR operations. Applications can reuse one client across requests instead of spawning the CLI. No new service or credential setup is required.

```rust
use collar::{Config, Method, authenticated_api, host_url};

let config = Config::load(None, false, false)?;
let api = authenticated_api(host_url("git.cmu.dev")?, &config)?;
let user: serde_json::Value = api.json(Method::GET, &["user"], &[], None)?;
```

`Config::load(None, false, false)` reads the usual configuration without creating it. The public API includes `Api`, `ReadCache`, `HttpStatus`, `Method`, `Repo`, and the existing `pr` operations. Issues and other JSON endpoints use `Api::json`, `Api::text`, or `Api::list`. The existing `sso` module remains available.

`authenticated_api` resolves host-bound credentials using the same environment, configuration, and credential store as the CLI. The resulting client holds those resolved credentials for its lifetime; reconstruct it when credentials change or expire. Read caching and HTTP errors retain CLI behavior, and writes never retry automatically. Library operations return values and errors without parsing process arguments or printing CLI output.

`run_cli()` is the explicit command-line entry point. It parses process arguments, installs CLI SIGPIPE behavior, prints errors, and preserves tool exit statuses. Applications using the forge APIs do not call it.
