# Watching CI from a script

This is the page for using Quinjet without a terminal: waiting for a pull
request's checks in a shell script, deciding what to do with the result, and
reading the log of the job that failed without opening a browser.

Three verbs carry the whole workflow.
[`quinjet pr checks`](../cli/pull-request/checks.md) lists the runs and can
block until they settle. [`quinjet pr logs`](../cli/pull-request/logs.md)
prints one run's steps and its output.
[`quinjet pr view`](../cli/pull-request/view.md) is the metadata around them.
All three need `gh` on `PATH` and authenticated, all three read GitHub and
never touch your checkout, and all three take `--json`. The
[conventions page](../cli/conventions.md) has the rules they share; this page is
about putting them together.

## Blocking until the checks settle

`--watch` re-reads the checks until nothing is pending, then exits with a code
that says whether they went green:

```bash
if quinjet pr checks 8 --watch --interval 15; then
  echo "green"
else
  echo "not green"
fi
```

The exit code is 0 only when no check is `failed` and none is `pending`. A
`skipped` or `cancelled` check does not make the exit non-zero, because neither
is a failure of the code. That matters for a repository with conditional jobs:
a workflow that skips its deploy job on a fork still exits 0.

`--watch` refreshes on its own clock: every 5 seconds by default, with a floor
of 2 seconds, so `--interval 1` and `--interval 0` both become 2. Each read is
forced past the cache, which is what makes it a watch rather than a replay.

Without a terminal, the output is appended rather than repainted, so a
redirected watch produces a readable log instead of a file full of escape
sequences:

```bash
quinjet pr checks 8 --watch --interval 30 >> ci.log 2>&1
```

Two things will hang a watch, and both have the same fix. A pull request with
no checks at all never settles, because an empty list is not a finished one. A
workflow waiting on a required reviewer or a deployment approval stays pending
for as long as the person takes. Put a ceiling on it:

```bash
timeout 30m quinjet pr checks 8 --watch --interval 20
case $? in
  0)   echo "green" ;;
  124) echo "gave up waiting" ;;
  *)   echo "red" ;;
esac
```

`timeout` exits 124 when it fires, which is distinct from anything Quinjet
produces.

## When blocking is wrong

`--watch` holds the process. A cron job, a status line or a webhook handler
wants one reading and an exit code, which is `--exit-code`:

```bash
quinjet pr checks 8 --exit-code --json > checks.json || notify "checks are not green"
```

Without `--exit-code`, `quinjet pr checks` exits 0 whatever the checks say,
because listing them succeeded. That is the right default for a human and the
wrong one for a script, so ask for it explicitly.

A plain `quinjet pr checks 8` answers from the cache when the list was read in
the last thirty seconds, which is what makes it cheap to call in a prompt or a
status line. Add `--refresh` when the answer must be current, and remember that
`--watch` already refreshes on every tick.

## Reading the job that failed

Naming a check reads its steps and its log:

```bash
quinjet pr logs 8 "Minimum supported Rust"
```

The name can be a unique fragment, matched case-insensitively, so
`quinjet pr logs 8 package` finds `Package validation`. An exact match always
wins over a partial one. A fragment matching more than one run is exit 3 with
the candidates on stderr:

```console
$ quinjet pr logs 8 Format
error: `Format` matches more than one check
hint: name one of: Format, lint, and test (macos-latest), Format, lint, and test (ubuntu-latest), Format, lint, and test (windows-latest)
```

A name matching nothing is also exit 3, and lists every check on the pull
request. Both messages go to stderr, so a script can print them straight
through without polluting the data on stdout.

The useful part is that the output is attributed to steps rather than being one
undifferentiated archive. Each line is placed under the step that was running
when it was written, which is what turns "the job failed somewhere" into "step
5 failed":

```console
$ quinjet pr logs 8 "Minimum supported Rust"
+  Minimum supported Rust  (CI · SUCCESS)
https://github.com/pulkitxm/quinjet/actions/runs/31884531392/job/95011787569

+  1. Set up job  1s
  Current runner version: '2.336.0'
  ...
```

So the whole failure path is one pipeline: wait, ask which run is red, read
that run.

```bash
#!/usr/bin/env bash
set -euo pipefail

pr=${1:?usage: watch-pr <number>}

if timeout 30m quinjet pr checks "$pr" --watch --interval 20; then
  echo "all green"
  exit 0
fi

quinjet pr checks "$pr" --json |
  jq -r '.checks[] | select(.status == "failed") | .name' |
  while IFS= read -r name; do
    printf '\n===== %s =====\n' "$name"
    quinjet pr logs "$pr" "$name" | tail -n 60
  done
exit 1
```

Each refreshed read writes the check list back to the cache, and that entry
lives thirty seconds, so the `--json` read immediately after the watch is
answered from disk rather than costing another request.

To tail a job while it is still running rather than reading it afterwards, give
`pr logs` its own `--watch`. It stops on its own the moment the run finishes,
and exits 1 if the run failed:

```bash
quinjet pr logs 8 "Format, lint, and test (ubuntu-latest)" --watch --interval 15
```

The default here is 8 seconds with a floor of 3. A finished run is never
re-read, so pointing `--watch` at one prints it once and exits.

Some checks have no log to read at all. A status context posted by a
third-party service, or a merge-queue check, has no GitHub Actions job behind
it, so `quinjet pr logs` exits 4 with
`<name> does not publish logs through GitHub Actions`. Exit 4 is worth handling
separately from exit 1: the run exists and is red, there is simply nothing to
print. The URL to open by hand is the `link` field of `pr checks --json`.

## Parsing with `jq`

Every read takes `--json`, one document per invocation, pretty-printed. Under
`--watch` each read is one compact line instead, so a stream can be consumed as
it arrives.

`pr checks --json` is an object, not an array. The runs are under `checks`, and
`fromCache` says whether the answer came off disk:

```bash
quinjet pr checks 8 --json | jq -r '.checks[] | [.status, .name] | @tsv'
```

```text
passed    Format, lint, and test (macos-latest)
passed    Format, lint, and test (ubuntu-latest)
passed    Format, lint, and test (windows-latest)
passed    Minimum supported Rust
passed    Package validation
passed    label
passed    lychee
```

`status` is Quinjet's own normalization and is one of `passed`, `failed`,
`pending`, `skipped`, `cancelled` or `unknown`. `state` beside it is GitHub's
raw string, such as `SUCCESS`. Match on `status`; it is the field the exit code
is computed from.

Useful one-liners:

```bash
quinjet pr checks 8 --json | jq -r '.checks[] | select(.status=="failed") | .name'
quinjet pr checks 8 --json | jq -r '.checks[] | select(.status=="pending") | .link'
quinjet pr checks 8 --json | jq '[.checks[] | select(.status=="failed")] | length'
quinjet pr view 8 --json | jq -r '.state, .headOid, "\(.additions)/\(.deletions)"'
```

A watched stream is one object per line, so `jq -c` turns it into a feed:

```bash
quinjet pr checks 8 --json --watch |
  jq -c '{pending: [.checks[]|select(.status=="pending")|.name]}'
```

`pr logs --json` is an object with `steps`, `looseLines`, `truncated`,
`unavailable` and `logPending`. `looseLines` is output written before the first
step or after the last one, which is where a runner reports provisioning and
teardown failures, so a job that died before doing anything has empty `steps`
and everything in `looseLines`. `logPending` is true only for the first seconds
of a job, before GitHub has published anything; `truncated` says a size cap was
reached.

```bash
quinjet pr logs 8 "Minimum supported Rust" --json |
  jq -r '.steps[] | select(.conclusion != "success") | .name'

quinjet pr logs 8 "Minimum supported Rust" --json |
  jq -r '.steps[].lines[] | select(.severity=="error") | .text'
```

`severity` comes from the runner's own `##[error]`, `##[warning]`, `##[notice]`
and `##[command]` markers, which is a more reliable filter than grepping for
the word "error". Step `number` is GitHub's numbering and can have gaps, so
sort or select on it rather than assuming `1..n`.

A verb that fails writes nothing on stdout, so
`quinjet pr checks 8 --json > checks.json` either writes a whole document or
writes nothing, and a half-written file never reaches `jq`. Exit 1 from
`--watch` or `--exit-code` is the exception, and it is the useful one: the
document is complete on stdout, and the code carries the verdict.

## Exit codes to handle

| Code | What it means here |
| --- | --- |
| 0 | The read succeeded. For `--watch` and `--exit-code`, every check also passed. |
| 1 | A check is failed or pending under `--watch` or `--exit-code`, or `gh` failed, or the pull request could not be read. |
| 2 | The command line was wrong: an unknown flag, a missing number, an interval that will not parse. |
| 3 | A name matched nothing or matched more than one thing: the check name, or a `--repo` that no remote of this checkout points at. |
| 4 | The check run exists but publishes no GitHub Actions log. |

Codes 1 and 3 are the two a CI script normally distinguishes: 1 is a red build,
3 is a script that has the wrong name in it.

## Choosing an interval

Watching is not free, and the arithmetic is simple enough to do before choosing
a number.

`quinjet pr checks --watch` is one `gh pr checks` call per tick. At the default
5 seconds that is 720 calls an hour, at 30 seconds it is 120. It is a
reasonable rate for a run you are actually waiting on and a poor one for a
background monitor.

`quinjet pr logs --watch` is heavier. Each tick re-reads the check list, reads
the job's steps, and downloads the log again, because a running job's output
has no stable identity to cache under: re-reading it is exactly what makes it
tail. That is three requests per tick, and the log download is the whole
archive every time, not a delta, up to the 8 MiB and 200,000 line caps. A
chatty job producing a few megabytes of output, tailed at the default 8
seconds, moves gigabytes an hour for no benefit. Raise the interval for a long
job.

GitHub's documented limit for an authenticated user token is 5,000 requests an
hour, shared with every other tool using the same token, and the token an
Actions job gets by default is more limited again. Practical settings:

- Waiting on a build in your own terminal: leave the defaults.
- A script blocking a merge: `--interval 20` or `--interval 30`. The wait is
  bounded by the build, not by the poll.
- Anything running unattended, or several pull requests at once:
  `--interval 60` and a `timeout` around it.
- Tailing a talkative job: `--interval 30`, or read it once after it finishes,
  which is a single cached request.

`--interval` only means anything alongside `--watch`. On a single read it
parses and is ignored, so `quinjet pr checks 8 --interval 30` is just
`quinjet pr checks 8`.

The caching rules do the rest. A settled run's steps and log are keyed by job
id and kept forever, so reading a finished job twice costs one request and one
disk read. Metadata lives five minutes, the check list thirty seconds,
repository identity a day. `--watch` bypasses those deliberately;
`quinjet pr logs 8 <name>` on a finished run does not, so it is safe in a loop.

## From inside a CI job

Quinjet works in a workflow as long as `gh` is authenticated and the repository
is checked out. The pull-request number is in the event payload:

```yaml
- uses: actions/checkout@v5
- name: Wait for the other checks
  env:
    GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    NUMBER: ${{ github.event.pull_request.number }}
  run: |
    timeout 20m quinjet pr checks "$NUMBER" --watch --interval 30
    quinjet pr checks "$NUMBER" --json > checks.json
```

Two constraints to know. Quinjet resolves a pull-request number through the
remotes of the checkout it is standing in, so it needs a real clone, not a bare
API token; `--repo owner/name` selects among the repositories those remotes
point at, and a repository no remote points at is exit 3 with
`no remote of this checkout points at ...`. And a job that watches its own
workflow will wait for itself: exclude your own check, or watch from outside.

`gh` inherits `GH_TOKEN`, `GH_HOST` and `GH_ENTERPRISE_TOKEN` exactly as it
would if you ran it yourself, because Quinjet runs it with prompts, paging,
color and update checks disabled and hands it an argument array rather than a
shell string. GitHub Enterprise hosts configured in `gh` work unchanged.

The cache is worth pointing somewhere deliberate in CI. Its root is the first of
`$QUINJET_CACHE_DIR`, `%LOCALAPPDATA%\quinjet\cache` on Windows,
`$XDG_CACHE_HOME/quinjet`, `~/Library/Caches/quinjet` on macOS, and
`~/.cache/quinjet`. It is bounded to 128 MiB and 2,048 entries, and never stores
credentials. Setting `QUINJET_CACHE_DIR` to a path inside the workspace makes
it cacheable between runs; leaving it alone is fine for a one-shot job.

## Why not `gh pr checks`

`gh pr checks --watch` covers the waiting part, and if that is all a script
needs it is one fewer tool to install. Quinjet is worth reaching for when the
next question is why:

- One tool covers the wait and the log. `gh` will tell you a check failed;
  reading the job then means `gh run view --log-failed`, or opening the run in
  a browser and finding the job by hand.
- The log arrives attributed to steps. Quinjet aligns the runner's timestamped
  lines against the steps API, so `--json` gives you steps with their status,
  duration and their own output, and `severity` on each line. A raw archive
  gives you neither.
- Names are forgiving. A unique fragment is enough, and an ambiguous one lists
  the candidates rather than guessing.
- It is the same cache the terminal interface uses. A pull request read on
  screen is already warm on the command line, and a settled run's log, read
  once, is a disk read forever after.
- The exit codes separate the cases. 1 for a red build, 3 for a name that does
  not exist, 4 for a check with no readable log, so a script can respond to
  each differently.

The reverse also holds. `gh` is what talks to GitHub underneath, and anything
Quinjet does not model, creating a pull request, approving one, re-running a
job, is a job for `gh` directly.

## Where to go next

- [`quinjet pr`](../cli/pull-request/README.md) for every flag on `checks`,
  `logs`, `view`, `files`, `diff` and `conversation`
- [Conventions and contracts](../cli/conventions.md) for the `--json`
  guarantee, the caching rules and the full exit-code table
- [The terminal interface](../cli/tui.md) for the same live checks with a
  screen, and the map from its keys to these verbs
- [All `quinjet` commands](../cli/README.md)
