# Shell completions

Collar generates completions from its command definitions. Supported shells are
Bash, Elvish, Fish, PowerShell, Zsh, and Nushell. Regenerate persistent completions
when you update Collar.

## Nushell

Generate a module in Nushell's configuration directory:

```nu
collar completion nushell | save --force ($nu.default-config-dir | path join 'collar.nu')
```

Add this line to `config.nu`, then restart Nushell:

```nu
use collar.nu *
```

The generated module provides typed external-command definitions. Generating it
does not require Nushell to be installed on the generating machine.

Collar's JSON output also works with Nu's structured pipelines:

```nu
collar pr list -R ScottyLabs/collar --json number,title | from json | select number title
```

For a command that can fail, capture and inspect the external exit status before
using its output:

```nu
let result = (collar pr checks 42 -R ScottyLabs/collar --json | complete)
if $result.exit_code != 0 {
    error make {msg: $result.stderr}
}
$result.stdout | from json
```

## Bash

Enable completions in the current Bash session:

```bash
source <(collar completion bash)
```

For persistent setup, save the generated script in your Bash completion directory
or load it from your shell configuration.

## Fish

Enable completions in the current Fish session:

```fish
collar completion fish | source
```

For persistent setup:

```fish
collar completion fish > ~/.config/fish/completions/collar.fish
```

Create the completions directory first if it does not exist.

## Other shells

Generate a script for the shell you use:

```sh
collar completion zsh
collar completion powershell
collar completion elvish
```

Load it through that shell's completion configuration. These scripts describe
Collar's commands and options; they do not perform network requests or supply
live repository names.
