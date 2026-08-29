# `quinjet stack review`

Reads the whole stack at once and answers the three questions a single pull
request cannot: what can merge right now, which one member everything else is
waiting on, and where two members touch the same file.

Usage:

```bash
quinjet stack review <number> [--incremental] [--exit-code] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | Any pull request in the stack. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--incremental` | flag | off | Measures each member against its own parent rather than using GitHub's totals. |
| `--exit-code` | flag | off | Exit 1 when anything blocks the stack. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the caches for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Each member is read through [`quinjet pr gate`](../pull-request/gate.md), so a
member's verdict and blockers here are exactly the ones that verb reports. What
this adds is everything that only exists across members.

## Safe merge order

The merge order is the clear members from the bottom, stopping at the first that
is not clear. That stop is the whole point: a member above a blocked one cannot
merge either, however green it looks, because its base has not landed yet.

```text
merge     1, 2, then stop
```

A stack whose bottom member is blocked reports `nothing can merge yet`, even if
every member above it is approved and green. A `merged` member does not stop the
order, because it has already landed.

## Own versus downstream

Every member is marked with where its block comes from:

| Word | Meaning |
| --- | --- |
| `own` | The member's own gate says it cannot merge. There is work to do here. |
| `downstream` | The member is clear and is only waiting for a layer below it. There is nothing to do here. |
| `clear` | The member is clear and nothing below it is blocked. |

That distinction is what stops a reviewer opening five pull requests to find
four of them are fine. `downstreamBlocked` in the JSON lists exactly the
positions where nobody should be spending time.

## The critical path

`criticalPosition` is the lowest blocked member: the one member whose blockers
are holding everything above it. `criticalPath` is that position and every
position above it, which is what that one member is holding up.

```text
critical  position 2 (#42) CI: 1 required check failed
          holding up 3 members
```

`earliestFailingCheck` is the failing check lowest in merge order, for the same
reason: everything above it waits for that member either way, so it is the check
worth looking at first.

## Approvals invalidated by a later push

An approval given on a commit that is no longer the head is reported by
reviewer, not just counted:

```text
stale     1 approval invalidated by a later push (octocat)
```

Naming the reviewer matters, because this is a request to go back to a person
rather than a state that clears itself. Only approvals count here: a
changes-requested review that is also stale is not an approval anybody lost.

## Duplicated changes across members

Two members that change the same file are where a rebase conflict comes from,
and no single pull request's diff can show it:

```text
touched by more than one member
  src/lib.rs                                       positions 1, 2
```

This needs real path lists, so it is only populated under `--incremental`, which
compares each member from its own base commit to its own head commit. That is
one comparison per member, so it is not the default; without it the review uses
GitHub's per-member totals, which are enough for the merge order and the
critical path but say nothing about which files moved.

Path lists are capped at 200 per member, with `pathsTruncated` set when a
member is wider than that.

`--json` shape, one object:

```json
{
  "schemaVersion": 1,
  "number": 12,
  "baseRef": "main",
  "size": 2,
  "selectedPosition": 2,
  "members": [
    {
      "position": 1,
      "number": 41,
      "title": "Build stack model",
      "url": "https://github.com/acme/project/pull/41",
      "selected": false,
      "verdict": "mergeable",
      "blockSource": "none",
      "blockers": [],
      "headOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
      "additions": 1,
      "deletions": 0,
      "changedFiles": 1,
      "staleApprovals": [],
      "unresolvedThreads": 0,
      "failingChecks": [],
      "paths": ["src/lib.rs"],
      "pathsTruncated": false
    }
  ],
  "mergeOrder": [1],
  "criticalPath": [2],
  "criticalPosition": 2,
  "downstreamBlocked": [],
  "earliestFailingCheck": {
    "position": 2,
    "number": 42,
    "check": "windows / test",
    "required": true
  },
  "duplicatedPaths": [{ "path": "src/lib.rs", "positions": [1, 2] }],
  "staleApprovals": 1,
  "unresolvedThreads": 1,
  "truncated": false,
  "warnings": []
}
```

`verdict` is the gate's, one of `mergeable`, `blocked`, `merged`, `closed`,
`unknown`. `blockSource` is `none`, `own` or `downstream`. Members are always in
stack order, whatever order GitHub returned them in.

Sixteen members are reviewed; a deeper stack sets `truncated` and says so in
`warnings`. A member whose gate could not be read is a warning rather than a
missing verdict, so a partial answer never reads as a clear one.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The review was printed. |
| 1 | With `--exit-code`, something blocks the stack. |
| 3 | The pull request is not part of a stack. |

Without `--exit-code` this verb always exits 0, unlike
[`quinjet stack gate`](./gate.md), which reports the verdict by default. The two
answer different questions: the gate is a decision, this is a reading.

Examples:

```bash
quinjet stack review 42
quinjet stack review 42 --incremental
quinjet stack review 42 --json | jq -r '.mergeOrder | join(", ")'
quinjet stack review 42 --json | jq -r '.members[] | select(.blockSource == "downstream") | .number'
quinjet stack review 42 --incremental --json | jq -r '.duplicatedPaths[] | .path'
```

## Where to go next

- [`quinjet stack gate`](./gate.md) for the merge decision itself
- [`quinjet stack feedback`](./feedback.md) for the outstanding conversation
- [`quinjet pr gate`](../pull-request/gate.md), which each member's verdict comes from
- [`quinjet stack`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
