# Governance discovery

Inspect canonical governance declarations without a local clone:

```sh
collar governance project list
collar governance project view
collar governance service-access
```

Collar resolves the latest canonical revision through the Forgejo API. Use `--offline` for the last verified snapshot. An optional `--checkout PATH` reads local changes instead.

These commands report declared ownership, membership, features, and service access; they do not verify that provisioning has been applied. Membership, feature, and repository declarations remain in the governance repository's normal review and Atlantis workflow. Collar does not author those changes or wrap the governance provisioner.
