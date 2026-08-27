# `quinjet stack push`

Pushes active branches in the current stack with the lease checks implemented
by `gh stack`.

Usage:

```bash
quinjet stack push [--git-remote <remote>] [--yes]
```

Without `--yes`, Quinjet prints a preview and changes nothing.

```bash
quinjet stack push --git-remote upstream --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
