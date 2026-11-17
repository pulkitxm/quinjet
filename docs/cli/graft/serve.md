# `quinjet graft serve`

Hosts one repository's Quinjet commands on a Graftcode gateway in the
foreground and keeps it running until interrupted. Agents do not need it; they
use [`quinjet mcp`](../mcp/README.md) and the shared gateway. `serve` is for
typed Grafts, for hosting on another machine, and for exposing write commands.

```bash
quinjet graft serve
quinjet graft serve --register quinjet --port 7400
quinjet graft serve --allow-writes
quinjet graft serve --publish --json
```

Quinjet generates a Node.js receiver from its own command tree, writes it to
the cache directory, starts `gg` against it with `QUINJET_GRAFT_BIN` and
`QUINJET_GRAFT_REPO` set, and waits for the gateway. It then opens an MCP
session, lists the tools, and only then prints the MCP, Vision, and WebSocket
addresses. With `--json` the same facts are one document, printed once the
gateway is ready.

| Option | Default | Effect |
| --- | --- | --- |
| `--port <PORT>` | `7381` | WebSocket and MCP port. The gateway's HTTP server takes the next port. |
| `--allow-writes` | off | Adds every command that changes the repository or a pull request. Destructive ones keep their `yes` preview. |
| `--gateway <PATH>` | `QUINJET_GRAFTCODE_GATEWAY`, `gg` on `PATH`, then Quinjet's download | The gateway executable. When none is configured or found, Quinjet downloads it. |
| `--publish` | off | Uploads method signatures to grft.dev so typed Grafts can be installed. |
| `--register <NAME>` | unset | Registers the running gateway for `graft call`. |
| `--startup-timeout <SECONDS>` | `180` | How long to wait for the first start. |

Host-only commands are never exposed: the terminal interface, account
switching, browser opening, completions, manual pages, updates, recent
projects, and `graft` itself.

Every method is asynchronous and takes two trailing optional arguments,
`repoPath` and `accessToken`; a `serve` gateway answers for its own repository
and needs no token.

Exit code 1 means the gateway could not be found, failed to start, or exited
with an error.
