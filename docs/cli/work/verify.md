# `quinjet work verify`

Runs one command inside a session's worktree and records how it went, or
re-runs the commands the session has already recorded.

Usage:

```bash
quinjet work verify <session> [--exit-code] [-C <DIR>] [--json] [-- <command>...]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SESSION>` | string | required | The session identifier. |
| `<COMMAND>...` | words after `--` | unset | The command to run. Without one, the commands already recorded are re-run in order. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--exit-code` | flag | off | Exit 1 when any recorded verification has failed. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## No shell

The words after `--` are the program and its arguments, spawned directly. There
is no shell, so no pipe, no redirection, no glob, and no variable expansion.
`quinjet work verify w42-1 -- sh -c 'cargo test | tee log'` is how you get a
shell if you want one, and then it is visibly your decision rather than an
accident of quoting.

The command is stored as an argv array rather than a string for the same reason.
A record that flattened it to `"cargo test"` would imply a shell parsed it, and
that would be a lie about what happened.

The command runs with the session's worktree as its working directory, with
`LC_ALL=C` and `GIT_TERMINAL_PROMPT=0` set. Its output is captured and the tail
is kept on the record, standard error last, so a failure says why without the
record becoming a log file.

## Re-running

`quinjet work verify w42-1` with nothing after `--` re-runs every command the
session has recorded, in the order it recorded them, and replaces each result.

Running a command that is already recorded replaces its row rather than adding a
second one, so a session's verification list is always the latest result for
each distinct command, never a history.

A session that has recorded nothing has nothing to re-run, and says so by
exiting 1. That is deliberate: a session that has run no checks has not
verified anything, which is not the same as passing, and reporting a pass it
did not earn would be the worst possible answer here.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The command ran. Its own exit status is on the record, not here, unless `--exit-code` is given. |
| 1 | With `--exit-code`, some recorded verification has failed. Also when the session has no worktree, or nothing to re-run. |
| 3 | No session has that identifier. |

Without `--exit-code` a failing command is not a failing verb: the command ran,
which is what was asked, and its result is on the record. With `--exit-code` the
verb reports the session's verdict, which is what a script wants.

Examples:

```bash
quinjet work verify w42-1 -- cargo fmt --check
quinjet work verify w42-1 -- cargo test
quinjet work verify w42-1 --exit-code
quinjet work verify w42-1 --json | jq -r '.verifications[] | select(.passed | not) | .command | join(" ")'
```

## Where to go next

- [`quinjet work publish`](./publish.md), which names a failing verification rather than hiding it
- [`quinjet work inspect`](./inspect.md) for the whole record
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
