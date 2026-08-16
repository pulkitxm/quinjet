# `quinjet completions`

Prints a shell completion script for Quinjet on stdout.

Usage:

```bash
quinjet completions <SHELL> [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SHELL>` | one of `bash`, `elvish`, `fish`, `powershell`, `zsh` | required | The shell to write the script for. Any other value exits 2 and lists the accepted ones. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--json` | flag | off | Wraps the script in one JSON object instead of printing it raw. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The script is rendered from the same command tree the parser uses, so it
always offers exactly the verbs and flags this build has. There is no
committed copy to fall behind, and nothing to regenerate after adding a verb.

This verb reads no repository. `-C` is accepted because it is global, and is
ignored: the answer does not depend on where you are, so it works outside a
Git repository.

The five supported generators are bash, zsh, fish, elvish, and PowerShell.
Process tests run every generator from a directory that is not a repository,
and pass the generated bash script to `bash -n` for a real syntax check.

```console
$ quinjet completions bash | head -3
_quinjet() {
    local i cur prev opts cmd
    COMPREPLY=()
```

## Installing it

Generation does not install anything. Each shell looks in a different place,
and none of them reads Quinjet's stdout automatically, so save or evaluate the
script explicitly.

```bash
quinjet completions bash > /usr/share/bash-completion/completions/quinjet
quinjet completions zsh > ~/.zfunc/_quinjet
quinjet completions fish > ~/.config/fish/completions/quinjet.fish
```

For zsh the directory has to be on `fpath` before `compinit` runs, so
`~/.zshrc` needs `fpath=(~/.zfunc $fpath)` above the `compinit` line. For
bash, a system directory needs root, and `~/.local/share/bash-completion/completions/quinjet`
works without it. PowerShell has no completion directory; it evaluates the
script instead:

```powershell
quinjet completions powershell | Out-String | Invoke-Expression
```

Adding that line to your PowerShell profile makes it permanent. The same
pattern works in any shell if you prefer generating on startup, at the cost of
running Quinjet once per new shell.

`--json` shape, the shell you asked for and the script as one string:

```json
{
  "shell": "bash",
  "script": "_quinjet() {\n    local i cur prev opts cmd\n..."
}
```

Examples:

```bash
quinjet completions zsh
quinjet completions fish > ~/.config/fish/completions/quinjet.fish
quinjet completions bash --json
```

## Where to go next

- [`quinjet man`](./man.md) for the manual pages, generated the same way
- [Generated references](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
