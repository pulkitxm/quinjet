# `quinjet stash apply`

Applies one stash to the working tree and leaves it in the list.

Usage:

```bash
quinjet stash apply <REFERENCE> [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<REFERENCE>` | `stash@{N}` | required | The stash to apply. Required: unlike [`pop`](./pop.md), `apply` has no "newest" shortcut. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Global. |
| `--json` | flag | off | Prints the result sentence as one JSON object instead of a line of text. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The reference is checked before Git sees it. It has to be `stash@{` followed by
one or more ASCII digits and a closing `}`, and nothing else, which is the same
shape [`stash list`](./list.md) filters its output to. Anything else fails
without running Git:

```console
$ quinjet stash apply 'stash@{HEAD}'
error: refusing to use an invalid stash reference
```

That is exit 1. The check is about shape, not existence: `stash@{9}` passes it
and then fails in Git's own terms with
`error: Git command failed: stash@{9} is not a valid reference`, also exit 1.
A shell will eat the braces, so quote the reference.

What runs is:

```bash
git stash apply --index stash@{0}
```

`--index` is always passed and cannot be turned off. It means the staged half
of the stash goes back into the index rather than arriving as unstaged work, so
what was staged when the stash was taken is staged again. It is also the part
most likely to fail: when the index cannot be restored, for example because
those paths have moved on since, Git fails and the verb exits 1. Git may
already have written some of the change to the working tree by then, so a
failed apply can leave a dirty tree behind. Re-read `quinjet status` rather
than assuming nothing happened.

Applying does not remove the entry, so the same reference stays valid and the
indices do not renumber. That is the difference from [`pop`](./pop.md), and it
is what makes `apply` the right verb for putting one stash onto more than one
branch. A conflict during the apply leaves the conflicted files in the working
tree for [`quinjet resolve`](../changes/README.md), and the stash remains.

The branch recorded in the stash is not consulted. A stash taken on one branch
applies to any other, because Git is merging trees rather than checking a name.

`--json` shape, an object with one key, naming the reference that was applied:

```json
{
  "message": "Applied stash@{0}"
}
```

Examples:

```bash
quinjet stash apply 'stash@{0}'
quinjet stash apply "stash@{2}"
quinjet stash apply 'stash@{0}' --json
quinjet stash apply 'stash@{1}' -C ~/code/project
```

```console
$ quinjet stash apply 'stash@{0}'
Applied stash@{0}
```

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash pop`](./pop.md) for the same thing with the entry removed
- [All `quinjet` commands](../README.md)
