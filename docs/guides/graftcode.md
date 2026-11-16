# Quinjet and Graftcode

[Graftcode](https://graftcode.com) hosts plain public methods on a gateway
(`gg`) and makes them callable from other languages and processes: as MCP
tools at `/mcp`, as typed client packages ("Grafts") installed from npm, PyPI,
NuGet, Maven, and RubyGems, and through its Vision console. Quinjet grafts in
both directions.

```text
 MCP clients, typed Grafts, Vision                Quinjet terminal and CLI
              │                                             │
              ▼                                             │ graft call / palette
   gg  ◄── quinjet graft serve                              ▼
   │   hosts a receiver generated                  any Graftcode gateway
   │   from Quinjet's clap tree                    e.g. graftcode/agent (Python)
   ▼                                                        │
 quinjet <verb> --json  ◄─────────── reads through ─────────┘
 on the served repository            Quinjet's MCP tools
```

- **Quinjet as a Graft.** `quinjet graft serve` turns every read command into
  a gateway method bound to one repository. Agents get MCP tools, other
  languages get typed packages, and nobody writes an API.
- **Grafts inside Quinjet.** `quinjet graft call` and the terminal palette call
  any registered gateway's tools and fill their parameters from the repository:
  the staged patch, the branch, recent commits, the selected pull request, and
  the failing check's log.
- **The agent.** [`graftcode/agent`](../../graftcode/agent) is a Python receiver
  whose methods draft commit messages, review patches, explain failed checks,
  and triage a pull request by calling Quinjet's own grafted tools in a loop.

## Install the gateway

```bash
curl -fsSL https://grft.dev/get | sh
```

On Windows, run `iwr grft.dev/get | iex`. Binaries are also published at
<https://github.com/grft-dev/graftcode-gateway/releases>. Quinjet looks for `gg`
on `PATH`, then in `QUINJET_GRAFTCODE_GATEWAY`, and `--gateway` overrides both.
The gateway bundles its own Node.js runtime, so `graft serve` needs nothing
else.

## Serve a repository

```bash
quinjet graft serve --register quinjet
```

```text
Quinjet is grafted for /home/me/code/project

  MCP        http://localhost:7381/mcp
  Vision     http://localhost:7381/GV
  WebSocket  ws://localhost:7381/ws
  Tools      23 (read-only; pass --allow-writes for mutations)
  Registered `quinjet` for quinjet graft call
```

Quinjet writes the receiver into its cache directory, starts `gg` against it,
waits for the gateway, and confirms that its MCP endpoint lists every generated
tool before printing anything. The gateway's own log stays in the cache
directory beside the module.

| Option | Effect |
| --- | --- |
| `--port <PORT>` | WebSocket and MCP port, 7381 by default. Graftcode uses the next port for its HTTP server. |
| `--allow-writes` | Also exposes commands that change the repository or a pull request. They keep their preview-until-`yes` behavior. |
| `--publish` | Sends the method signatures to grft.dev so Vision shows install commands for typed Grafts. Without it nothing leaves the machine. |
| `--register <NAME>` | Registers the gateway for `graft call` once it is ready. |
| `--gateway <PATH>` | Uses this `gg` executable. |

### Connect an agent

Any MCP client that speaks streamable HTTP can use the endpoint:

```json
{"mcpServers": {"quinjet": {"url": "http://localhost:7381/mcp"}}}
```

Tools are named after command paths: `status`, `diff`, `log`, `show`,
`search`, `branchList`, `branchCompare`, `stashList`, `stashShow`,
`worktreeList`, `repos`, `accountList`, `prView`, `prFiles`, `prCommits`,
`prDiff`, `prConversation`, `prChecks`, `prLogs`, `prReviewsShow`,
`stackView`, `stackFiles`, and `stackDiff`. Each answers with the same JSON
document as `quinjet <command> --json`, and each description comes from the
command's own help text, so the tools cannot drift from the CLI.

### Call Quinjet from another language

With `--publish`, the gateway log and Vision list install commands for every
package manager. A Node.js script then calls Quinjet over Graftcode's binary
WebSocket transport:

```javascript
import { Quinjet, GraftConfig } from "@graft/npm-quinjet-graft_<id>";

GraftConfig.host = "ws://localhost:7381/ws";
const status = JSON.parse(await Quinjet.status());
const commits = JSON.parse(await Quinjet.log("", "", "3"));
```

Copy the exact package name from Vision; Graftcode generates it.

### Hosting the receiver elsewhere

`quinjet graft module --out <DIR>` writes `package.json` and `index.js`
without starting anything, for a container or another host:

```bash
quinjet graft module --out ./quinjet-graft
QUINJET_GRAFT_REPO=/srv/project gg --runtime nodejs --modules ./quinjet-graft/index.js --types Quinjet
```

The module runs `quinjet` from `QUINJET_GRAFT_BIN` or `PATH` against
`QUINJET_GRAFT_REPO`.

## Call gateways from Quinjet

```bash
quinjet graft add agent http://localhost:7390
quinjet graft tools
quinjet graft call agent draft_commit_message
quinjet graft call agent explain_check_failure --pr 42
quinjet graft call agent investigate_pull_request --pr 42
```

`graft add` accepts a gateway, Vision, WebSocket, or MCP address and stores
the MCP endpoint. `--token-env NAME` sends the value of that environment
variable as a bearer token; the token itself is never stored.

Gateways are reached over plain HTTP. Quinjet links no TLS library, which keeps
it a single dependency-free binary on every platform, so a gateway on another
machine is reached through an SSH tunnel:

```bash
ssh -N -L 7390:localhost:7390 build-box &
quinjet graft add agent http://localhost:7390
```

`graft call` fills every parameter it recognizes and sends an empty string for
the rest, because Graftcode binds arguments by position when one is missing.
Explicit `--arg name=value` pairs always win, and `--no-context` sends only
those.

| Parameter | Filled with |
| --- | --- |
| `repo_path` | the repository root |
| `branch` | the checked-out branch |
| `staged_diff` | the staged patch |
| `working_diff` | the unstaged patch |
| `diff` | the staged patch, or the unstaged one when nothing is staged |
| `recent_commits` | the last 20 commit subjects on `HEAD` |
| `pull_request` | the number given with `--pr` |
| `check_log` | the log of `--check`, or of the first failing check on `--pr` |

Names match in any case style, so `stagedDiff` and `staged-diff` work too.
Patches and logs are capped at 256 KiB.

### In the terminal interface

Open the command palette with `:` or `Ctrl+P` and choose **Run a Graftcode
Gateway Tool…**. Quinjet asks every registered gateway for its tools only
then. Enter runs the selected tool with the same context, plus the pull request
on screen and the log of the check that is open. Calls run on their own worker
lane, so a model that thinks for a minute never delays a refresh. A drafted
commit message opens in the commit editor with Enter, `c` copies any answer,
and an answer that arrives behind another dialog becomes a toast.

## The agent

The agent needs [uv](https://docs.astral.sh/uv/), the gateway, and Anthropic
API credentials in the environment (`ANTHROPIC_API_KEY`, or a profile from
`ant auth login`).

```bash
graftcode/agent/serve.sh
quinjet graft add agent http://localhost:7390
```

`serve.sh` syncs the locked environment, points the gateway at that
interpreter's `libpython`, and serves on `QUINJET_AGENT_PORT` (7390 by
default).

| Tool | What it does |
| --- | --- |
| `draft_commit_message` | Writes a Conventional Commits message from the staged patch, matching the recent subjects. |
| `review_diff` | Reports bugs and risky changes in the patch, most severe first. |
| `explain_check_failure` | Names the failing step, the error, the root cause, and the fix. |
| `investigate_pull_request` | Calls Quinjet's grafted read tools in a loop until it can give a verdict with evidence. |

Every parameter of every agent tool is one Quinjet fills, so none of them
needs an argument. The investigation reads `QUINJET_GRAFT_URL`
(`http://localhost:7381/mcp` by default) and only ever calls an allowlist of
read tools, even when the gateway serves writes.

Requests use `claude-opus-5-5` with server-side refusal fallbacks enabled.
`QUINJET_AGENT_MODEL` and `QUINJET_AGENT_EFFORT` override the model and effort.

## Security

- `gg` listens on every interface and has no authentication of its own. Serve
  read-only unless you trust the network, and reach gateways on other machines
  through an SSH tunnel rather than exposing them.
- Read tools never take a confirmation. With `--allow-writes`, destructive
  commands still only preview until a caller passes `yes`.
- Each served gateway is bound to one repository through the environment of
  the process; callers cannot point it at another path.
- `--publish` sends method names, parameter names, and help text to grft.dev.
  Repository content never leaves the machine through Graftcode.

## Troubleshooting

| Symptom | Cause |
| --- | --- |
| `the Graftcode gateway gg is not on PATH` | Install it, or pass `--gateway`. |
| `the gateway was not ready after 180 seconds` | The first start extracts runtimes. Read the log path printed with the error, or raise `--startup-timeout`. |
| `Cannot find Python installed on this machine` | The agent was started without `serve.sh`. Set `HYPERTUBE_PYTHON_RUNTIME_PATH` to a `libpython` file. |
| A tool answers for the wrong argument | A caller left a parameter out. Send every parameter; `graft call` does this for you. |
