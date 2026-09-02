# `quinjet work diff`

Prints what a session has changed since the commit it started at.

Usage:

```bash
quinjet work diff <session> [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SESSION>` | string | required | The session identifier. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The base is the object name the session recorded when it started, not the
branch's current tip and not the pull request's current head. A push landing on
the pull request while the session is open changes neither, so what this prints
is always the session's own work.

The comparison is `git diff <start oid>` inside the session's worktree, which
means it covers changes the session has not staged as well as ones it has. A
session's work is whatever is in its worktree; committing is a separate step
that [`quinjet work publish`](./publish.md) does.

Untracked files do not appear here, because `git diff` does not report them.
[`quinjet work publish`](./publish.md) does list them, and commits them, so the
publish preview is the complete picture of what would be recorded.

`--json` shape, one object:

```json
{
  "id": "w42-1",
  "startOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
  "files": ["src/lib.rs"],
  "patch": "diff --git a/src/lib.rs b/src/lib.rs\n...",
  "truncated": false
}
```

A session that has changed nothing prints one line saying so rather than an
empty patch, and returns an empty `files` array.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The diff was printed, including when it is empty. |
| 1 | The session has no worktree, or its worktree has been deleted. |
| 3 | No session has that identifier. |

## Where to go next

- [`quinjet work publish`](./publish.md) to record it
- [`quinjet work inspect`](./inspect.md) for the session itself
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
