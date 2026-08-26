# `quinjet stack checkout`

Checks out a stack by stack number, pull request number, pull request URL, or
locally tracked branch. A target is required so the command never opens the
interactive `gh stack` picker.

Usage:

```bash
quinjet stack checkout <stack|pr|url|branch> [--yes]
```

Without `--yes`, Quinjet prints a preview and does not change the checkout.

```bash
quinjet stack checkout 42 --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
