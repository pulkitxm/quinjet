# `quinjet pr reviews progress`

Reports what is left to review, measured against a commit rather than against
the whole pull request.

Usage:

```bash
quinjet pr reviews progress <number> [--since <oid> | --since-review] [--all] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--since <OID>` | commit id | unset | Measure the delta from this commit of the pull request. |
| `--since-review` | flag | on by default | Measure from your last visit, or your last review. Conflicts with `--since`. |
| `--all` | flag | off | List every changed file, not only what is left. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the metadata cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## The question it answers

Coming back to a pull request that moved is the hard part of reviewing. `pr
diff` prints the whole thing again, and reading it a second time to find the
part that changed is the work this verb removes.

```console
$ quinjet pr reviews progress 42
#42  1 of 3 files read  ·  2 unresolved
since    your last review bbbbbbbbbbbb
commits  1 new since then
changed  1 file(s) you had read have moved since
threads  1 awaiting your reply, 1 awaiting others, 1 outdated

changed   src/lib.rs *
unviewed  src/main.rs

next     changed src/lib.rs
```

A `*` marks a file that is part of the delta since the `since` commit. The
`next` line is the same answer [`quinjet pr reviews next`](./review-next.md)
gives on its own.

## Where `since` comes from

In order:

1. **Your last visit**, when [`quinjet pr reviews visit`](./review-visit.md)
   recorded one and the head has moved since. This is local state.
2. **Your last review**, read from GitHub's `latestReviews` filtered to the
   authenticated viewer. This is what makes the verb mean the same thing on a
   machine that has never seen the pull request before.
3. **The merge base**, when neither exists. Then the delta is the whole pull
   request, which is the honest answer for a first read.

`--since <oid>` overrides all of it. The commit is resolved against the pull
request's own commit list, accepting a full object name or a unique
abbreviation, so a typo exits 3 rather than silently widening the delta to
something unrelated:

```console
$ quinjet pr diff 42 --since deadbeef
error: `deadbeef` does not name a commit in pull request #42
```

The merge base is accepted too, even though it is not in the commit list.

## File state

Quinjet tracks which files you have read, locally, under the state root in
`review-progress.json`. Nothing is written to GitHub: file-viewed state there
belongs to the web session, and mirroring it would need write access this never
asks for.

Each mark records the head commit it was made at, which is what lets a later
read give a third answer rather than guessing:

| State | JSON | Meaning |
| --- | --- | --- |
| `viewed` | `viewed` | Read at the current head, or read earlier and provably unchanged since. |
| `unviewed` | `unviewed` | Never read. |
| `changed` | `changed-since-viewed` | Read earlier, and a later commit changed it. |
| `unknown` | `viewed-at-unknown-commit` | Read earlier, and Quinjet could not compare that commit with the head. |

Everything except `viewed` counts as remaining. That is deliberate: a file whose
history Quinjet cannot check is work, not a pass. The comparison uses local Git
when the checkout already holds both commits, which costs nothing, and GitHub's
comparison endpoint otherwise; both sides are immutable, so the answer is cached
forever. Files read at more than eight distinct commits stop being compared, and
a warning says so.

The record is capped at 64 pull requests and 4,096 viewed files, oldest dropped
first, because this is a working note rather than an archive.

## Threads

Unresolved threads are split by who owes the next word, which is the part that
tells an author and a reviewer different things:

- `awaitingYourReply` counts unresolved threads whose newest comment is not
  yours.
- `awaitingOthers` counts unresolved threads whose newest comment is yours.
- `outdatedUnresolved` counts unresolved threads a later commit made outdated,
  which is where a comment about code that no longer exists hides.

## `--json`

```json
{
  "schemaVersion": 1,
  "repository": "acme/project",
  "number": 42,
  "headOid": "aaaa",
  "since": { "oid": "bbbb", "source": "review", "detail": "COMMENTED" },
  "visitedAt": "",
  "files": [
    {
      "path": "src/lib.rs",
      "status": "modified",
      "state": "changed-since-viewed",
      "viewedAtOid": "bbbb",
      "changedSince": true
    }
  ],
  "viewed": 1,
  "remaining": 2,
  "changedSinceViewed": 1,
  "changedSince": 1,
  "newCommits": [],
  "threads": {
    "total": 3,
    "unresolved": 2,
    "outdatedUnresolved": 1,
    "awaitingYourReply": 1,
    "awaitingOthers": 1
  },
  "next": { "kind": "file", "path": "src/lib.rs", "state": "changed-since-viewed" },
  "threadStep": { "kind": "thread", "id": "THREAD_1", "path": "src/lib.rs", "line": 12, "outdated": false, "author": "hubot", "excerpt": "Please rename this" },
  "truncated": false,
  "warnings": []
}
```

`since.source` is one of `visit`, `review`, `explicit`, `merge-base`.
`threadStep` is the next unresolved thread whether or not files remain, so a
caller that wants threads does not have to read the review again. `files` lists
every changed file whatever `--all` says; `--all` only affects the text face.

## Examples

```bash
quinjet pr reviews progress 42
quinjet pr reviews progress 42 --all
quinjet pr reviews progress 42 --since 4f2a1c9
quinjet pr reviews progress 42 --json | jq -r '.files[] | select(.state != "viewed") | .path'
```

A review loop, in four commands:

```bash
quinjet pr diff 42 --since-review           # what moved under you
quinjet pr reviews viewed 42 src/lib.rs     # mark it read
quinjet pr reviews next 42                  # what is next
quinjet pr reviews visit 42                 # stamp this head as seen
```

## Where to go next

- [`quinjet pr reviews next`](./review-next.md) for the next step alone
- [`quinjet pr reviews viewed`](./review-viewed.md) for marking files read
- [`quinjet pr reviews visit`](./review-visit.md) for stamping a visit
- [`quinjet pr diff`](./diff.md) for `--since` and `--since-review` patches
- [`quinjet pr reviews`](./reviews.md), the rest of the review family
- [All `quinjet` commands](../README.md)
