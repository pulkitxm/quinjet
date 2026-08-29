# `quinjet pr checks rerun`

Reruns a pull request's failed jobs, its failed runs, or the one job a named
check reported.

Usage:

```bash
quinjet pr checks rerun <number> <--failed | --all | --check <name>> [--yes] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--failed` | flag | off | Rerun only the failed jobs of every failed run. One of the three is required. |
| `--all` | flag | off | Rerun every failed run from the start. |
| `--check <NAME>` | string | unset | Rerun the one GitHub Actions job a named check reported. |
| `--yes` | flag | off | Confirm; without it the command reports what it would rerun and changes nothing. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second workflow-run cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Preview first

Without `--yes` the verb names the exact runs it would act on and changes
nothing, which is the same contract every destructive verb in Quinjet follows:

```console
$ quinjet pr checks rerun 42 --failed
Would rerun the failed jobs of `CI` (run 7701), `Lint` (run 7702). Pass --yes to do it.
```

```console
$ quinjet pr checks rerun 42 --failed --yes
Reran the failed jobs of `CI` (run 7701), `Lint` (run 7702)
```

The preview and the confirmation read from the same list, so what the second
prints is always what the first described. That matters here more than for most
verbs: a rerun spends CI minutes, and a preview that summarized rather than
enumerated could hide a run you did not mean to touch.

Nothing to act on is not an error. A pull request whose runs all passed reports
why and exits 0:

```console
$ quinjet pr checks rerun 42 --failed --yes
Nothing to act on: no workflow run on this pull request has failed
```

## The three scopes

**`--failed`** posts to `actions/runs/<id>/rerun-failed-jobs` for each run whose
conclusion is a failure or a cancellation. This is the usual one: it reruns the
jobs that did not pass and reuses the results of the ones that did.

**`--all`** posts to `actions/runs/<id>/rerun` instead, which reruns the whole
run from the start. Reach for it when a passing job's result is not trustworthy,
such as after a flaky cache or a changed secret.

**`--check <name>`** reruns one job. The name is resolved the way
[`quinjet pr logs`](./logs.md) resolves it: exactly first, then as a unique
case-insensitive substring, exiting 3 with the list of names when it matches
nothing or more than one thing. The job id comes from the check's link, so a
check that is not a GitHub Actions job cannot be rerun on its own and exits 4:

```console
$ quinjet pr checks rerun 42 --check codecov
error: the `codecov/patch` check is not a GitHub Actions job, so it cannot be rerun on its own
```

A cancelled run counts as failed for `--failed` and `--all`, because cancelling
is usually followed by wanting it back.

## Where the runs come from

One request lists the workflow runs for the head commit, cached for 30 seconds
under `workflow-runs-v1\n<repository url>\n<head oid>`. Then one POST per run.
[`quinjet pr checks runs`](./checks-runs.md) prints that same listing, which is
the way to see what a rerun would act on without asking for a preview.

`--check` does not read the run list at all: it reads the check listing, which
is what carries the job id.

## Examples

```bash
quinjet pr checks rerun 42 --failed
quinjet pr checks rerun 42 --failed --yes
quinjet pr checks rerun 42 --all --yes
quinjet pr checks rerun 42 --check "windows-latest" --yes
```

Rerunning and then waiting for the result:

```bash
quinjet pr checks rerun "$PR" --failed --yes
timeout 30m quinjet pr checks "$PR" --watch --interval 30
```

## Where to go next

- [`quinjet pr checks runs`](./checks-runs.md) for what a rerun would act on
- [`quinjet pr checks cancel`](./checks-cancel.md) for stopping runs instead
- [`quinjet pr checks`](./checks.md) for the check listing
- [`quinjet pr logs`](./logs.md) for why a job failed
- [All `quinjet` commands](../README.md)
