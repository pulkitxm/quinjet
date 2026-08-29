# `quinjet pr checks runs`

Lists the GitHub Actions workflow runs behind a pull request's checks.

Usage:

```bash
quinjet pr checks runs <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second run cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Runs, not checks

[`quinjet pr checks`](./checks.md) lists check runs, which is what a reviewer
reads. This lists the workflow runs those checks belong to, which is what
rerunning, cancelling, artifacts and deployment approvals are all addressed by.
One run holds many checks, so this listing is shorter and its ids are the ones
the other verbs act on.

```console
$ quinjet pr checks runs 42
failed     pull_request  CI                                       run 7701
running    pull_request  Deploy                                   run 7702
passed     pull_request  Docs                                     run 7703
```

Runs are listed newest first by id, which is the order GitHub creates them in.
`state` reads the run's status before its conclusion, so a run reported as
`in_progress` with a conclusion from a previous attempt is `running` rather than
whatever that attempt concluded:

| State | Meaning |
| --- | --- |
| `queued` | Requested or waiting, including waiting on a deployment approval. |
| `running` | In progress. |
| `passed` | Completed successfully. |
| `failed` | Failed, timed out, failed at startup, or needs an action. |
| `cancelled` | Cancelled. |
| `skipped` | Skipped or neutral. |
| `unknown` | A conclusion Quinjet does not recognize. |

`failed` and `cancelled` are what [`quinjet pr checks rerun`](./checks-rerun.md)
acts on; `queued` and `running` are what
[`quinjet pr checks cancel`](./checks-cancel.md) acts on.

One request lists them, cached for 30 seconds under
`workflow-runs-v1\n<repository url>\n<head oid>`, and at most 100 runs are
listed before `truncated` is set.

## `--json`

```json
{
  "headOid": "aaaa",
  "runs": [
    {
      "id": 7701,
      "name": "CI",
      "state": "failed",
      "status": "completed",
      "conclusion": "failure",
      "url": "https://github.com/acme/project/actions/runs/7701",
      "attempt": 1,
      "event": "pull_request"
    }
  ],
  "truncated": false,
  "fromCache": false
}
```

`status` and `conclusion` are GitHub's own words, kept verbatim beside the
`state` Quinjet derives from them.

## Examples

```bash
quinjet pr checks runs 42
quinjet pr checks runs 42 --json | jq -r '.runs[] | select(.state == "failed") | .id'
```

## Where to go next

- [`quinjet pr checks rerun`](./checks-rerun.md) and [`cancel`](./checks-cancel.md)
- [`quinjet pr artifacts`](./artifacts.md) for what those runs uploaded
- [`quinjet pr deployments`](./deployments.md) for what they are waiting on
- [All `quinjet` commands](../README.md)
