# Repository administration

`collar repo admin` manages repositories through the Forgejo REST API on the selected host. Select the affected repository with `-R OWNER/REPO` (or the normal Git remote selection) for fork, edit, archive, unarchive, delete, and mirror synchronization. These commands require an authenticated token with the corresponding Forgejo permission. They do not grant permissions or bypass server policy.

Create a repository by naming its owner and owner type explicitly:

```sh
collar repo admin create terrier-tools --owner ScottyLabs --owner-type org --private
collar repo admin create scratch --owner my-account --owner-type user
```

Personal creation first verifies that `--owner` is the authenticated account. Forking supports the native Forgejo options:

```sh
collar repo admin fork -R upstream-org/project
collar repo admin fork -R upstream-org/project --organization ScottyLabs --name project-experiment
```

Edit accepts `--description`, `--homepage`, `--default-branch`, and one of `--private` or `--public`. Archive and unarchive set Forgejo's archived property. Permanent deletion requires `--confirm OWNER/REPO` to exactly match the selected repository. Mirror synchronization only runs when Forgejo reports the selected repository as a mirror; it asks Forgejo to sync the remote mirror and does not synchronize a local Git checkout.

Repositories governed by a canonical ScottyLabs declaration should be changed through that repository's review and governance process. The CLI exposes the forge's native operations and does not update governance declarations automatically.
