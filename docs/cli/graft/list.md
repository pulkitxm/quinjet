# `quinjet graft list`

Lists registered gateways.

```bash
quinjet graft list
quinjet graft list --json
```

Text output shows each name and MCP endpoint, and the variable a token comes
from when one is configured. JSON output is an array of objects with `name`,
`url`, and an optional `tokenEnv`.
