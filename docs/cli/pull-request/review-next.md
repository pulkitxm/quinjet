# `quinjet pr reviews next`

Prints the one thing to look at next in a pull request review.

Usage:

```bash
quinjet pr reviews next <number> [--files | --threads] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--files` | flag | off | Only consider changed files. Conflicts with `--threads`. |
| `--threads` | flag | off | Only consider unresolved threads. Conflicts with `--files`. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the metadata cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## The order

Files come before threads, and within files:

1. A file you had read that a later commit changed.
2. A file you had read at a commit Quinjet could not compare.
3. A file you have never read.

Re-reading what moved under you comes before starting something new, because a
file you already approved and that then changed is the one most likely to be
missed. Within each group the order follows the changed-file index, which is
stable across reads, so pressing the same key twice does not jump around.

When no file is left, the next unresolved thread is the answer, preferring one
whose newest comment is not yours: a thread you already replied to is waiting on
somebody else.

```console
$ quinjet pr reviews next 42
file    src/lib.rs
state   changed
```

```console
$ quinjet pr reviews next 42 --threads
thread  src/lib.rs:12
id      THREAD_1
from    @hubot
state   outdated by a later commit
says    Please rename this
```

`--files` and `--threads` narrow the answer without changing the order within
what is left. With nothing left in the requested category:

```console
$ quinjet pr reviews next 42 --files
Nothing left to review
```

That is exit 0, not an error: having nothing to do is a successful answer.

## `--json`

A file step:

```json
{ "kind": "file", "path": "src/lib.rs", "state": "changed-since-viewed" }
```

A thread step:

```json
{
  "kind": "thread",
  "id": "THREAD_1",
  "path": "src/lib.rs",
  "line": 12,
  "outdated": true,
  "author": "hubot",
  "excerpt": "Please rename this"
}
```

Nothing left:

```json
{ "next": null }
```

`kind` is the discriminator; read it before anything else. `excerpt` is the
first non-empty line of the newest comment, cut to 72 characters, so a queue row
stays a row. `line` is `null` for a file-level thread.

## Examples

```bash
quinjet pr reviews next 42
quinjet pr reviews next 42 --threads --json | jq -r .id
```

Opening the next file in an editor:

```bash
path=$(quinjet pr reviews next 42 --files --json | jq -r '.path // empty')
[ -n "$path" ] && "$EDITOR" "$path"
```

Replying to the next thread, then resolving it:

```bash
thread=$(quinjet pr reviews next 42 --threads --json | jq -r '.id // empty')
if [ -n "$thread" ]; then
  quinjet pr reviews reply 42 "$thread" --body "Fixed in the latest push"
  quinjet pr reviews resolve 42 "$thread"
fi
```

## Where to go next

- [`quinjet pr reviews progress`](./review-progress.md) for the whole reading
- [`quinjet pr reviews viewed`](./review-viewed.md) for marking files read
- [`quinjet pr reviews`](./reviews.md), the rest of the review family
- [All `quinjet` commands](../README.md)
