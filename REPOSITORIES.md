# Repositories

Clone, inspect, and fork existing repositories:

```sh
collar repo clone ScottyLabs/terrier
collar repo view -R ScottyLabs/terrier
collar repo fork -R ScottyLabs/terrier
```

Cloning uses SSH by default; use `--https` for HTTPS. Repository views support JSON output.

ScottyLabs repositories, their metadata, and mirrors are declared in governance code. Creation, metadata changes, archiving, deletion, and mirror synchronization are not exposed as Collar repository commands. Use the canonical governance workflow for provisioning changes.

Forking creates a copy without changing the source repository. It defaults to your account; `--organization` and `--name` select a destination organization and name. Forge permissions still apply.
