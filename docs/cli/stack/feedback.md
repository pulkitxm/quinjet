# `quinjet stack feedback`

Lists everything outstanding across a stack, bottom to top, in one queue.

Usage:

```bash
quinjet stack feedback <number> [--unresolved] [--mine] [--exit-code] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | Any pull request in the stack. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--unresolved` | flag | off | Only what the merge is actually waiting on. |
| `--mine` | flag | off | Only what is waiting on a reply from you. |
| `--exit-code` | flag | off | Exit 1 when anything blocking remains. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the caches for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Each member's rows come from the same reduction
[`quinjet pr feedback`](../pull-request/feedback.md) uses, so a row here says
exactly what it says there, including the Quinjet command that resolves it.

## Why the order matters

The queue is in stack order, bottom first, and that is not cosmetic. Answering a
thread on the bottom member is what lets everything above it move; answering one
on the top member changes nothing until the ones below it are done.

`next_position` names the lowest member carrying anything blocking, which is
where the stack unblocks from:

```text
next  position 1 #41 thread src/lib.rs:9
```

## Filters move the counts with the rows

`--unresolved` and `--mine` narrow every member's rows, and the per-member counts
and the stack totals are both recomputed from what is left, so the summary always
describes what is on screen. `next_position` moves with them: filtering away the
bottom member's only blocking row makes the next member up the one to start
from.

Line-level check findings are not included. A stack-wide queue that also carried
every CI annotation from every member would be unreadable, and
[`quinjet stack review`](./review.md) already names the failing check lowest in
merge order, which is the one that matters.

`--json` shape, one object:

```json
{
  "schemaVersion": 1,
  "number": 12,
  "size": 2,
  "selectedPosition": 2,
  "viewer": "octocat",
  "members": [
    {
      "position": 1,
      "number": 41,
      "title": "Build stack model",
      "selected": false,
      "items": [
        {
          "kind": "thread",
          "id": "THREAD_1",
          "owner": "you",
          "summary": "Please rename this file",
          "action": "reply with `quinjet pr reviews reply 41 THREAD_1 --body ...`"
        }
      ],
      "counts": { "blocking": 1, "advisory": 0, "awaitingYou": 1, "awaitingOthers": 0 }
    }
  ],
  "counts": { "blocking": 1, "advisory": 0, "awaitingYou": 1, "awaitingOthers": 0 },
  "nextPosition": 1,
  "truncated": false,
  "warnings": []
}
```

Item shape is the one documented in
[`quinjet pr feedback`](../pull-request/feedback.md). Sixteen members are read;
a deeper stack sets `truncated`.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The queue was printed. |
| 1 | With `--exit-code`, something blocking remains after the filters. |
| 3 | The pull request is not part of a stack. |

Examples:

```bash
quinjet stack feedback 42
quinjet stack feedback 42 --unresolved
quinjet stack feedback 42 --mine --exit-code
quinjet stack feedback 42 --json | jq -r '.members[] | "\(.position) \(.counts.blocking)"'
```

## Where to go next

- [`quinjet stack review`](./review.md) for merge order and the critical path
- [`quinjet pr feedback`](../pull-request/feedback.md) for one member in full
- [`quinjet work start --from feedback`](../work/start.md) to start answering
- [`quinjet stack`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
