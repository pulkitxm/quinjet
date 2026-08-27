# `quinjet stack add`

Adds a named branch on top of the active stack. It can also stage and commit
changes without opening an editor.

Usage:

```bash
quinjet stack add <branch> [--all|--update] [--message <text>] [--yes]
```

`--all` stages tracked and untracked files. `--update` stages only tracked
files. Either staging option requires `--message`. Without `--yes`, Quinjet
prints a preview and changes nothing.

```bash
quinjet stack add tests --all --message "Add tests" --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
