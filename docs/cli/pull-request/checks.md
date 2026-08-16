# `quinjet pr checks`

Lists a pull request's check runs, and optionally blocks until they settle.

Usage:

```bash
quinjet pr checks <number> [--repo <owner/name>] [--refresh] [--watch] [--interval <seconds>] [--exit-code] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second check cache for this read. Ignored under `--watch`, which always refreshes. |
| `--watch` | flag | off | Keeps reading until every check has settled, then exits with the verdict. |
| `--interval <SECONDS>` | integer of at least 2 | `5` | Seconds between reads while watching. Requires `--watch`; lower values are usage errors. |
| `--exit-code` | flag | off | Exit 1 when any check has failed or is still pending. Conflicts with `--watch`, which always reports the verdict. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout, or one compact object per read under `--watch`. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Underneath, this is one command:

```text
gh pr checks <number> --repo <canonical-url> \
  --json bucket,completedAt,description,link,name,startedAt,state,workflow \
  --jq '.[] | [.name, .workflow, .state, .bucket, (.description // ""), (.link // ""), (.startedAt // ""), (.completedAt // "")] | @tsv'
```

`gh pr checks` does not report success by exiting 0. It exits 1 when any run has
failed and 8 when any run is still pending, both of which are entirely normal
states for a pull request under CI. Quinjet therefore judges the response by its
content: exit 0, or exit 1 or 8 with something on stdout, are all accepted, and
anything else is an error unless its stderr mentions "no checks", which is
translated into an empty list. That is why this one read does not go through the
usual cached `gh` helper: it has to inspect the body before deciding whether to
cache it.

Status comes from `bucket`, not from `state`. GitHub's bucket values map to
Quinjet's as `pass` to `passed`, `fail` to `failed`, `pending` to `pending`,
`skipping` to `skipped`, `cancel` to `cancelled`, and anything else to
`unknown`. `state` is kept verbatim in the output as GitHub's own word, which is
what [`quinjet pr logs`](./logs.md) prints in its heading.

Rows are sorted by workflow name and then by check name, both lower-cased, and
never by status. That is deliberate: a pending check turning green must not make
the rows jump under a cursor during a live refresh, and it also means the same
pull request produces the same order on every read. Names longer than 44
characters are truncated with an ellipsis in the text form only; `--json` always
carries the whole name.

The listing is cached for 30 seconds under
`checks-v1\n<repository url>\n<number>\n<head oid>`. Keying on the head commit
means a force push invalidates it immediately rather than after the TTL, and
30 seconds is short because this is the one thing here that genuinely changes
minute to minute.

## The `--watch` and `--exit-code` contract

Without `--watch`, the verb reads once and exits 0 regardless of what the checks
say, unless `--exit-code` is given. With `--exit-code`, the exit code is 1 when
any check is `failed` or `pending`, and 0 otherwise. Note that pending counts as
unhappy: a single reading of a pull request whose CI has not finished is a
failure for `--exit-code`, because the question it answers is "is this green
right now", not "did it finish".

State this plainly, because it is the one place in Quinjet where a non-zero exit
still carries output: the listing is printed in full, on stdout, and only then
does the process exit 1. `quinjet pr checks 8 --exit-code --json > checks.json`
leaves a complete document in `checks.json` and returns 1, and the same is true
of the final frame under `--watch`. The exit code is a verdict on the pull
request, not a report that the command failed, so it does not follow the usual
rule that a failing verb writes nothing to stdout.

With `--watch`, the verb reads every `--interval` seconds, forcing a refresh
each time, and stops when both of these hold:

- no check is `pending`, and
- the list is not empty.

Then it exits 1 when any check is `failed`, and 0 when none is. A `skipped`,
`cancelled` or `unknown` run is not a failure and does not stop the exit code
being 0, so in practice `--watch` exits 0 when the pull request went green and 1
when it did not. `--exit-code` is not consulted under `--watch`; passing both is
harmless.

The second condition is the one that surprises people. A pull request that
reports no checks at all never settles, and `--watch` will poll forever, because
a workflow that has not yet been scheduled is indistinguishable from one that
does not exist. If a repository may legitimately have no CI, use a single read
with `--exit-code` instead, or wrap the watch in `timeout`.

While watching, each non-final read is followed by a line on stdout:

```text
watching, refreshing every 5s (Ctrl+C to stop)
```

When stdout is a terminal the screen is cleared before each frame; when it is
redirected the frames simply append, so a watch written to a file is a readable
log. Under `--json --watch` each read is one compact line, which is what makes
`quinjet pr checks 8 --json --watch | jq .` show a reading at a time.

`--json` shape, one object. `fromCache` says whether this reading came off disk
rather than from GitHub, which is always `false` under `--watch`. `link` is the
check run's URL and is what [`quinjet pr logs`](./logs.md) reads the Actions job
id out of; a check whose `link` does not end in `/job/<id>` has no readable log:

```json
{
  "checks": [
    {
      "name": "Format, lint, and test (ubuntu-latest)",
      "workflow": "CI",
      "state": "SUCCESS",
      "status": "passed",
      "description": "",
      "link": "https://github.com/pulkitxm/quinjet/actions/runs/31884531392/job/95011787585",
      "startedAt": "2026-08-15T12:24:50Z",
      "completedAt": "2026-08-15T12:25:30Z"
    }
  ],
  "fromCache": true
}
```

`status` is one of `pending`, `passed`, `failed`, `skipped`, `cancelled`,
`unknown`. `startedAt` and `completedAt` are empty strings rather than `null`
when GitHub has not set them, and the duration in the text form is derived from
the pair, printed as `49s`, `2m 0s` or `1h 4m` and omitted entirely when either
stamp is missing.

Examples:

```bash
quinjet pr checks 8
quinjet pr checks 8 --json
quinjet pr checks 8 --exit-code
quinjet pr checks 8 --watch --interval 15
quinjet pr checks 8 --json --watch | jq -r '.checks[] | select(.status == "failed") | .name'
```

```console
$ quinjet pr checks 8
+  passed    Format, lint, and test (macos-latest)        CI  49s
+  passed    Format, lint, and test (ubuntu-latest)       CI  40s
+  passed    Format, lint, and test (windows-latest)      CI  2m 0s
+  passed    Minimum supported Rust                       CI  21s
+  passed    Package validation                           CI  39s
+  passed    label                                        Label PRs  5s
+  passed    lychee                                       Link check  7s

7 passed, 0 pending, 0 failed
```

The glyph column is `+` passed, `x` failed, `o` pending, `-` skipped, `/`
cancelled, `?` unknown. The summary line counts only the first three, so a pull
request with two skipped runs and nothing else reports `0 passed, 0 pending,
0 failed`. A pull request with no checks prints `No checks reported` and nothing
else.

In CI, the useful form is a watch with a timeout around it, because the watch
itself has no deadline:

```bash
#!/usr/bin/env bash
set -euo pipefail

if timeout 30m quinjet pr checks "$PR" --watch --interval 30; then
  echo "green"
else
  status=$?
  if [ "$status" -eq 124 ]; then
    echo "CI did not settle within 30 minutes" >&2
  else
    quinjet pr checks "$PR" --json \
      | jq -r '.checks[] | select(.status == "failed") | .name'
  fi
  exit 1
fi
```

`--interval 30` there rather than the default 5 is worth copying: every read is
a forced `gh pr checks` call, so a long watch at the default rate is twelve
requests a minute against your rate limit for the whole life of the job.

## Where to go next

- [`quinjet pr logs`](./logs.md) for the log behind a red row
- [`quinjet pr`](./README.md), the rest of this group and its caching rules
- [Conventions and contracts](../conventions.md) for the shared exit-code table
  and what `--watch` does to the one-document rule
- [All `quinjet` commands](../README.md)
