# `quinjet pr checks cancel`

Cancels every workflow run on a pull request's head commit that has not settled.

Usage:

```bash
quinjet pr checks cancel <number> [--yes] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--yes` | flag | off | Confirm; without it the command reports what it would cancel and changes nothing. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Has no effect: the run list is always refreshed for this verb. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What it acts on

Only runs that are queued or in progress. A run that has already finished cannot
be cancelled, and one that GitHub has not scheduled yet does not appear in the
listing, so the set is exactly what a cancel can change:

```console
$ quinjet pr checks cancel 42
Would cancel `Deploy` (run 7702), `CI` (run 7703). Pass --yes to do it.
```

The run listing is always read fresh for this verb rather than from the 30
second cache, because a cancel acting on a stale list would either miss a run
that just started or try to cancel one that just finished. That is the one place
in this group where the cache is deliberately skipped.

Nothing in flight is not an error:

```console
$ quinjet pr checks cancel 42 --yes
Nothing to act on: no workflow run on this pull request is still going
```

## Examples

```bash
quinjet pr checks cancel 42
quinjet pr checks cancel 42 --yes
```

Cancelling before pushing a replacement, so the old runs do not race the new:

```bash
quinjet pr checks cancel "$PR" --yes
git push --force-with-lease
```

## Where to go next

- [`quinjet pr checks runs`](./checks-runs.md) for what is still going
- [`quinjet pr checks rerun`](./checks-rerun.md) for starting them again
- [All `quinjet` commands](../README.md)
