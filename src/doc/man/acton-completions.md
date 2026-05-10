# acton-completions(1)

## Name

acton-completions --- Generate static shell completion scripts

## Synopsis

`acton completions` [_options_] _shell_

## Description

Generate a static completion script for the selected shell.

Acton also supports dynamic completions through the `COMPLETE=<shell> acton`
mechanism. Dynamic completions are usually more powerful because they can use
project context such as contract and script names.

## Quick Start

- for dynamic completions, prefer `COMPLETE=<shell> acton`
- for a static script without editing shell startup files yet, run
  `acton completions <shell>` and inspect the output first
- regenerate static completion files after upgrading Acton

## Options

### Completion Options

{{#options}}

{{#option "_shell_" }}
Shell to generate completions for.

Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`, `nushell`
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-pass-through }}

## Dynamic Completions

Dynamic completions are recommended when your shell supports them.

### Zsh

Add this to `~/.zshrc`:

```acton-cli
source <(COMPLETE=zsh acton)
```

### Bash

Add this to `~/.bashrc`:

```acton-cli
source <(COMPLETE=bash acton)
```

### Fish

Add this to `~/.config/fish/config.fish`:

```fish
COMPLETE=fish acton | source
```

## Static Installation

`acton completions` writes the completion script to standard output. Typical
installation patterns include:

### Bash

For the current shell session:

```acton-cli
source <(acton completions bash)
```

For a persistent setup, either append to `~/.bashrc`:

```acton-cli
acton completions bash >> ~/.bashrc
```

or install into the system completion directory:

```bash
acton completions bash | sudo tee /usr/share/bash-completion/completions/acton
```

### Zsh

Zsh usually expects the completion file in a directory listed in `$fpath`:

```acton-cli
mkdir -p ~/.zsh/completions
acton completions zsh > ~/.zsh/completions/_acton
echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
echo 'autoload -Uz compinit && compinit' >> ~/.zshrc
```

Reload the shell or run:

```bash
source ~/.zshrc
```

### Fish

```acton-cli
acton completions fish > ~/.config/fish/completions/acton.fish
```

### PowerShell

For the current session:

```powershell
acton completions powershell | Out-String | Invoke-Expression
```

For a persistent setup, add the generated script to your PowerShell profile.

### Elvish

```acton-cli
acton completions elvish > ~/.elvish/completions/acton.elv
```

Then load it from `~/.elvish/rc.elv`:

```bash
use completions/acton
```

### Nushell

```bash
acton completions nushell | save --force ~/.config/nushell/completions/acton.nu
```

Then load it from `config.nu`:

```nu
source ~/.config/nushell/completions/acton.nu
```

## Verifying Installation

After installation, try:

```acton-cli
acton <TAB>
acton test --<TAB>
```

You should see command names and flag suggestions from the selected shell.

## Troubleshooting

- if static completions show old commands, regenerate them after updating Acton
- Zsh usually needs `compinit` after changing completion locations
- if Zsh still does not complete, delete `~/.zcompdump` and rerun `compinit`
- Bash may require the shell-specific completion package to be installed
- if PowerShell completions do not persist, verify that the generated script is
  loaded from `$PROFILE`

## Exit Status

- `0`: The completion script was generated successfully.
- `1`: The shell value was unsupported or output could not be written.

## Examples

1. Generate a static Zsh script:

   ```bash
   acton completions zsh
   ```

2. Enable dynamic Bash completions:

   ```bash
   source <(COMPLETE=bash acton)
   ```

3. Inspect generated output without installing it:

   ```bash
   acton completions zsh | head
   ```

## See Also

- [Shell completions guide](https://ton-blockchain.github.io/acton/docs/commands/shell-completions)
