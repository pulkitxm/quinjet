# `quinjet stash clear`

Deletes every stash in the repository, and only with `--yes`.

Usage:

```bash
quinjet stash clear [--yes] [-C <DIR>] [--json]
```

Arguments: none. `quinjet stash clear` takes no positional argument, because it
is all of the stashes or none of them; passing one exits 2.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--yes` | flag | off | Confirms the deletion. Without it the verb counts the stashes, says what it would drop, and changes nothing. |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Global. |
| `--json` | flag | off | Prints the sentence as one JSON object instead of a line of text. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Without `--yes` the verb does one read and no write. It runs the same
`git stash list` that [`stash list`](./list.md) runs, counts the entries it
parsed, prints the count and exits 0:

```console
$ quinjet stash clear
Would drop 3 stashes. Pass --yes to drop them.
```

The count is the number of entries Quinjet accepted, so a malformed reflog
record that `stash list` would have skipped is not counted here either. The
sentence is not pluralized: one stash produces `Would drop 1 stashes. Pass
--yes to drop them.`, and an empty repository produces `Would drop 0 stashes.
Pass --yes to drop them.`

With `--yes` there is no reference to validate and one Git call to make:

```bash
git stash clear
```

`git stash clear` succeeds on an empty stash list, so `clear --yes` in a
repository with nothing stashed still prints `Dropped all stashes` and exits 0.
There is no way to clear only the stashes of one branch, and no way to keep the
newest: this verb has no filter at all.

Everything it deletes becomes unreferenced rather than erased, so
`git fsck --unreachable` plus `git stash store` can sometimes recover an entry
until Git next prunes. Quinjet offers no undo, and unlike the terminal
interface, which puts a "Permanently delete every stash? This cannot be undone."
confirmation on screen, the command line's only guard is `--yes`.

`--json` shape, an object with one key carrying whichever sentence the run
produced:

```json
{
  "message": "Would drop 3 stashes. Pass --yes to drop them."
}
```

```json
{
  "message": "Dropped all stashes"
}
```

Examples:

```bash
quinjet stash clear
quinjet stash clear --json
quinjet stash clear --yes
quinjet stash clear --yes -C ~/code/project
```

```console
$ quinjet stash clear --yes
Dropped all stashes
```

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash drop`](./drop.md) to delete one entry instead of all of them
- [`quinjet stash list`](./list.md) to see what `clear` would take
- [All `quinjet` commands](../README.md)
