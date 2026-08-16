# `quinjet stash pop`

Applies one stash and drops it, or the newest one when no reference is given.

Usage:

```bash
quinjet stash pop [REFERENCE] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[REFERENCE]` | `stash@{N}` | the newest stash | The stash to pop. Optional: with nothing here, Git pops `stash@{0}`. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Global. |
| `--json` | flag | off | Prints the result sentence as one JSON object instead of a line of text. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`pop` is the only verb in the group whose reference is optional, and the
omission is passed through rather than resolved: Quinjet appends no reference
at all and lets Git default to the newest entry. The two forms are:

```bash
git stash pop --index
git stash pop --index stash@{1}
```

That distinction matters in a repository somebody else is also using. Reading
`stash list` and then popping `stash@{0}` is a race, because a push in another
window renumbers the entries between the two commands. Popping with no
reference is not: whatever the newest entry is at the moment Git runs is the
one that goes. Use the bare form when the intent is "the last thing I
stashed".

When a reference is given it is validated first, exactly as in
[`apply`](./apply.md): `stash@{` then ASCII digits then `}`, or the verb fails
with `refusing to use an invalid stash reference` and exit 1 without running
Git. A well-formed reference that does not exist reaches Git and fails there
instead.

`--index` is always passed and cannot be turned off, so the staged half of the
stash returns to the index. A pop only drops the entry if the application
succeeded, which is Git's behavior rather than Quinjet's: a conflicting pop
leaves the conflicted files in the working tree and keeps the stash, so nothing
is lost, and the conflict is then a job for
[`quinjet resolve`](../changes/README.md). A pop whose index could not be
restored fails with exit 1 and can leave the working tree partly changed.

With no stashes at all there is nothing to default to, and Git says so:

```console
$ quinjet stash pop
error: Git command failed: No stash entries found.
```

That is exit 1. A successful pop renumbers everything after the entry that
went, so any reference held from before the pop now points at different work.

`--json` shape, an object with one key. The sentence differs between the two
forms, because with no reference there is nothing to name:

```json
{
  "message": "Popped latest stash"
}
```

```json
{
  "message": "Popped stash@{1}"
}
```

Neither sentence reports what changed in the working tree. Follow with
`quinjet status` for that.

Examples:

```bash
quinjet stash pop
quinjet stash pop 'stash@{0}'
quinjet stash pop 'stash@{2}' --json
quinjet stash pop -C ~/code/project
```

```console
$ quinjet stash pop
Popped latest stash
```

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash apply`](./apply.md) for the same thing with the entry kept
- [All `quinjet` commands](../README.md)
