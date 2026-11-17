# `quinjet ai`

Asks Claude about the staged changes or a pull request. Each command is also a
tool of the shared Graftcode gateway, so agents connected through
[`quinjet mcp`](../mcp/README.md) can call it.

```bash
quinjet ai commit-message
quinjet ai review --json
quinjet ai explain-check 42 --check clippy
quinjet ai investigate 42
pbpaste | quinjet ai key
```

| Command | What it sends | What comes back |
| --- | --- | --- |
| `commit-message` | The staged patch, the branch, and the last 20 commit subjects | A Conventional Commits message that follows the history |
| `review` | The staged patch, or the unstaged one when nothing is staged | Findings as `path:line - problem - why it matters`, or `No findings.` |
| `explain-check <NUMBER>` | The log of `--check`, or of the first failing check | Failing step, error, root cause, and fix |
| `investigate <NUMBER>` | Nothing up front; Claude reads the pull request through Quinjet's read commands | Verdict, evidence, root cause, and next steps |

`investigate` gives Claude Quinjet's 23 read commands as tools, runs each one it
asks for against this repository, and stops after 16 rounds at the latest.

## The API key

Quinjet reads `ANTHROPIC_API_KEY`, then the key saved by `quinjet ai key`.
`quinjet ai key` reads one line from standard input and stores it in
`anthropic.json` in Quinjet's state directory with owner-only permissions.
`quinjet ai key --clear` forgets it. The key reaches `curl` through standard
input, never as a command-line argument.

| Variable | Effect |
| --- | --- |
| `QUINJET_AI_MODEL` | Model, `claude-opus-5-5` by default |
| `QUINJET_AI_EFFORT` | Effort for every command instead of each command's own |
| `ANTHROPIC_BASE_URL` | API endpoint, for a proxy |

Requests opt into server-side refusal fallbacks. Text output is Claude's
answer; JSON output is `{ "task", "text", "model" }`. Exit code 1 means no key,
nothing to work on, or an API error, with the reason on standard error.
