# `quinjet stack merge`

Uses GitHub's atomic stack merge for the active stack or a stack or pull request
number. Every pull request through the selected target merges together, or none
of them merge.

Usage:

```bash
quinjet stack merge [<stack|pr>] <--merge|--squash|--rebase> [--yes]
```

The merge method is required. Quinjet passes confirmation to `gh stack` only
after its own `--yes` is present.

```bash
quinjet stack merge 42 --squash --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
