# `quinjet graft start`, `status`, `stop`, and `install`

Manage the shared Graftcode gateway that [`quinjet mcp`](../mcp/README.md) and
the terminal interface start by themselves.

```bash
quinjet graft status
quinjet graft start --json
quinjet graft stop
quinjet graft install
```

| Command | What it does |
| --- | --- |
| `status` | Prints whether the gateway runs, its MCP, Vision, and WebSocket addresses, the tool count, the number of clients holding it, and its log. |
| `start` | Starts the gateway in the background if it is not running, downloading `gg` first when needed. `--gateway <PATH>` picks the executable. |
| `stop` | Asks the gateway to stop and waits for it. The next agent call starts it again. |
| `install` | Downloads `gg` into Quinjet's cache, verified against the release's SHA-256 digest, unless one is already available. |

One gateway serves every repository and agent session on the machine. It
listens on the first free pair of ports from 7381, requires an access token
that changes on every start, and stops ten minutes after the last client lets
go of it. Its state, module, and log live under `graft/service` in Quinjet's
cache directory (`QUINJET_CACHE_DIR` overrides it).
