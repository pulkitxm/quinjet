# Generated references

Two verbs generate references rather than reading a repository: shell
completion scripts and manual pages. Both are generated on demand from the
same command definition the parser itself runs. Neither is installed
automatically, and neither needs a repository or `git`, so both work anywhere.

## Commands

- [`quinjet completions`](./completions.md): a completion script for bash,
  zsh, fish, elvish, or PowerShell.
- [`quinjet man`](./man.md): the manual page, or one page per command.

## Why they are generated

A completion list and a manual are the two documents that rot fastest, because
nothing breaks when they fall behind. Here they are rendered from the clap
command tree at the moment you ask, so a verb added in `src/cli/mod.rs` is
offered by your shell and documented in `man` without anyone remembering to
update a second file.

```bash
quinjet completions zsh > ~/.zfunc/_quinjet
quinjet man --dir ~/.local/share/man/man1
```

## Installing the output

Neither verb installs anything itself: each writes to stdout, or to a
directory you name, so you decide where it lands and nothing runs as root
behind your back. The pages linked above give the path for each shell and for
`man` on Linux and macOS.
