# `quinjet sync`

Pulls the current branch, then pushes it, in that order and in one command.

Usage:

```bash
quinjet sync [-C <DIR>] [--json]
```

Arguments: none. `quinjet sync` takes no positional argument and exits 2 if
given one.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | Repository to run against. Global, so it may appear before or after the verb. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`sync` is not a Git command. It is the two verbs run in sequence, with no extra
logic of its own:

```bash
git -C <root> -c core.quotepath=false pull
```

then the push routine from [`quinjet push`](./push.md), which reads the status
and either runs `git push` or, when the branch has no upstream, checks
`git remote get-url origin` and runs `git push --set-upstream origin HEAD`.
Everything true of [`quinjet pull`](./pull.md) and
[`quinjet push`](./push.md) individually is true here, including the pull being
plain, the push being plain when an upstream exists, and the refusal when there
is no upstream and no `origin`.

The ordering is the point. Pulling first is what turns a rejected
non-fast-forward push into a merge or a rebase you have already done, so `sync`
is the verb for a branch several people share. It is not atomic, and it does not
roll back. If the pull succeeds and the push fails, the pull has still happened,
and running `quinjet sync` again repeats both halves.

A failing pull stops the command before anything is pushed, so a conflict leaves
the working tree mid-merge with nothing sent. Exit code 1 in that case reports
Git's own error, and the conflicted files are visible in `quinjet status` and
fixable with [`quinjet resolve`](../changes/README.md). There is no
`--continue`: finish the merge, then run `quinjet push`.

`--json` shape, an object with a single key. Note that the sentence is neither
of the two the halves would have printed:

```json
{
  "message": "Synchronization complete"
}
```

Because there is only one message and it arrives at the end, a partial `sync`
never prints anything on stdout. Under `--json` that means the document is
written only when both halves succeeded, which is the usual rule that a non-zero
exit leaves stdout empty.

Examples:

```bash
quinjet sync
quinjet sync --json
quinjet sync -C ~/code/project
quinjet commit -m "docs: describe the remote verbs" && quinjet sync
```

```console
$ quinjet sync
Synchronization complete
```

```console
$ quinjet sync
error: Git command failed: CONFLICT (content): Merge conflict in docs/cli/README.md
```

## Where to go next

- [`quinjet fetch`, `pull`, `push`, `sync`, `repos`](./README.md), the rest of
  this group
- [`quinjet pull`](./pull.md) and [`quinjet push`](./push.md), the two halves
- [All `quinjet` commands](../README.md)
