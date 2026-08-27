# `quinjet stack up`

Moves toward the top of the active stack, skipping merged branches.

```bash
quinjet stack up [<steps>] [--yes]
```

`<STEPS>` defaults to 1 and must be positive. Without `--yes`, Quinjet prints a
preview and does not change the checkout.

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
