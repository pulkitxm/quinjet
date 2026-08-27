# `quinjet stack link`

Creates or updates a GitHub stack from branches, pull request numbers, or pull
request URLs ordered bottom to top. At least two members are required.

Usage:

```bash
quinjet stack link <member> <member>... [--base <branch>] [--open] [--git-remote <remote>] [--yes]
```

`--open` marks linked pull requests ready for review. Without `--yes`, Quinjet
prints a preview and changes nothing.

```bash
quinjet stack link 41 42 43 --open --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
