# `quinjet stash drop`

Deletes one stash without applying it, and only with `--yes`.

Usage:

```bash
quinjet stash drop <REFERENCE> [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<REFERENCE>` | `stash@{N}` | required | The stash to delete. Required; there is no "newest" shortcut and no `--all`. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--yes` | flag | off | Confirms the deletion. Without it the verb reports what it would drop and changes nothing. |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Global. |
| `--json` | flag | off | Prints the sentence as one JSON object instead of a line of text. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Without `--yes` the verb never reaches Git. It prints one sentence, exits 0,
and does not even check that the reference exists or has the right shape, so
the dry run is a statement of intent rather than a validation:

```console
$ quinjet stash drop 'stash@{0}'
Would drop `stash@{0}`. Pass --yes to drop it.
```

The exit code is 0, which is the rule the whole command line follows: a missing
confirmation is a deliberate no-op, never an error to handle. It is the same
`--yes` that guards [`quinjet discard`](../changes/README.md) and
[`quinjet branch delete`](../branch/README.md), described once in
[conventions and contracts](../conventions.md).

With `--yes` the reference is validated against the `stash@{N}` shape and then
handed to Git:

```bash
git stash drop stash@{0}
```

A reference of the wrong shape fails before Git runs, with
`refusing to use an invalid stash reference` and exit 1. A well-formed
reference that does not exist reaches Git and fails there:

```console
$ quinjet stash drop 'stash@{9}' --yes
error: Git command failed: error: stash@{9} is not a valid reference
```

That is exit 1 as well. The dry run of the same command would have printed its
Would drop sentence for `stash@{9}` and exited 0, so the dry run is not a way
to test whether an entry exists. [`stash list`](./list.md) is.

Dropping renumbers everything after the entry that went: with three stashes,
dropping `stash@{1}` makes the old `stash@{2}` the new `stash@{1}`. A script
deleting several entries should therefore work from the highest index down, or
re-read the list between deletions.

A dropped stash is unreferenced rather than erased, so `git fsck --unreachable`
plus `git stash store` can sometimes recover it until Git next prunes. Quinjet
offers nothing for that, and offers no undo.

`--json` shape, an object with one key. It carries whichever sentence the run
produced, so a caller has to read the text to tell a refusal from a deletion:

```json
{
  "message": "Would drop `stash@{0}`. Pass --yes to drop it."
}
```

```json
{
  "message": "Dropped stash@{0}"
}
```

Examples:

```bash
quinjet stash drop 'stash@{0}'
quinjet stash drop 'stash@{0}' --yes
quinjet stash drop 'stash@{2}' --yes --json
quinjet stash drop 'stash@{1}' --yes -C ~/code/project
```

```console
$ quinjet stash drop 'stash@{0}' --yes
Dropped stash@{0}
```

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash show`](./show.md) to read an entry before deleting it
- [`quinjet stash clear`](./clear.md) to delete all of them at once
- [All `quinjet` commands](../README.md)
