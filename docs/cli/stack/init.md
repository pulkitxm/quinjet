# `quinjet stack init`

Initializes a local stack from branches ordered bottom to top. Existing branches
are adopted and missing branches are created by `gh stack`.

Usage:

```bash
quinjet stack init <branch>... [--base <branch>] [--yes]
```

Without `--yes`, Quinjet prints a preview and changes nothing. `--base` selects
a trunk other than the repository default.

Examples:

```bash
quinjet stack init api ui
quinjet stack init api ui --base develop --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
