# `quinjet stack gate`

Runs [the merge gate](../pull-request/gate.md) over every member of a stack and
reports the safe merge order.

Usage:

```bash
quinjet stack gate <number> [--repo <owner/name>] [--refresh] [--no-exit-code] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | Any pull-request number in the stack. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the caches for the stack and for every member's gate. |
| `--no-exit-code` | flag | off | Always exit 0, whatever the verdict is. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What it adds over one gate per member

A stack merges bottom-first, so a blocked layer blocks everything above it
whatever those layers say about themselves. Reading each member's gate answers
"is this one ready"; this verb answers "what can I merge, and where should I
work".

Two derived fields carry that:

- `mergeablePrefix` is the run of positions from the bottom that can merge right
  now. It stops at the first member that cannot, because merging past a blocked
  layer is not safe even when the layer above it is green on its own.
- `criticalPosition` is that first member that cannot. It is the lowest blocked
  layer, and therefore the one worth working on: fixing anything above it moves
  nothing.

A member that is already `merged` counts as clear for both, so a partly landed
stack keeps reporting a useful prefix.

The whole stack's `verdict` is the first non-clear member's verdict in merge
order, and it drives the exit code exactly as `pr gate` does: 0 for
`mergeable`, 1 for `blocked` or `closed`, 4 for `unknown`.

## Cost

One stack read, then one gate per member. Each member's gate is at most two
GitHub reads and is cached for 20 seconds under its own head commit, so a
repeated `stack gate` during one working session mostly answers from disk.

A member whose gate cannot be read does not fail the whole command. It is
dropped from `members` and its reason is added to `warnings`, so a permission
problem on one layer still leaves a useful answer about the rest. Note the
consequence: a dropped member cannot appear in `mergeablePrefix`, so the prefix
is conservative rather than optimistic when a read fails.

## Text output

```console
$ quinjet stack gate 42
blocked  stack #12  2 layers  destination main
>   2  #42      blocked    Add stack view
         CI: 1 required check failed
         approval: the latest push has not been approved
         threads: 1 unresolved thread
         branch: head is 4 commits behind main
    1  #41      mergeable  Build stack model

merge order  positions 1 can merge now, bottom first
critical     position 2 (#42) CI: 1 required check failed
```

Members print top-first, matching [`quinjet stack view`](./view.md), and `>`
marks the member the number you gave belongs to. The two lines under the ladder
read in merge order instead, because that is the order you would act in.

## `--json`

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
      "selected": false,
      "gate": { "verdict": "mergeable", "blockers": [] }
    },
    {
      "position": 2,
      "number": 42,
      "title": "Add stack view",
      "selected": true,
      "gate": { "verdict": "blocked", "blockers": [] }
    }
  ],
  "verdict": "blocked",
  "mergeablePrefix": [1],
  "criticalPosition": 2,
  "truncated": false,
  "warnings": []
}
```

`members` is sorted by `position`, bottom-first, whatever order GitHub returned.
Each member's `gate` is the complete `pr gate` document for that pull request,
with the same `schemaVersion` contract, so anything that reads one gate reads
these without changes.

`criticalPosition` is `null` when nothing is blocked.

## Examples

```bash
quinjet stack gate 42
quinjet stack gate 42 --json | jq '.mergeablePrefix'
quinjet stack gate 42 --json | jq -r '.members[] | select(.gate.verdict != "mergeable") | "#\(.number) \(.gate.blockers[0].summary)"'
```

Merging the ready prefix, bottom-first, is one loop:

```bash
for number in $(quinjet stack gate "$PR" --no-exit-code --json \
  | jq -r '[.members[] | select(.gate.verdict == "mergeable")] | .[].number'); do
  quinjet pr merge "$number" --squash --yes
done
```

## Where to go next

- [`quinjet pr gate`](../pull-request/gate.md) for what one member's verdict means
- [`quinjet stack view`](./view.md) for the ladder without the verdicts
- [`quinjet stack merge`](./merge.md) for merging a stack atomically
- [`quinjet stack`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
