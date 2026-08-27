# `quinjet stack sync`

Fetches, reconciles, rebases, pushes, and synchronizes the current local and
GitHub stack. `--prune` removes local branches for merged pull requests.

Usage:

```bash
quinjet stack sync [--prune] [--git-remote <remote>] [--yes]
```

Without `--yes`, Quinjet prints a preview and changes nothing. A divergence
that requires an interactive choice is rejected by the noninteractive
transport.

```bash
quinjet stack sync --prune --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
