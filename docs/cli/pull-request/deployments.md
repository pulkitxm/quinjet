# `quinjet pr deployments`

Lists what a pull request's head commit deployed and what is waiting for a
human, and lets those waiting runs through.

Usage:

```bash
quinjet pr deployments <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
quinjet pr deployments approve <number> <environment> [--comment <text>] [--yes] [-C <DIR>] [--json]
quinjet pr deployments reject <number> <environment> [--comment <text>] [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `<ENVIRONMENT>` | string | required for `approve` and `reject` | The environment holding the runs. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--comment <TEXT>` | string | empty | Note recorded with the decision. |
| `--yes` | flag | off | Confirm; without it the command reports what it would do and changes nothing. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second workflow-run cache for the listing. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Two different things

A deployment that happened and a deployment waiting to happen come from
different endpoints, because the second does not exist yet as a deployment: it
is a workflow run held by an environment's protection rules. The listing shows
both, apart:

```console
$ quinjet pr deployments 42
Waiting for approval
  staging                  run 7702  Deploy  reviewers octocat
  production               run 7702  Deploy  (you cannot review this)

Deployed
  preview                  Fri Aug 21 2:30 AM  transient
```

`Waiting for approval` comes from each unsettled run's `pending_deployments`;
`Deployed` comes from the repository's deployments for the head commit. Either
side failing leaves a `note` line rather than failing the command, because the
other side is still worth reading.

Pending approvals are **never cached**. They are the input to a mutation, and
acting on a stale one would approve a run that is no longer the one waiting.

## Approving and rejecting

```console
$ quinjet pr deployments approve 42 staging
Would approve `staging` for run 7702. Pass --yes to do it.
```

```console
$ quinjet pr deployments approve 42 staging --comment "shipping it" --yes
Approved `staging` for run 7702
```

The environment is matched case-insensitively and ignoring surrounding space,
and **every** run waiting on it is included: one environment can hold more than
one run at a time, and approving it means all of them. The preview names each,
so the count is never a surprise.

Requests are grouped by run, because GitHub's endpoint takes a run and a list of
environment ids together, so one waiting run with two named environments is one
request rather than two.

Two refusals are worth knowing:

- An environment GitHub says you cannot review exits 4 rather than sending a
  request that would be rejected:

  ```console
  $ quinjet pr deployments approve 42 production --yes
  error: GitHub does not let you review `production` on run 7702
  ```

- An environment nothing is waiting on exits 0, says so, and lists the ones that
  are, because a typo in an environment name is the common cause:

  ```console
  $ quinjet pr deployments approve 42 stagng --yes
  Nothing to act on: no run is waiting on `stagng`
  hint: the waiting environments are: production, staging
  ```

## `--json`

```json
{
  "headOid": "aaaa",
  "pending": [
    {
      "runId": 7702,
      "workflow": "Deploy",
      "environment": "staging",
      "environmentId": 55,
      "waitTimer": 0,
      "viewerCanApprove": true,
      "reviewers": ["octocat"]
    }
  ],
  "deployments": [
    {
      "id": 4100,
      "environment": "preview",
      "description": "Preview build",
      "createdAt": "2026-08-21T02:30:00Z",
      "url": "https://api.github.com/deployments/4100",
      "transient": true
    }
  ],
  "warnings": []
}
```

## Examples

```bash
quinjet pr deployments 42
quinjet pr deployments approve 42 staging --yes
quinjet pr deployments reject 42 production --comment "not this build" --yes
quinjet pr deployments 42 --json | jq -r '.pending[] | select(.viewerCanApprove) | .environment'
```

Approving everything you are allowed to:

```bash
quinjet pr deployments "$PR" --json \
  | jq -r '[.pending[] | select(.viewerCanApprove) | .environment] | unique | .[]' \
  | while read -r environment; do
      quinjet pr deployments approve "$PR" "$environment" --yes
    done
```

## Where to go next

- [`quinjet pr gate`](./gate.md), which reports a waiting deployment as a blocker
- [`quinjet pr checks runs`](./checks-runs.md) for the runs being held
- [All `quinjet` commands](../README.md)
