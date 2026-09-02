# `quinjet pr reviews visit`

Records the pull request's current head as the commit you last looked at.

Usage:

```bash
quinjet pr reviews visit <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the metadata cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Why it is a separate verb

`--since-review` needs a mark to measure from, and there are two candidates: a
review you submitted on GitHub, and a look you took without submitting anything.
The second is the common case in the middle of a review, and only the machine
you are on can know about it.

Reads do not record it. `quinjet pr reviews progress` and
[`quinjet pr diff --since-review`](./diff.md) leave the mark alone, so running
either twice gives the same answer. Stamping the visit is an explicit act,
because it is what makes the next delta smaller.

```console
$ quinjet pr reviews visit 42
Recorded a visit to #42 at 3180ef896154
```

The recorded commit takes precedence over your last review, but only while it is
older than the current head: a visit recorded at the head that is still current
would make the delta empty, so it is ignored and the last review is used instead.
That way stamping a visit and immediately asking for the delta still tells you
what changed since you last reviewed, rather than nothing.

Use [`quinjet pr reviews viewed <number> --reset`](./review-viewed.md) to forget
the visit along with the rest of the record.

## Examples

```bash
quinjet pr reviews visit 42
```

Ending a review session with one command:

```bash
quinjet pr reviews submit 42 --approve --body "Looks good"
quinjet pr reviews visit 42
```

## Where to go next

- [`quinjet pr reviews progress`](./review-progress.md) for what the visit changes
- [`quinjet pr diff`](./diff.md) for `--since-review`
- [All `quinjet` commands](../README.md)
