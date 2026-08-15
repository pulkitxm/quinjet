# `quinjet log`

Lists commits from any branch, tag or commit, newest first, without changing
HEAD or touching the worktree.

Usage:

```bash
quinjet log [REVISION] [--skip <SKIP>] [-n <LIMIT>] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[REVISION]` | branch, tag, commit, or any expression Git can resolve to a commit | `HEAD` | Where to start reading. Resolved by Quinjet before Git sees it. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--skip <SKIP>` | unsigned integer | `0` | Commits to drop from the front of the range, for paging. Becomes `--skip=<n>`. |
| `-n, --limit <LIMIT>` | unsigned integer | `30` | Commits to print. Becomes `--max-count=<n>`. `0` means 300, the internal page size, not unlimited. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of text. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The revision is resolved first, by `Repository::resolve_revision`, and this is
the interesting part of the verb. A revision that is empty or starts with `-` is
refused before any Git process starts, with
`` refusing to resolve `-n` as a revision ``, so no argument can ever be read by
Git as an option. `HEAD` is answered as itself with no Git call. Anything else
is tried first as
`git rev-parse --symbolic-full-name --verify --quiet <revision>`, and the answer
is kept only if it is a ref under `refs/heads/`, `refs/remotes/` or
`refs/tags/`, and otherwise as
`git rev-parse --verify --quiet <revision>^{commit}`, which yields a full object
id. If both fail the verb exits 3 with
`` `<revision>` does not name a commit in this repository `` and the hint
`` run `quinjet log` or `quinjet branch list --all` for what this repository
holds ``. A revision that starts with `-` or is empty exits 3 the same way.

So `HEAD`, `main`, `origin/main`, `v0.0.6`, a short id such as `e2d95c2`, and an
expression such as `HEAD~3` or `main^2` all work. A name that resolves to a ref
keeps its full ref name rather than becoming an id: `main` becomes
`refs/heads/main`, `origin/main` becomes `refs/remotes/origin/main` and the tag
`v0.0.6` becomes `refs/tags/v0.0.6`. Only the two forms that no ref answers, a
short id and a `~`/`^` expression, come back as a full 40 character object id.
A branch and a tag of the same name are resolved by Git's own precedence, since
Quinjet asks `rev-parse` rather than guessing.

The history read then applies a whitelist of its own and refuses anything that
is not `HEAD`, a `refs/heads/`, `refs/remotes/` or `refs/tags/` ref, or a full
object id of 40 or 64 hex characters, with
`refusing to load history for an invalid branch reference`. Resolution can only
produce one of those, so you cannot reach that message from the command line. It
is there so that no caller inside Quinjet can smuggle an option into:

```text
git log --topo-order --decorate=short --no-color \
    --skip=<skip> --max-count=<limit> --format=<record> <revision> --
```

`--topo-order` means the order is topological, not chronological: a branch's
commits stay contiguous, so after a merge the printed order can differ from a
date-sorted `git log` and the relative dates can look out of sequence. The
trailing `--` guarantees the revision is never reinterpreted as a path.

The record format is delimiter-safe rather than line-oriented: fields are
separated by unit separator `\x1f` and commits by record separator `\x1e`, so a
subject containing tabs, pipes or newlines parses correctly. The fields are
`%H %h %P %aN %aE %aI %cN %cE %cI %ar %s %D`. `%aN` and `%aE` are the
mailmap-respecting forms, so `.mailmap` changes what you see. `%D` is the
decoration list, which `--decorate=short` renders as `HEAD -> main`,
`origin/main` and `tag: v0.0.6`.

The text form is one line per commit: short id, relative date, author padded to
16 columns and truncated with an ellipsis past that, subject, and the
decorations in brackets when there are any. The relative date is not padded, so
the author column does not always line up. The committer is carried in `--json`
but never printed.

An empty range is not an error. `quinjet log --skip 100000` prints nothing and
exits 0, and `[]` under `--json`. An unborn branch is a different matter: there
is no `HEAD`, so the verb exits 1 with
`Git command failed: fatal: bad revision 'HEAD'`.

Reading another branch never changes yours. There is no checkout, no ref
written, and no index or worktree touched by this verb.

`--json` shape, an array of commit objects, newest first:

```json
[
  {
    "id": "e2d95c224418b5568e27d705e9539daf191519b8",
    "shortId": "e2d95c2",
    "parentIds": [
      "629a80535b7630b5a93dfb09737f216c7a4f0217"
    ],
    "author": "Pulkit",
    "authorEmail": "kpulkit15234@gmail.com",
    "authoredAt": "2026-08-16T00:12:36+05:30",
    "committer": "Pulkit",
    "committerEmail": "kpulkit15234@gmail.com",
    "committedAt": "2026-08-16T00:12:36+05:30",
    "relativeDate": "6 minutes ago",
    "subject": "test: pin the command line's contract",
    "decorations": [
      "HEAD -> feat/cli-command-surface"
    ]
  }
]
```

`parentIds` is empty for a root commit and has two or more entries for a merge,
which is the cheapest way to spot either. `authoredAt` and `committedAt` are
strict ISO 8601 with the original offset, not normalized to UTC, so they are
comparable only after parsing. `relativeDate` is Git's own wording and is
computed at read time. `decorations` is already split and trimmed, and the
`tag:` prefix is kept as Git writes it. There is no body: the format carries
`%s` only, so a commit's message beyond its subject line is not available from
this verb.

Examples:

```bash
quinjet log
quinjet log -n 10
quinjet log origin/main -n 5
quinjet log v0.0.6 -n 1
quinjet log --skip 30 -n 30 --json
```

```console
$ quinjet log -n 6
e2d95c2  6 minutes ago  Pulkit            test: pin the command line's contract  (HEAD -> feat/cli-command-surface)
629a805  8 minutes ago  Pulkit            feat: give every operation a subcommand
fe6a382  15 minutes ago  Pulkit            feat: name every operation once, in one command layer
32a089f  20 minutes ago  Pulkit            feat: make every value the app renders serializable
6ce4acd  5 hours ago  github-actions[…  chore: release v0.0.6  (tag: v0.0.6, origin/main, main)
58eae9b  5 hours ago  Pulkit            Merge pull request #8 from pulkitxm/feat/pr-conversation-live-checks
```

```console
$ quinjet log nope
error: `nope` does not name a commit in this repository
hint: run `quinjet log` or `quinjet branch list --all` for what this repository holds
```

## Where to go next

- [`quinjet show`](./show.md) for one of these commits and its patch
- [`quinjet branch`](../branch/README.md) for the branches a revision can name
- [`quinjet status`, `diff`, `log`, `show`](./README.md), the rest of this group
  and revision resolution, ordering and the exit codes
- [All `quinjet` commands](../README.md)
