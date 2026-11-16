# Quinjet and Graftcode

[Graftcode](https://graftcode.com) hosts plain methods on a gateway (`gg`) and
makes them callable from anywhere: as MCP tools for AI agents, as typed client
packages ("Grafts") for npm, PyPI, NuGet, Maven, and RubyGems, and from its
Vision console. Quinjet uses it to put its Git and GitHub commands in front of
every agent on the machine, without anyone starting a server by hand.

```text
 Claude Code, Cursor, Codex,           Quinjet terminal
 VS Code, Windsurf, Gemini CLI         (opens the gateway on launch)
          │ starts                                │
          ▼                                       ▼
   quinjet mcp  ──── MCP over HTTP ────►  shared Graftcode gateway (gg)
   (stdio, one per agent session)         one per machine, 27 tools
                                                  │
                                                  ▼
                                   quinjet <command> --json
                                   in the agent's repository
```

## Connect your agents once

Open the command palette (`:` or `Ctrl+P`) and choose **Connect AI Agents to
Quinjet**, or run:

```bash
quinjet mcp install
```

Quinjet finds the agents installed on this machine and adds itself to each one:

| Agent | Where the entry goes |
| --- | --- |
| Claude Code | `claude mcp add --scope user quinjet -- quinjet mcp` |
| Codex | `[mcp_servers.quinjet]` in `~/.codex/config.toml` |
| Cursor | `mcpServers.quinjet` in `~/.cursor/mcp.json` |
| VS Code | `servers.quinjet` in the user `mcp.json` |
| Windsurf | `mcpServers.quinjet` in `~/.codeium/windsurf/mcp_config.json` |
| Gemini CLI | `mcpServers.quinjet` in `~/.gemini/settings.json` |

Every entry is the same command, `quinjet mcp`. Running the install again
reports `already connected` and changes nothing. `--agent <name>` limits it to
one agent, and `--print` prints the entry to add by hand anywhere else.

That is the whole setup. From then on an agent starts `quinjet mcp` by itself
when it needs a tool, in whatever repository it is working in, whether
Quinjet's terminal interface is open or not.

## What happens when an agent calls a tool

1. The agent launches `quinjet mcp`. It answers the MCP handshake and the tool
   list immediately from Quinjet's own command tree.
2. On the first tool call, `quinjet mcp` looks for the shared gateway. If none
   is running, it starts one in the background. If `gg` is not installed yet,
   it downloads the release for this platform from GitHub into Quinjet's cache,
   checks it against the published SHA-256 digest, and unpacks it. Nothing is
   installed system-wide and nothing needs `sudo`.
3. The call goes to the gateway over MCP, with the agent's folder as the
   repository. The gateway runs `quinjet <command> --json` there and answers
   with the same JSON the CLI prints.

A warm start takes about two seconds and later calls take tens of
milliseconds. The gateway serves every repository and every agent session at
once; calls run concurrently.

### The tools

| Tools | Commands |
| --- | --- |
| `status`, `diff`, `log`, `show`, `search` | Working tree, patches, history, and search |
| `branchList`, `branchCompare`, `stashList`, `stashShow`, `worktreeList` | Branches, stashes, and worktrees |
| `repos`, `accountList` | GitHub repositories and accounts |
| `prView`, `prFiles`, `prCommits`, `prDiff`, `prConversation`, `prChecks`, `prLogs`, `prReviewsShow` | Pull requests, checks, and check logs |
| `stackView`, `stackFiles`, `stackDiff` | Stacked pull requests |
| `aiCommitMessage`, `aiReview`, `aiExplainCheck`, `aiInvestigate` | Quinjet's own Claude features, described below |

Every tool takes an optional `repository` path, for agents that are not started
inside the repository they mean. Descriptions come from each command's help
text, so the tools cannot drift from the CLI.

## Inside Quinjet

The terminal interface starts the shared gateway when it opens (when `gg` is
already installed) and holds it open while it runs. The status bar shows
`Graftcode ●` while the gateway is up and `Graftcode ○` when it could not
start. The palette has:

| Palette entry | What it does |
| --- | --- |
| Draft Commit Message with Claude | Writes a Conventional Commits message for the staged patch; `Enter` opens it in the commit editor |
| Review Changes with Claude | Reviews the staged patch, or the unstaged one, most severe finding first |
| Explain Failing Check with Claude | Explains the check whose log is open on the Pull Requests tab |
| Investigate Pull Request with Claude | Triages the selected pull request by reading its checks, logs, diff, and conversation |
| Connect AI Agents to Quinjet | Runs `quinjet mcp install` and prepares the gateway |
| Show Graftcode Gateway | The Vision, MCP, and WebSocket addresses, the log, and connected clients |
| Run a Tool from Another Graftcode Gateway… | Calls a gateway registered with `quinjet graft add` |

In the commit editor, `Ctrl+G` drafts the message in place. Claude's answers
arrive on their own worker lane, so a slow model never delays a refresh, and
`Esc` cancels a request that is still running.

## Claude in Quinjet

The same four features are CLI commands, and therefore gateway tools too:

```bash
quinjet ai commit-message
quinjet ai review
quinjet ai explain-check 42
quinjet ai explain-check 42 --check "Build (linux)"
quinjet ai investigate 42
```

They need an Anthropic API key. Quinjet reads `ANTHROPIC_API_KEY`, or a key
saved with `quinjet ai key`, which matters for agents that start Quinjet
without your shell's environment:

```bash
pbpaste | quinjet ai key
quinjet ai key --clear
```

The key is stored in `anthropic.json` in Quinjet's state directory with
owner-only permissions, and it reaches `curl` through standard input, never on
a command line. Requests use `claude-opus-5-5` with server-side refusal
fallbacks. `investigate` runs a tool loop over Quinjet's read commands only, at
most 16 rounds, with prompt caching. `QUINJET_AI_MODEL`, `QUINJET_AI_EFFORT`,
and `ANTHROPIC_BASE_URL` override the model, the effort, and the endpoint.

## Managing the shared gateway

```bash
quinjet graft status
quinjet graft start
quinjet graft stop
quinjet graft install
```

`status` prints the addresses, the tool count, and how many clients hold the
gateway open. `start` brings it up without an agent, downloading `gg` if
needed. `stop` shuts it down; the next agent call starts it again. `install`
only downloads `gg`.

The gateway stops ten minutes after the last terminal window or agent session
lets go of it. Each client renews a lease file every 30 seconds, and every tool
call refreshes an activity stamp. If Quinjet is upgraded, the next client
notices the old gateway runs another build and replaces it.

Quinjet looks for `gg` in `--gateway`, then `QUINJET_GRAFTCODE_GATEWAY`, then
`PATH`, then its own download. A configured path that does not exist is an
error rather than a reason to download.

## Typed Grafts

`quinjet graft serve` is the foreground, single-repository gateway, for typed
clients and for hosting on another machine:

```bash
quinjet graft serve --publish
```

With `--publish`, Graftcode uploads the method signatures (never repository
content) to grft.dev, and Vision shows install commands for every package
manager. A Node.js script then calls Quinjet over Graftcode's binary WebSocket
transport:

```javascript
import { Quinjet, GraftConfig } from "@graft/npm-quinjet-graft_<id>";

GraftConfig.host = "ws://localhost:7381/ws";
const status = JSON.parse(await Quinjet.status());
const commits = JSON.parse(await Quinjet.log("", "", "3"));
```

Every method is asynchronous and takes two trailing optional arguments,
`repoPath` and `accessToken`, which `graft serve` does not need.
`quinjet graft module --out <DIR>` writes the module without starting anything,
for a container or another host.

## Other gateways

`quinjet graft add`, `tools`, and `call` work with any Graftcode gateway, and
fill parameters such as `staged_diff`, `branch`, `pull_request`, and
`check_log` from the repository. See [`quinjet graft call`](../cli/graft/call.md).

## Security

- `gg` listens on every network interface and has no authentication of its
  own. The shared gateway therefore requires an access token that changes on
  every start and lives in a file only your account can read. A call without it
  is refused, so other machines on the network can see the Vision page but
  cannot run a command.
- The shared gateway exposes read commands and the four Claude features only.
  Nothing it serves changes a repository or a pull request.
- `graft serve` has no token. Run it read-only unless you trust the network,
  and reach it from another machine through an SSH tunnel.
- The Claude features send the patch or log they work on to the Anthropic API.
  Graftcode itself never receives repository content.

## Troubleshooting

| Symptom | Cause |
| --- | --- |
| A tool answers `no Graftcode gateway at …` | `QUINJET_GRAFTCODE_GATEWAY` or `--gateway` names a missing file. Unset it to let Quinjet download `gg`. |
| `the Graftcode gateway was not ready after 180 seconds` | The first start unpacks runtimes. Read the log that `quinjet graft status` names. |
| `this Quinjet gateway is shared; connect through quinjet mcp` | A client called the shared gateway directly. Use `quinjet mcp`, or `graft serve` for direct access. |
| `no Anthropic API key` | Run `quinjet ai key`, or set `ANTHROPIC_API_KEY`. |
| `… is not inside a Git repository; pass repository` | The agent started outside a repository. Pass the `repository` argument. |
| The status bar shows `Graftcode ○` | `gg` is not installed yet. Run **Connect AI Agents to Quinjet**. |
