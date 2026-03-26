# acton-completions(1)

## NAME

acton-completions --- Generate static shell completion scripts

## SYNOPSIS

`acton completions` [_options_] _shell_

## DESCRIPTION

Generate a static completion script for the selected shell.

Acton also supports dynamic completions through the `COMPLETE=<shell> acton`
mechanism. Dynamic completions are usually more powerful because they can use
project context such as contract and script names.

## OPTIONS

### Completion Options

{{#options}}

{{#option "_shell_" }}
Shell to generate completions for.

Possible values: `bash`, `elvish`, `fish`, `powershell`, `zsh`
{{/option}}

{{/options}}

### Display Options

{{> options-display }}

### Project Options

{{> options-project-pass-through }}

## DYNAMIC COMPLETIONS

Dynamic completions are recommended when your shell supports them.

### Zsh

Add this to `~/.zshrc`:

```bash
source <(COMPLETE=zsh acton)
```

### Bash

Add this to `~/.bashrc`:

```bash
source <(COMPLETE=bash acton)
```

### Fish

Add this to `~/.config/fish/config.fish`:

```fish
COMPLETE=fish acton | source
```

## STATIC INSTALLATION

`acton completions` writes the completion script to standard output. Typical
installation patterns include:

### Bash

For the current shell session:

```bash
source <(acton completions bash)
```

For a persistent setup, either append to `~/.bashrc`:

```bash
acton completions bash >> ~/.bashrc
```

or install into the system completion directory:

```bash
acton completions bash | sudo tee /usr/share/bash-completion/completions/acton
```

### Zsh

Zsh usually expects the completion file in a directory listed in `$fpath`:

```bash
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

```bash
acton completions fish > ~/.config/fish/completions/acton.fish
```

### PowerShell

For the current session:

```powershell
acton completions powershell | Out-String | Invoke-Expression
```

For a persistent setup, add the generated script to your PowerShell profile.

### Elvish

```bash
acton completions elvish > ~/.elvish/completions/acton.elv
```

Then load it from `~/.elvish/rc.elv`:

```bash
use completions/acton
```

## VERIFYING INSTALLATION

After installation, try:

```bash
acton <TAB>
acton test --<TAB>
```

You should see command names and flag suggestions from the selected shell.

## TROUBLESHOOTING

- if static completions show old commands, regenerate them after updating Acton
- Zsh usually needs `compinit` after changing completion locations
- if Zsh still does not complete, delete `~/.zcompdump` and rerun `compinit`
- Bash may require the shell-specific completion package to be installed
- if PowerShell completions do not persist, verify that the generated script is
  loaded from `$PROFILE`

## EXAMPLES

1. Generate a static Zsh script:

   ```bash
   acton completions zsh
   ```

2. Enable dynamic Bash completions:

   ```bash
   source <(COMPLETE=bash acton)
   ```

## SEE ALSO

- [Shell completions guide](https://ton-blockchain.github.io/acton/docs/commands/shell-completions)
