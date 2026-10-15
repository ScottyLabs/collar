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

For persistent setup:

```bash
mkdir -p ~/.local/share/bash-completion/completions
collar completion bash > ~/.local/share/bash-completion/completions/collar
```

If your system does not load that directory through `bash-completion`, add
`source ~/.local/share/bash-completion/completions/collar` to `~/.bashrc`.

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

## Zsh

Generate an autoloadable completion file:

```zsh
mkdir -p ~/.zsh/completions
collar completion zsh > ~/.zsh/completions/_collar
```

Add these lines to `~/.zshrc`, placing the `fpath` line before any existing
`compinit` call. Run `compinit` only once:

```zsh
fpath=(~/.zsh/completions $fpath)
autoload -Uz compinit
compinit
```

Restart Zsh to load the completion. For the current session, run the same setup
lines after generating the file.

## PowerShell

Load completions for the current session:

```powershell
collar completion powershell | Out-String | Invoke-Expression
```

For persistent setup, save the script beside your PowerShell profile:

```powershell
$completionDir = Split-Path -Parent $PROFILE
New-Item -ItemType Directory -Force $completionDir | Out-Null
collar completion powershell | Set-Content (Join-Path $completionDir 'collar.ps1')
```

Add `. (Join-Path (Split-Path -Parent $PROFILE) 'collar.ps1')` to `$PROFILE`.
Restart PowerShell or dot-source the file to load it now. Your execution policy
must permit loading the script; follow your machine's policy.

## Elvish

Save the generated module under Elvish's configuration directory:

```elvish
mkdir -p ~/.config/elvish/lib
collar completion elvish > ~/.config/elvish/lib/collar.elv
```

Add `use collar` to `~/.config/elvish/rc.elv`, then restart Elvish. Use your
Elvish configuration directory if you have customized its location. In an
interactive session, `use collar` loads the module immediately.

## Updating completions

After updating Collar, repeat the generation command for your shell and start a
new session. Scripts describe command syntax; they do not make network requests
or complete live repository names. Completions cover Collar's commands and options.
