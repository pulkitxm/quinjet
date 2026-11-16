# `quinjet mcp`

Serves Quinjet to an AI agent over the Model Context Protocol on standard input
and output. Agents start it themselves; nobody runs it in a terminal.

```bash
quinjet mcp install
quinjet mcp install --agent claude-code
quinjet mcp install --print
```

| Page | What it covers |
| --- | --- |
| [`quinjet mcp install`](./install.md) | Add `quinjet mcp` to the agents installed on this machine |

## The server

`quinjet mcp` answers `initialize`, `ping`, and `tools/list` at once from
Quinjet's command tree, so an agent never waits to see the tools. The first
`tools/call` makes sure the shared Graftcode gateway is running, starting it in
the background and downloading `gg` into the cache when needed, and every call
then goes through that gateway.

The repository is the folder the agent started `quinjet mcp` in, or `-C <DIR>`.
Every tool also accepts a `repository` argument for a different checkout.
Calls run concurrently, and the session holds the gateway open until the agent
closes standard input.

| Option | Effect |
| --- | --- |
| `--gateway <PATH>` | Uses this `gg` instead of `QUINJET_GRAFTCODE_GATEWAY`, `PATH`, or the download. |

Errors come back as tool results with `isError` set, never as a crash: a
folder outside Git, a missing gateway, an unknown tool, or a command that
failed all explain themselves in text. Progress such as the first download is
written to standard error, which agents keep in their logs.

The [Graftcode guide](../../guides/graftcode.md) describes the shared gateway.
