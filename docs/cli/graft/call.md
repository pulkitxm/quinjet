# `quinjet graft call`

Calls one tool on a registered gateway and prints its answer.

```bash
quinjet graft call agent draft_commit_message
quinjet graft call agent explain_check_failure --pr 42 --check clippy
quinjet graft call quinjet log --arg limit=5
quinjet graft call agent review_diff --no-context --arg diff="$(git diff main)"
```

Before calling, Quinjet lists the gateway's tools, resolves the tool name in
any case style, and fills every parameter whose name it recognizes:

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

Every other parameter is sent as an empty string. Graftcode binds arguments by
position when one is missing, so a call always carries all of them.

| Option | Effect |
| --- | --- |
| `-a, --arg <NAME=VALUE>` | Sets one argument. JSON numbers and booleans keep their type. Repeatable. |
| `--pr <NUMBER>` | Fills `pull_request` and makes `check_log` available. |
| `--check <NAME>` | Chooses the check whose log fills `check_log`. |
| `--no-context` | Sends only the `--arg` values. |
| `--timeout <SECONDS>` | How long to wait for the answer, 600 by default. |

Text output is the tool's answer. JSON output also lists which parameters were
`filled` and which were sent `empty`, the tool's `isError` flag, and any
structured content.

| Exit code | Meaning |
| --- | --- |
| 0 | The tool answered. |
| 1 | The tool reported an error, or the arguments were rejected. |
| 3 | No such gateway, tool, or failing check. |
| 4 | The gateway could not be reached. |
