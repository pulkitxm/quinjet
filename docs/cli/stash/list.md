# `quinjet stash list`

Prints every stash in the repository, newest first.

Usage:

```bash
quinjet stash list [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| none | | | `list` takes no positional argument. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global; a subdirectory resolves to the worktree root. |
| `--json` | flag | off | Prints one JSON array on stdout instead of the table. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

There is no limit, no filter, no `--all` and no `--watch`. The whole stash
reflog is read every time.

One Git call does it:

```bash
git stash list --format=%gd%x1f%gs%x1f%cr%x1f%h%x1e
```

Records are separated by `0x1e`, fields by `0x1f`, so neither delimiter can
occur inside a message and no quoting is involved. The fields are the reflog
selector `%gd`, the reflog subject `%gs`, the relative committer date `%cr` and
the abbreviated hash `%h`, which become `reference`, `branch` plus `message`,
`relativeDate` and `shortId`.

Two records are thrown away rather than shown. A record with fewer than four
fields is skipped, and a record whose selector is not literally `stash@{N}`
with an all-digit `N` is skipped as well. That second filter is the group's
safety rule in one place: nothing that could not be validated later ever enters
the list, so a reference taken from `stash list` is always one the other verbs
will accept.

The branch and the message are one field split in two. Git writes
`WIP on <branch>: <commit> <subject>` when a stash has no message and
`On <branch>: <message>` when it has one. Quinjet strips whichever prefix
matches and splits the remainder on its first colon and space. So a message that itself
contains a colon, such as `fix: parser`, survives intact. A subject with
neither prefix, which is what `git stash store` writes, leaves the branch as the
empty string and the row reads `on : <subject>`. A stash taken with a detached
HEAD carries the branch Git recorded, which is `(no branch)`.

The order is Git's own reflog order, newest first, and Quinjet does not re-sort
it. `stash@{0}` is the newest entry and the indices renumber whenever anything
is pushed, popped or dropped, so a reference is a position at a moment rather
than a name. The `shortId` is stable across renumbering, but no verb accepts it
as a reference.

An empty stash list is not a failure: the text form prints nothing at all and
exits 0, and `--json` prints `[]`.

The text form is one padded row per stash, in the format
`{reference:<12} {shortId:<10} {relativeDate:<14} on {branch}: {message}`. A
long relative date simply pushes the rest of the row right rather than being
cut.

`--json` shape, an array of objects, one per stash, in the same order:

```json
[
  {
    "reference": "stash@{0}",
    "message": "index only",
    "branch": "main",
    "relativeDate": "5 seconds ago",
    "shortId": "3211d32"
  },
  {
    "reference": "stash@{1}",
    "message": "launch work",
    "branch": "main",
    "relativeDate": "5 seconds ago",
    "shortId": "6e8762c"
  }
]
```

`reference` is the only key any other verb accepts. `branch` is the branch
recorded in the reflog subject at stash time, not a ref that has to still
exist, and it is `""` rather than `null` when the subject had no recognized
prefix. `relativeDate` is Git's own phrasing and is not machine-readable;
there is no timestamp key. `shortId` is the stash commit, whose first parent is
the HEAD the stash was taken from.

Examples:

```bash
quinjet stash list
quinjet stash list --json
quinjet stash list -C ~/code/project
quinjet stash list --json | jq -r '.[] | "\(.reference) \(.message)"'
```

```console
$ quinjet stash list
stash@{0}    e09c928    0 seconds ago  on (no branch): 67b9bfd init
stash@{1}    41b0f7a    0 seconds ago  on main: 67b9bfd init
stash@{2}    8649889    80 seconds ago on main: quick experiment
```

The first row is a stash taken while HEAD was detached. The first two have no
message of their own, so what follows the colon is the `WIP on` subject Git
generated: the commit the stash was taken from and its subject.

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash show`](./show.md) to read one of these entries as a patch
- [All `quinjet` commands](../README.md)
