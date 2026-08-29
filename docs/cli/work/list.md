# `quinjet work list`

Lists the recorded work sessions, newest first.

Usage:

```bash
quinjet work list [-C <DIR>] [--json]
```

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Sessions are stored per machine rather than per repository, so this lists every
session Quinjet knows about, whichever checkout you run it from. Sixteen are
kept; the one nobody has touched in longest is dropped first.

```console
$ quinjet work list
w42-2      open       failed-checks  acme/project#42                          Add feature
w42-1      published  feedback       acme/project#42                          Add feature

2 session(s)
```

`--json` shape, one object with the same session documents
[`quinjet work start`](./start.md) prints:

```json
{
  "schemaVersion": 1,
  "sessions": [
    { "id": "w42-2", "state": "open", "source": "failed-checks", "...": "..." }
  ]
}
```

A machine with no sessions prints `No work sessions recorded` and an empty
array. Exits 0 either way.

## Where to go next

- [`quinjet work inspect`](./inspect.md) for one session in full
- [`quinjet work abort`](./abort.md) to clear one out
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
