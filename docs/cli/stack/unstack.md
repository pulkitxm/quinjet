# `quinjet stack unstack`

Removes stack tracking locally and on GitHub. With no stack number, it targets
the active stack. `--local` preserves the GitHub stack.

Usage:

```bash
quinjet stack unstack [<stack>] [--local] [--yes]
```

Without `--yes`, Quinjet prints a preview and changes nothing.

```bash
quinjet stack unstack 7 --local --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
