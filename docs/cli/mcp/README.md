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
| `--read-only` | Lists and runs only the 27 tools that read: the 23 reading commands and the four `ai` tools. Nothing stages, commits, pushes, merges, or comments. |

## The tools

Every Quinjet command that works on a repository or a pull request is a tool:
94 in all. The name is the command path in camel case (`pr checks` is
`prChecks`, `pr reviews resolve` is `prReviewsResolve`), every flag and
positional is a property of the input schema, and the answer is the command's
`--json` document.

Tools carry MCP annotations so agents can decide what needs a confirmation:

| Annotation | Tools |
| --- | --- |
| `readOnlyHint: true` | The 23 reading commands: status, diffs, history, search, branches, stashes, worktrees, pull requests, checks, logs, review threads, and stacks |
| `destructiveHint: true` | The 47 commands that take `yes`, such as `discard`, `branchDelete`, `stashClear`, `prMerge`, `prClose`, and `stackSync` |
| Neither | Changes that are easy to undo, such as `stage`, `commit`, `branchCreate`, `prComment`, and `prReviewsComment`, and the four `ai` tools |

A tool that takes `yes` never acts on the first call. Without `yes` it answers
with what it would do, for example
`Would delete feat/old. Pass --yes to delete it.`, and the agent calls it again
with `yes` set to `true` to go ahead. Interactive commands such as
`account switch`, `pr open`, and the terminal interface are never tools.

Errors come back as tool results with `isError` set, never as a crash: a
folder outside Git, a missing gateway, an unknown tool, or a command that
failed all explain themselves in text. Progress such as the first download is
written to standard error, which agents keep in their logs.

The [Graftcode guide](../../guides/graftcode.md) describes the shared gateway.
