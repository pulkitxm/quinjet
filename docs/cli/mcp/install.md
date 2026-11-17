# `quinjet mcp install`

Adds `quinjet mcp` to every AI agent installed on this machine.

```bash
quinjet mcp install
quinjet mcp install --agent cursor --agent codex
quinjet mcp install --print
quinjet mcp install --read-only
quinjet mcp install --json
```

| Agent | `--agent` | What changes |
| --- | --- | --- |
| Claude Code | `claude-code` | `claude mcp add --scope user quinjet -- quinjet mcp` |
| Codex | `codex` | `[mcp_servers.quinjet]` in `~/.codex/config.toml` |
| Cursor | `cursor` | `mcpServers.quinjet` in `~/.cursor/mcp.json` |
| VS Code | `vs-code` | `servers.quinjet` in the user `mcp.json` |
| Windsurf | `windsurf` | `mcpServers.quinjet` in `~/.codeium/windsurf/mcp_config.json` |
| Gemini CLI | `gemini-cli` | `mcpServers.quinjet` in `~/.gemini/settings.json` |

Without `--agent`, Quinjet configures the agents it finds and skips the rest.
Naming an agent configures it even when its folder does not exist yet.

The command is `quinjet` when that name on `PATH` is this executable, so
upgrades keep working, and the absolute path otherwise. Other entries in a
configuration file are kept. A file that is not plain JSON, such as a VS Code
file with comments, is left untouched and reported, with the entry to add by
hand.

| Outcome | Meaning |
| --- | --- |
| `connected` | The entry was added. |
| `updated` | An older `quinjet` entry was replaced. |
| `already connected` | Nothing changed. |
| `failed` | The reason follows; exit code 1. |

`--print` prints the `mcpServers` entry instead of writing anything.
`--read-only` writes `quinjet mcp --read-only` instead, so the agents only get
the tools that read.
