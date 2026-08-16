# `quinjet pr logs`

Prints one check run's steps and its GitHub Actions log, with each line attached
to the step that produced it.

Usage:

```bash
quinjet pr logs <number> <check> [--repo <owner/name>] [--refresh] [--watch] [--interval <seconds>] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `<CHECK>` | string | required | The check run to read, by name. Matched exactly first, then case-insensitively as a substring. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the 30 second cache when reading the check list to resolve `<CHECK>`. It does not affect the log, which is either immutable or never cached. |
| `--watch` | flag | off | Keeps re-reading while the run is still going, then exits with the run's verdict. |
| `--interval <SECONDS>` | unsigned integer | `8` | Seconds between reads while watching. Values below 3 are raised to 3. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout, or one compact object per read under `--watch`. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## Finding the run

`pr logs` starts by reading the same check list [`quinjet pr checks`](./checks.md)
prints, then picks a run out of it. An exact name match wins outright, so a
matrix job whose full name is `Format, lint, and test (ubuntu-latest)` is always
reachable by that name even if it is a substring of nothing. Only when no name
matches exactly does Quinjet fall back to a case-insensitive substring search,
and that search must land on exactly one run:

```console
$ quinjet pr logs 8 nosuch
error: no check on this pull request is called `nosuch`
hint: the checks are: Format, lint, and test (macos-latest), Format, lint, and test (ubuntu-latest), Format, lint, and test (windows-latest), Minimum supported Rust, Package validation, label, lychee
```

```console
$ quinjet pr logs 8 Format
error: `Format` matches more than one check
hint: name one of: Format, lint, and test (macos-latest), Format, lint, and test (ubuntu-latest), Format, lint, and test (windows-latest)
```

Both exit 3, and both list what you could have typed instead.

## Reading the run

A check run's only link to the machinery behind it is its `link` field, which
for GitHub Actions ends in `/actions/runs/<run>/job/<job>`. Quinjet takes the
part after the last `/job/`, cuts it at the first `?`, `#` or `/`, and parses it
as the Actions job id. A check whose link is not of that shape has no job id and
therefore nothing to read: every third-party status context and every
merge-queue check falls in that category. That is one of the two conditions that
produce exit 4, the other being a check GitHub has published nothing for yet.
Both are covered under [the two empty states](#the-two-empty-states):

```console
$ quinjet pr logs 12 codecov/project
error: codecov/project does not publish logs through GitHub Actions
```

With a job id, two reads follow:

```text
gh api repos/<owner>/<name>/actions/jobs/<job> \
  --jq '.steps[]? | [((.number // 0)|tostring), (.name // ""), (.status // ""), (.conclusion // ""), (.started_at // ""), (.completed_at // "")] | @tsv'

gh api --allow-escape-sequences repos/<owner>/<name>/actions/jobs/<job>/logs
```

The `--allow-escape-sequences` flag is retried without it when `gh` reports an
unknown flag, because older releases print raw responses unconditionally and do
not know it. A steps read that fails is not an error: the job still has a log
worth showing, so the steps simply come back empty and every line is loose.

A step's status is derived from `status` and `conclusion` together. Anything not
`completed` is `pending`; then `success` is `passed`, `failure`, `timed_out` and
`action_required` are `failed`, `skipped` and `neutral` are `skipped`,
`cancelled` and `stale` are `cancelled`, and anything else is `unknown`.

Step numbers come from GitHub and are printed as given, so they are not
contiguous. A job that skipped conditional steps prints `5.` and then `9.`.

## Attaching lines to steps

Runner output is one line per row: an RFC 3339 UTC timestamp, a space, then the
text. Quinjet strips the byte-order mark, strips ANSI escape sequences, and
turns the workflow command prefixes into a severity: `##[error]` to `error`,
`##[warning]` to `warning`, `##[notice]` to `notice`, `##[command]`, `##[group]`
and `[command]` to `command`, and `##[debug]` to `normal`. `##[endgroup]` and
`##[section]` lines become empty lines rather than disappearing, which is why a
log has blank rows where its groups closed.

Lines are then distributed across steps in one forward pass, comparing whole
seconds. That detail matters: runner lines carry sub-second precision while the
steps API reports whole seconds, so comparing the two as text would put
everything written during a step's final second into the step before it. A line
belongs to a step once its second is at or after that step's `started_at` and
before the next step's.

Output produced before the first step started, or after the last step's
`completed_at`, is returned loose and printed under a `Runner output` heading.
That is where provisioning failures and teardown errors live, and it is the
first place to look when a job failed with nothing under any step.

## The two empty states

They are different, and they exit differently.

`unavailable` means there is nothing to read at all: either the check publishes
no Actions log, or GitHub has published neither steps nor an archive for it. It
carries a reason, prints that reason, and exits 4. The second reason reads
`GitHub has not published anything for this check yet`.

`logPending` means the steps are known but the runner has not written a line
yet. This is only true for the first seconds of a job, because GitHub serves a
growing partial archive from then on. It prints
`Waiting for the runner to write its first output` above the steps and exits 0.

GitHub answers the log endpoint with 404 before the archive exists and with 410
once retention has expired. Neither is treated as an error: the steps are still
shown and the log reads as empty, so an old pull request whose logs have aged
out still lists its steps and their durations.

## Caching and `--watch`

A finished job can never change, so its steps and its log are keyed by the job
id alone under `check-steps-v1` and `check-log-v1` and kept indefinitely, up to
the 8 MiB log cap. A running job has no stable identity to key on and is never
cached: re-reading it is exactly what makes it tail.

`--watch` re-reads on a fixed interval, default 8 seconds and never below 3, and
each round does three things: re-reads the check list with a forced refresh,
re-selects the run by the exact name resolved on the first pass, and re-reads
the entire log. There is no byte-range read on the Actions log endpoint, so
every round transfers the whole archive again. This is why the floor is 3
seconds and the default is 8 rather than the 5 that `pr checks` uses: a
long-running job's log grows, and polling it at two seconds would move megabytes
a minute.

The exit contract for `--watch`:

- It stops as soon as the check is no longer `pending`.
- It exits 1 when the run's final status is `failed`, and 0 for every other
  settled status, including `skipped`, `cancelled` and `unknown`.
- It never exits 4. `unavailable` under `--watch` is rendered as text and the
  loop keeps going until the run settles, so a merge-queue check watched by
  mistake will print its reason and exit 0 rather than telling you it cannot be
  read.
- It cannot become ambiguous after the first pass, because the name it tracks is
  the resolved full name rather than the substring you typed. It can still exit
  3: every round re-selects the run out of a freshly refreshed check list, so a
  check that disappears from a later reading, a re-run that replaces it or a job
  dropped from the matrix, ends the watch with `no check on this pull request is
  called ...` and exit 3.

Between non-final reads it prints `watching, refreshing every 8s (Ctrl+C to
stop)`, clearing the screen first when stdout is a terminal.

## Caps

The log is capped at 8 MiB and at 200,000 lines. Both set `truncated` and both
end the text form with:

```json
[the log reached Quinjet's size cap and was truncated]
```

They are enforced at different moments, and only one of them keeps the result
off disk. The 8 MiB cap stops the read itself: the archive arrives incomplete,
so nothing is written to `check-log-v1` even for a settled job, and every later
read pays for the transfer again. The 200,000 line cap is applied afterwards,
when the bytes are parsed into lines, by which point a settled job's archive has
already been cached whole. Such a job is served from disk on every later read
and still reports `truncated`, because the cut is made again each time the
cached bytes are parsed.

`--json` shape, one object. `steps` holds the steps in number order with their
lines attached, `looseLines` holds everything outside any step, `unavailable` is
`null` or a reason string, and `logPending` is the first-seconds state described
above:

```json
{
  "steps": [
    {
      "number": 4,
      "name": "Run Swatinem/rust-cache@v2",
      "status": "passed",
      "conclusion": "success",
      "startedAt": "2026-08-15T12:25:03Z",
      "completedAt": "2026-08-15T12:25:08Z",
      "lines": [
        {
          "timestamp": "2026-08-15T12:25:03.0114455Z",
          "text": "rustc 1.85.1 (4eb161250 2025-03-15)",
          "severity": "normal"
        },
        {
          "timestamp": "2026-08-15T12:25:03.0115241Z",
          "text": "binary: rustc",
          "severity": "normal"
        }
      ]
    }
  ],
  "looseLines": [],
  "truncated": false,
  "unavailable": null,
  "logPending": false
}
```

`severity` is one of `normal`, `command`, `notice`, `warning`, `error`. It is
the one field that exists only for machines: the text form prints every line the
same way, indented by two spaces, so `--json` is how you find the error lines.
`timestamp` is empty when a line carried no recognizable stamp, which happens
for continuation lines inside a multi-line runner message.

Examples:

```bash
quinjet pr logs 8 "Minimum supported Rust"
quinjet pr logs 8 lychee --json
quinjet pr logs 8 "Format, lint, and test (ubuntu-latest)" --watch
quinjet pr logs 8 "Package validation" --json | jq -r '.steps[].lines[] | select(.severity == "error") | .text'
```

```console
$ quinjet pr logs 8 "Minimum supported Rust"
+  Minimum supported Rust  (CI · SUCCESS)
https://github.com/pulkitxm/quinjet/actions/runs/31884531392/job/95011787569

+  1. Set up job  1s
  Current runner version: '2.336.0'
  Runner Image Provisioner
  Hosted Compute Agent
  Version: 20260729.566

+  2. Run actions/checkout@v5  1s
  Run actions/checkout@v5
  with:
    repository: pulkitxm/quinjet
    token: ***

+  5. Run cargo check --all-features --locked  1s
      Updating crates.io index
      Checking quinjet v0.0.5 (/home/runner/work/quinjet/quinjet)

+  9. Post Run Swatinem/rust-cache@v2  0s

+  10. Post Run actions/checkout@v5  0s

+  11. Complete job  1s
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.51s
  Post job cleanup.
```

The heading is the glyph, the check name, then the workflow and GitHub's own
`state` word in brackets, followed by the run URL. A step with no output prints
its heading and nothing under it, which is what steps 9 and 10 show above.
A step still running shows its elapsed time so far with a trailing ellipsis,
such as `1m 12s…`, instead of a final duration.

In CI, the useful shape is to wait for the checks and then pull the log of
whatever went red:

```bash
#!/usr/bin/env bash
set -uo pipefail

timeout 30m quinjet pr checks "$PR" --watch --interval 30 && exit 0

quinjet pr checks "$PR" --json \
  | jq -r '.checks[] | select(.status == "failed") | .name' \
  | while IFS= read -r name; do
      echo "::group::$name"
      quinjet pr logs "$PR" "$name" --json \
        | jq -r '(.steps[].lines[], .looseLines[]) | select(.severity == "error") | .text'
      echo "::endgroup::"
    done
exit 1
```

Reading `looseLines` as well as the steps is the part worth copying: a job that
died during provisioning has all of its output there and nothing under any step.

## Where to go next

- [`quinjet pr checks`](./checks.md) for the list this verb selects from
- [`quinjet pr`](./README.md), the rest of this group and its caching rules
- [Conventions and contracts](../conventions.md) for exit code 4 and the
  `--watch` exception to the one-document rule
- [All `quinjet` commands](../README.md)
