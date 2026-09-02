# `quinjet pr feedback`

One queue of everything outstanding on a pull request, out of the review
threads, the review verdicts, and what CI reported about particular lines.

Usage:

```bash
quinjet pr feedback <number> [--unresolved] [--mine] [--no-checks] [--full] [--exit-code] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--unresolved` | flag | off | Only rows the merge is actually waiting on. |
| `--mine` | flag | off | Only rows waiting on a reply from you. |
| `--no-checks` | flag | off | Leave out the line-level findings a check reported, and do not read them. |
| `--full` | flag | off | Print each row's whole text and what resolves it. |
| `--exit-code` | flag | off | Exit 1 when anything the merge is waiting on remains. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the metadata, gate and annotation caches for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Why one queue

An author coming back to a pull request has to read three views to find out what
is outstanding: the conversation for review threads, the checks for what failed,
and the review verdicts for who is blocking. Each answers part of it, and none
says which of them is waiting on *them*.

```console
$ quinjet pr feedback 42
changes   you     @hubot                           1 reviewer requested changes
failure   -       feature.txt:1                    use a slice
thread    you     feature.txt:1                    Please rename this file
outdated  others  feature.txt:1                    Left over from an older push
advisory  -       README.md:2                      Spell Checker

2 blocking, 3 advisory · 1 on you, 1 on others
next  changes @hubot
```

## What a row is

| Kind | Blocking | Source |
| --- | --- | --- |
| `changes` | yes | A reviewer's `CHANGES_REQUESTED` verdict, one row per reviewer. |
| `failure` | yes | A check annotation at failure level. |
| `thread` | yes | An unresolved review thread on code that still exists. |
| `outdated` | no | An unresolved thread a later commit made outdated. |
| `advisory` | no | A check annotation at warning or notice level. |

Rows are ordered by that table, so the first row is always the most direct
obstacle and the order is stable across reads. Blocking means the merge is
waiting on it; advisory means it is worth reading. That separation is the same
one [`quinjet pr gate`](./gate.md) draws between blockers and notes, for the
same reason: a queue where everything looks equally urgent is a list, not a
queue.

A reviewer's changes-requested verdict is one row rather than one per comment.
The verdict is what stands between the pull request and merging, and the
comments behind it are already their own rows.

## Who each row waits on

The `owner` column is what makes one queue useful to both an author and a
reviewer, and it is computed against the authenticated viewer:

| Owner | Meaning |
| --- | --- |
| `you` | The newest word is somebody else's, so it is yours to answer. |
| `others` | The newest word is yours, so it is waiting on somebody else. |
| `nobody` | Nothing was said: a check finding rather than a conversation. |

A changes-requested row you wrote yourself is `others`, because you are waiting
on the author rather than on yourself. `--mine` keeps only `you`.

Every row also carries the thing that resolves it, spelled out so a caller does
not have to know the verb map:

```console
$ quinjet pr feedback 42 --mine --full
thread    you     feature.txt:1                    Please rename this file
      Please rename this file
      -> reply with `quinjet pr reviews reply <n> THREAD_1 --body ...`
```

## Cost

Four reads at most: the pull-request metadata, the merge gate, the review
threads, and the check annotations. `--no-checks` drops the last of them and
does not ask for it at all, which is worth knowing on a pull request whose CI
writes hundreds of annotations.

Filters narrow the rows and the counts together, so the summary line always
describes what was printed, and `--exit-code` reads the filtered counts:
`--mine --exit-code` exits 1 only when something is waiting on you.

## `--json`

```json
{
  "schemaVersion": 1,
  "number": 42,
  "headOid": "aaaa",
  "viewer": "octocat",
  "items": [
    {
      "kind": "thread",
      "id": "THREAD_1",
      "path": "feature.txt",
      "line": 1,
      "author": "hubot",
      "summary": "Please rename this file",
      "body": "Please rename this file",
      "url": "https://github.com/acme/project/pull/42",
      "owner": "you",
      "mine": false,
      "action": "reply with `quinjet pr reviews reply <n> THREAD_1 --body ...`"
    }
  ],
  "counts": { "blocking": 2, "advisory": 3, "awaitingYou": 1, "awaitingOthers": 1 },
  "truncated": false,
  "warnings": []
}
```

`id` is the review thread's node id for a thread, the check run's id for a check
finding, and the reviewer's login for a verdict, which is what each of them is
addressed by. `kind` in the JSON is the full name, so an outdated thread is
`outdated-thread` where the text face prints `outdated`.

## Examples

```bash
quinjet pr feedback 42
quinjet pr feedback 42 --unresolved
quinjet pr feedback 42 --mine --full
quinjet pr feedback 42 --json | jq -r '.items[] | select(.owner == "you") | .action'
```

Replying to everything waiting on you:

```bash
quinjet pr feedback "$PR" --mine --json \
  | jq -r '.items[] | select(.kind == "thread") | .id' \
  | while read -r thread; do
      quinjet pr reviews reply "$PR" "$thread" --body "Fixed in the latest push"
    done
```

## Where to go next

- [`quinjet pr gate`](./gate.md) for whether those rows block the merge
- [`quinjet pr suggestions`](./suggestions.md) for the ones with a patch attached
- [`quinjet pr reviews`](./reviews.md) for replying and resolving
- [`quinjet pr checks annotations`](./checks-annotations.md) for the findings alone
- [All `quinjet` commands](../README.md)
