# `quinjet stack modify`

Aborts or continues an existing stack modification. Quinjet does not expose the
interactive modification editor.

Usage:

```bash
quinjet stack modify <--abort|--continue> [--yes]
```

Exactly one recovery action is required. Without `--yes`, Quinjet prints a
preview and changes nothing.

```bash
quinjet stack modify --continue --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
