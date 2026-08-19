# `quinjet worktree list`

Prints every Git worktree attached to this repository.

Usage:

```bash
quinjet worktree list [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| none | | | `list` takes no positional argument. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global; a subdirectory or a linked worktree resolves to that tree's root. |
| `--json` | flag | off | Prints one JSON array on stdout instead of the table. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

There is no filter, no `--all` and no `--watch`. The whole worktree list is
read every time.

One Git call does it:

```bash
git worktree list --porcelain -z
```

Records are separated by an extra NUL, fields by a single NUL. The fields
Quinjet keeps are `worktree <path>`, `HEAD <oid>`, `branch <ref>`, `detached`,
`bare`, `locked [reason]` and `prunable [reason]`. A record without a
`worktree` path is skipped. The branch field is stored without the
`refs/heads/` prefix.

`current` is true when that path is the same worktree the session opened,
compared after canonicalizing both sides when the filesystem allows it.
Opening the listing from a linked tree therefore marks that tree, not the
main checkout.

The text form is one row per worktree:

```text
* /path/to/main     main              abcdef01
  /path/to/topic    topic             fedcba98
  /path/to/hotfix   detached          01234567  prunable
```

The first column is `*` for this session and a space otherwise. The path is
unquoted. The branch column is the short name, `detached`, `bare`, or `-` when
Git reported none of those. A locked or prunable tree gets that word appended.

An empty extra listing is not a failure: a repository with only its main tree
prints one row and exits 0, and `--json` prints a one-element array.

`--json` shape, an array of objects, one per worktree, in Git's order (main
tree first):

```json
[
  {
    "path": "/tmp/repo",
    "head": "abcdef0123456789",
    "branch": "main",
    "current": true,
    "bare": false,
    "detached": false,
    "locked": null,
    "prunable": null
  },
  {
    "path": "/tmp/repo-topic",
    "head": "fedcba9876543210",
    "branch": "topic",
    "current": false,
    "bare": false,
    "detached": false,
    "locked": null,
    "prunable": null
  }
]
```

`locked` and `prunable` are strings when Git supplied a reason, an empty
string when the flag is present without a reason, and `null` when it is
absent. `branch` is `null` for a detached or bare tree.

The command line does not switch Quinjet onto another tree. In the terminal
interface, Enter on a row in Recent projects rebinds this session to that
path, which is the same as `quinjet tui <path>`.
