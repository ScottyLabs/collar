# Browser sign-in

Collar's browser sign-in uses Forgejo's pre-registered `git-credential-oauth`
OAuth application through the `git-credential-oauth` helper. Forgejo 16
pre-registers that application's client ID, so users do not create or share an
OAuth client secret. The helper performs the OAuth authorization flow and
returns the access and refresh tokens to Collar; Collar validates the access
token against the forge's `/api/v1/user` endpoint before saving both tokens
together in the operating system credential store. The helper requests no
explicit OAuth scopes, so the resulting token acts with the signed-in user's
normal Forgejo permissions. When the access token expires, API commands report
that sign-in is required. Run `collar auth login` again to use the saved refresh
token; refresh never starts implicitly during another command.

Install `git-credential-oauth` with the package manager for your operating
system, then run `collar auth login` and approve Collar's access in the browser.
The helper must be available on `PATH`. Forgejo's built-in application uses a
loopback callback, so the browser and Collar must run on the same machine. The
flow requires an HTTPS forge URL, except for loopback development instances.

If the pre-registered application is unavailable, Collar cannot complete
browser sign-in. Ask the forge administrator to enable it. Never substitute
another application's client ID or client secret.
