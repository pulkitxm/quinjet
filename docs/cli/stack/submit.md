# `quinjet stack submit`

Pushes every active branch and creates or updates its pull request and GitHub
stack. Quinjet always uses `gh stack submit --auto`, so no editor opens.

Usage:

```bash
quinjet stack submit [--open] [--git-remote <remote>] [--yes]
```

`--open` marks new and existing pull requests ready for review. Without
`--yes`, Quinjet prints a preview and changes nothing.

```bash
quinjet stack submit --open --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
