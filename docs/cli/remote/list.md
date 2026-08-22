# `quinjet remote list`

Lists recent SSH repositories and checks whether each SSH machine is currently
reachable.

```bash
quinjet remote list
quinjet remote list --json
```

Each reachability check uses non-interactive SSH authentication and a
three-second connection timeout. Text output labels each entry `accessible` or
`unavailable` and shows its use count. JSON output contains `target`, `folder`,
`accessible`, and `uses` fields. The terminal machine picker groups folders by
SSH target and orders machines by their total use count.
