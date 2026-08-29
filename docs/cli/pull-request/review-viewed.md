# `quinjet pr reviews viewed`

Marks changed files as read or unread in Quinjet's local review progress.

Usage:

```bash
quinjet pr reviews viewed <number> [<path>...] [--all] [--unviewed] [--reset] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `<PATH>...` | paths | none | Repository-relative paths to mark. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Mark every changed file. Conflicts with named paths. |
| `--unviewed` | flag | off | Mark as unread rather than read. |
| `--reset` | flag | off | Forget this pull request's local progress entirely. Conflicts with the other three. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the metadata cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What a mark records

The path and the head commit it was marked at. Recording the commit is the whole
point: a file marked at an older head keeps its mark only while nothing has
changed it since, and reopens as `changed-since-viewed` when something has. See
[`quinjet pr reviews progress`](./review-progress.md) for how that is decided.

Marking is local. Nothing is sent to GitHub, and no write permission is used.
Two checkouts of the same repository on the same machine share the record; two
machines do not.

Paths are recorded as given and are not validated against the changed-file list,
so marking a path the pull request does not touch is accepted and simply never
appears in a reading. `--all` reads the changed-file list first and marks
exactly what is in it.

```console
$ quinjet pr reviews viewed 42 src/lib.rs src/main.rs
Marked 2 file(s) as read in #42
```

```console
$ quinjet pr reviews viewed 42 src/lib.rs --unviewed
Marked 1 file(s) as unread in #42
```

The count is what changed. Marking a file read twice reports 1 both times,
because the second mark replaces the first with the current head rather than
adding a duplicate. Marking a file unread that was not read reports 0.

`--reset` forgets the whole record, including the recorded visit:

```console
$ quinjet pr reviews viewed 42 --reset
Cleared local review progress for #42
```

## Examples

```bash
quinjet pr reviews viewed 42 src/lib.rs
quinjet pr reviews viewed 42 --all
quinjet pr reviews viewed 42 --reset
```

Marking everything the delta did not touch, so only what moved is left:

```bash
quinjet pr reviews viewed 42 --all
quinjet pr reviews progress 42 --json \
  | jq -r '.files[] | select(.changedSince) | .path' \
  | xargs -r quinjet pr reviews viewed 42 --unviewed
```

## Where to go next

- [`quinjet pr reviews progress`](./review-progress.md) for what the marks mean
- [`quinjet pr reviews next`](./review-next.md) for the next step
- [`quinjet pr reviews visit`](./review-visit.md) for stamping a visit
- [All `quinjet` commands](../README.md)
