# `quinjet branch list`

Prints the branches in this repository, newest tip commit first.

Usage:

```bash
quinjet branch list [--all] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| none | | | `list` takes no positional argument. There is no name pattern and no glob. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Include remote-tracking branches, and switch to the listing that carries each branch's full ref. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Any directory inside the worktree works. |
| `--json` | flag | off | Prints one JSON array on stdout instead of the table. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`--all` does not add rows to the default listing. It runs a different read.

Without it, Quinjet runs

```text
git for-each-ref --sort=-committerdate \
  --format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:relative)%1f%(objectname:short)%1e \
  refs/heads
```

so the answer is local branches only, in Git's own committer-date order, and
each row knows the upstream it tracks. With `--all`, Quinjet runs

```text
git for-each-ref --sort=-committerdate \
  --format=%(refname:short)%1f%(refname)%1f%(HEAD)%1f%(committerdate:relative)%1f%(objectname:short)%1f%(symref)%1e \
  refs/heads refs/remotes
```

which trades the upstream for two other things: `%(refname)`, the full ref that
[`quinjet branch compare`](./compare.md) accepts, and `%(symref)`, which is how
symbolic aliases are recognized. Any ref with a non-empty `%(symref)` is
dropped, so `origin/HEAD` is never listed as a branch. Refs outside
`refs/heads/` and `refs/remotes/`, and records Git emits with fewer fields than
the format asked for, are skipped without comment.

The `--all` answer is then re-sorted by `(not current, remote)`, a stable sort,
so the current branch comes first, then the other local branches in date order,
then the remote-tracking branches in date order. The plain listing is not
re-sorted at all: the current branch sits wherever its commit date puts it, and
is identified only by the `*` in the first column.

`%(HEAD)` is what fills that column, and it means the branch checked out in
this worktree. On a detached HEAD nothing is marked. In a linked worktree, a
branch checked out by a different worktree looks like any other row here, even
though [`switch`](./switch.md) and [`delete`](./delete.md) will refuse it.

Nothing in this verb touches a network or a cache. Remote-tracking rows are as
old as your last fetch, and a branch someone deleted on the server is still
listed until a pruning fetch removes the ref locally. On an unborn branch, in a
repository with no commits, there are no refs at all: the table is empty, the
JSON is `[]`, and the exit code is 0.

`--json` shape, an array with one object per branch, in the printed order.
Without `--all`:

```json
[
  {
    "name": "chore/ci-hardening",
    "current": false,
    "upstream": "origin/chore/ci-hardening",
    "relativeDate": "5 minutes ago",
    "shortId": "f83fcd6"
  },
  {
    "name": "feat/cli-command-surface",
    "current": true,
    "upstream": null,
    "relativeDate": "6 minutes ago",
    "shortId": "e2d95c2"
  }
]
```

With `--all` the objects have a different shape:

```json
[
  {
    "name": "feat/cli-command-surface",
    "reference": "refs/heads/feat/cli-command-surface",
    "current": true,
    "remote": false,
    "relativeDate": "6 minutes ago",
    "shortId": "e2d95c2"
  },
  {
    "name": "origin/main",
    "reference": "refs/remotes/origin/main",
    "current": false,
    "remote": true,
    "relativeDate": "5 hours ago",
    "shortId": "6ce4acd"
  }
]
```

`upstream` is `null` rather than absent when a branch tracks nothing, and it
exists only in the plain listing. `reference` and `remote` exist only under
`--all`. `relativeDate` is Git's phrasing of the tip commit's committer date,
not the age of the branch. `shortId` is `%(objectname:short)`, so its length is
whatever Git considers unambiguous in this repository.

Examples:

```bash
quinjet branch list
quinjet branch list --all
quinjet branch list --json
quinjet branch list --all -C ~/code/project
quinjet branch list --all --json | jq -r '.[] | select(.remote) | .reference'
```

```console
$ quinjet branch list
  chore/ci-hardening           f83fcd6    5 minutes ago  -> origin/chore/ci-hardening
* feat/cli-command-surface     e2d95c2    6 minutes ago
  main                         6ce4acd    5 hours ago  -> origin/main
  feat/pr-conversation-live-checks df8b3a8    6 hours ago  -> origin/feat/pr-conversation-live-checks
```

The fourth row shows the column rule: the name column is a minimum of 28
characters, not a maximum, so a longer name pushes the rest of the row right
instead of being cut. Under `--all` that minimum is 40, and the upstream arrow
is replaced by a `local` or `remote` word:

```console
$ quinjet branch list --all
* feat/cli-command-surface                 local    e2d95c2    6 minutes ago
  chore/ci-hardening                       local    f83fcd6    5 minutes ago
  main                                     local    6ce4acd    5 hours ago
  origin/chore/ci-hardening                remote   f83fcd6    5 minutes ago
  origin/main                              remote   6ce4acd    5 hours ago
```

Note the second and third rows: the current branch was hoisted above a branch
with a newer commit, which only happens under `--all`.

## Where to go next

- [`quinjet branch`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
