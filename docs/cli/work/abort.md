# `quinjet work abort`

Removes a session's worktree and branch and forgets the session.

Usage:

```bash
quinjet work abort <session> [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SESSION>` | string | required | The session identifier. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--yes` | flag | off | Confirms. Without it the command reports what it would remove and stops. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

```console
$ quinjet work abort w42-1
Would abandon w42-1 and delete quinjet/work/w42-1
  removing /home/you/acme-work-42
  2 checkpoint commit(s) would go with the branch
The pull request is not touched.
Pass --yes to remove it.
```

The preview counts the checkpoint commits, because those are the part that
cannot be got back. A session whose work you want to keep should be pushed
first; `git push origin quinjet/work/<id>` from the worktree is enough, and
[`quinjet work publish`](./publish.md) prints that line for you.

`--yes` runs `git worktree remove --force` and then deletes the branch, and
forgets the record. Uncommitted changes in the worktree go with it, which is
what `--force` means and why the preview exists.

The pull request is untouched. A session was never anything GitHub knew about,
so abandoning one leaves no trace there: no comment, no closed thread, no
deleted remote branch.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The preview was printed, or the session was removed. |
| 1 | Git refused to remove the worktree. |
| 3 | No session has that identifier. |

## Where to go next

- [`quinjet work publish`](./publish.md) to keep the work instead
- [`quinjet work list`](./list.md) for what is left
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
