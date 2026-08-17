# Generated references

Three verbs generate references rather than reading a repository: shell
completion scripts, manual pages, and machine-readable capabilities. All are
generated on demand from the same command definition the parser itself runs.
Completion scripts and the `q` shortcut for `quinjet` are installed
automatically; existing completion scripts are refreshed after a binary
change. Manual pages and capabilities remain output-only. None needs a
repository or `git`, so all work anywhere.

## Commands

- [`quinjet completions`](./completions.md): a completion script for bash, zsh,
  fish, elvish, or PowerShell, plus the `q` launcher.
- [`quinjet man`](./man.md): the manual page, or one page per command.
- [`quinjet capabilities`](./capabilities.md): the installed command and argument schema.

## Why they are generated

A completion list and a manual are the two documents that rot fastest, because
nothing breaks when they fall behind. Here they are rendered from the clap
command tree at the moment you ask, so a verb added in `src/cli/mod.rs` is
offered by your shell and documented in `man` without anyone remembering to
update a second file.

```bash
quinjet completions zsh --install
quinjet man --dir ~/.local/share/man/man1
```

## Installing the output

`completions --install` writes to user-owned shell directories, adds a marked
completion block where a profile is needed, and places the `q` launcher on
`PATH`. The release scripts invoke its automatic mode directly, and
other installation methods invoke the same maintenance path on first run.
Installed-once state prevents later updates from restoring any script, block,
or launcher the user removed.
Without `--install`, completion generation still writes to stdout for packagers
and custom layouts. `man` writes to stdout or to the directory named with
`--dir`; capabilities writes to stdout.
