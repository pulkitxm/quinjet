# `quinjet stash push`

Puts the current changes on the stash and leaves the working tree clean.

Usage:

```bash
quinjet stash push [-m <message>] [--include-untracked | --staged] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| none | | | `push` takes no positional argument. It stashes the repository, not a path. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-m, --message <MESSAGE>` | string | `""` | The message to record. Trimmed before use; a blank or whitespace-only value is dropped and Git writes its own subject. |
| `--include-untracked` | flag | off | Also stash files Git is not tracking, as `git stash push --include-untracked`. Conflicts with `--staged`. |
| `--staged` | flag | off | Stash only what is staged and leave the unstaged working tree alone, as `git stash push --staged`. Conflicts with `--include-untracked`. |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Global. |
| `--json` | flag | off | Prints the result sentence as one JSON object instead of a line of text. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The argv is assembled in exactly that order: `git stash push`, then
`--include-untracked` if asked, then `--staged` if asked, then `--message` and
the trimmed text only when the trimmed text is not empty. So
`quinjet stash push` and `quinjet stash push -m "   "` produce the identical
Git call, and the reflog gets Git's generated `WIP on <branch>: <commit>
<subject>` subject rather than an empty message. Passing a message gives the
`On <branch>: <message>` form instead, which is what [`stash
list`](./list.md) splits back apart.

The three variants are the whole of what `push` can do. There are no
pathspecs, so a partial stash is not expressible; there is no `--keep-index`,
no `--all` for ignored files, and no interactive `--patch`. `--staged` and
`--include-untracked` are declared as conflicting in clap, so
`quinjet stash push --staged --include-untracked` is rejected before any Git
runs:

```text
error: the argument '--staged' cannot be used with '--include-untracked'

Usage: quinjet stash push --staged

For more information, try '--help'.
```

That is a usage error and exits 2.

Two edge cases are worth knowing before scripting this. First, a clean working
tree is not a failure: `git stash push` prints `No local changes to save` and
exits 0, so Quinjet prints `Changes stashed` and exits 0 as well, even though
the stash list did not grow. Check with `quinjet stash list` if the difference
matters. Second, `--staged` requires Git 2.35 or newer; on anything older the
option is rejected by Git and the verb exits 1 carrying Git's message.

On a branch with no commits there is nothing to stash against, and Git says so:

```console
$ quinjet stash push -m "first attempt"
error: Git command failed: You do not have the initial commit yet
```

That is exit 1. On a detached HEAD the push succeeds and the entry records
`(no branch)` as its branch.

`--json` shape, an object with one key, the same sentence the text form prints:

```json
{
  "message": "Changes stashed"
}
```

The sentence is fixed. It does not name the new stash, does not count the files
that moved, and is the same for all three variants, so read
`quinjet stash list --json` afterwards if the caller needs the reference.

Examples:

```bash
quinjet stash push
quinjet stash push -m "launch work"
quinjet stash push --include-untracked -m "before the rebase"
quinjet stash push --staged -m "index only"
quinjet stash push -C ~/code/project --json
```

```console
$ quinjet stash push --include-untracked -m "launch work"
Changes stashed
```

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash pop`](./pop.md) to take the newest one back
- [All `quinjet` commands](../README.md)
