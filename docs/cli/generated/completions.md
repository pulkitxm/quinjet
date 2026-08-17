# `quinjet completions`

Prints a shell completion script for Quinjet on stdout, or installs it with a
`q` shortcut for the selected shell.

Usage:

```bash
quinjet completions <SHELL> [--install] [--json]
quinjet completions --install [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SHELL>` | one of `bash`, `elvish`, `fish`, `powershell`, `zsh` | required unless `--install` detects it | The shell to write or install a script for. Any other value exits 2 and lists the accepted ones. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--install` | flag | off | Writes the generated script and a `q` launcher on `PATH` instead of writing the script to stdout. This explicit form restores files or launchers removed after an earlier installation. |
| `--json` | flag | off | Wraps the generated script or installed paths in one JSON object. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The script is rendered from the same command tree the parser uses, so it
always offers exactly the verbs and flags this build has. There is no
committed copy to fall behind, and nothing to regenerate after adding a verb.
Paths are marked as paths, while branch names, revisions, stash references,
pull-request numbers, check names, and intervals are marked as non-path values,
so the generated shell integration does not offer unrelated filenames for IDs.

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

The release installers run the automatic form before they finish. A binary
installed by Cargo, cargo-binstall, a package manager, or a direct copy runs the
same check on its first invocation. Completion installation also adds a `q`
launcher beside the Quinjet executable, or in a user bin directory already on
`PATH` when the executable directory is not writable. On Unix this is a
symbolic link; on Windows it is `q.cmd`. The launcher is available to the
current shell immediately when that directory is already on `PATH`. Quinjet
never replaces an unrelated `q` command that is already on `PATH`. The current
shell is detected from `$SHELL`, from a PowerShell environment, or from the
Windows platform. When no shell is detectable, first-run maintenance still
installs `q` but leaves completion selection for an explicit command. The Unix
release installer uses bash completion paths when `$SHELL` is unset so both
pieces are installed. Name a shell explicitly when detection is not
appropriate:

```bash
quinjet completions --install
quinjet completions bash --install
quinjet completions zsh --install
```

Start a new shell session after the first installation so its profile loads any
completion integration. The `q` launcher does not require a restart.

The generated scripts use user-owned paths:

| Shell | Script |
| --- | --- |
| bash | `$XDG_DATA_HOME/bash-completion/completions/quinjet`, else `~/.local/share/bash-completion/completions/quinjet` |
| fish | `$XDG_CONFIG_HOME/fish/completions/quinjet.fish`, else `~/.config/fish/completions/quinjet.fish` |
| zsh | `$ZDOTDIR/.zfunc/_quinjet`, else `~/.zfunc/_quinjet` |
| elvish | `$XDG_CONFIG_HOME/elvish/lib/quinjet.elv`, else `~/.config/elvish/lib/quinjet.elv` |
| PowerShell | `quinjet-completions.ps1` beside each current-user, all-hosts profile |

For zsh, Quinjet adds one marked block to `.zshrc` that places `.zfunc` on
`fpath` and initializes completion. For elvish it adds `use quinjet` to
`rc.elv`. For PowerShell it adds a dot-source line to the profile. Bash and fish
discover their user completion directories directly. Existing profile text and
permissions are preserved, and a marked block is never added twice. Versions
that installed `q` as a profile alias migrate it to the launcher and remove the
old marked alias block.

Installed scripts start with a marker containing the Quinjet version that
generated them. Normal startup reads only this line. It rewrites a script after
the version changes and leaves a current script alone. `quinjet update` runs the newly replaced executable to refresh the
active shell immediately.

Installed-once records live under `$XDG_STATE_HOME/quinjet`, else
`~/.local/state/quinjet`, or `%LOCALAPPDATA%\Quinjet\state` on Windows. Once
the relevant record exists, automatic maintenance treats a missing completion
script, completion profile block, or `q` launcher as a user choice. Neither
startup nor an update recreates it. Running
`quinjet completions <SHELL> --install` explicitly recreates missing files and
the launcher. This keeps updates current without fighting a user who removes
the integration.

Without `--install`, the command still writes the raw generated script to
stdout for package maintainers and custom layouts:

```bash
quinjet completions bash > /usr/share/bash-completion/completions/quinjet
quinjet completions fish > ./quinjet.fish
```

`--json` shape, the shell you asked for and the script as one string:

```json
{
  "shell": "bash",
  "script": "_quinjet() {\n    local i cur prev opts cmd\n..."
}
```

The install form reports the shell and every script path. Under `--json` it is
one object:

```json
{
  "shell": "bash",
  "shortcut": "q",
  "paths": [
    "/home/you/.local/share/bash-completion/completions/quinjet",
    "/home/you/.local/bin/q"
  ]
}
```

Examples:

```bash
quinjet completions zsh
quinjet completions fish --install
quinjet completions --install
quinjet completions bash --json
quinjet completion bash
```

## Where to go next

- [`quinjet man`](./man.md) for the manual pages, generated the same way
- [Generated references](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
