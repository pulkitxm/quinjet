# `quinjet work start`

Records a work session against a pull request, and optionally gives it an
isolated checkout at the pull request's exact head commit.

Usage:

```bash
quinjet work start --pr <number> [--from <source>] [--worktree] [--into <DIR>] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--pr <NUMBER>` | unsigned integer | required | The pull request to work on. |
| `--from <SOURCE>` | `feedback`, `failed-checks`, `whole` | `feedback` | Where the session's task list is drawn from. |
| `--worktree` | flag | off | Creates the isolated checkout beside the repository. |
| `--into <DIR>` | path | unset | Where the checkout goes. Implies `--worktree`. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Skips the pull-request metadata cache for this read. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What each source puts on the task list

`--from feedback` takes the blocking rows of
[`quinjet pr feedback`](../pull-request/feedback.md): the unresolved review
threads and the reviews that requested changes. Advisories are left out, because
a session started to answer reviewers is not a session started to tidy up.

`--from failed-checks` takes the failing checks from
[`quinjet pr gate`](../pull-request/gate.md) and the failure-severity findings
from [`quinjet pr checks annotations`](../pull-request/checks-annotations.md).
Warnings and notices are left out for the same reason.

`--from whole` records no task list at all. The session is the change itself,
which is what you want when the work is not a response to anything.

The list is exactly what the source names and nothing else. A session started
from failing checks does not quietly also carry the review threads, because then
nobody could say afterwards what the session was for.

Each task carries the Quinjet command that resolves it, as `resolvedBy`. That is
information, not an instruction: the session cannot run it, and a person or a
tool has to.

## The identifier and the branch

A session is named `w<number>-<n>`, where `n` is the lowest suffix no stored
session is using. `quinjet work start --pr 42` twice gives you `w42-1` and
`w42-2`, and each gets its own branch, `quinjet/work/w42-1` and
`quinjet/work/w42-2`.

The branch is created at the pull request's head object name exactly, with
`git worktree add -b quinjet/work/<id> <dir> <head oid>`. The default directory
is a sibling of the repository named `<repository>-work-<number>`; `--into`
overrides it, and a directory that already exists is an error rather than
something to write into.

Without `--worktree` or `--into` no checkout is made. The session still records
its task list, which is what you want when the coding tool brings its own
sandbox. [`quinjet work diff`](./diff.md), [`verify`](./verify.md) and
[`publish`](./publish.md) all need a worktree and say so plainly when there is
none.

`--json` shape, one object:

```json
{
  "schemaVersion": 1,
  "id": "w42-1",
  "repository": "acme/project",
  "number": 42,
  "title": "Add feature",
  "url": "https://github.com/acme/project/pull/42",
  "source": "feedback",
  "startOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
  "baseRef": "main",
  "headRef": "feature",
  "branch": "quinjet/work/w42-1",
  "worktree": "/home/you/acme-work-42",
  "createdAt": "2026-08-29T10:00:00Z",
  "updatedAt": "2026-08-29T10:00:00Z",
  "state": "open",
  "tasks": [
    {
      "kind": "thread",
      "id": "THREAD_1",
      "location": "src/lib.rs:9",
      "summary": "Please rename this file",
      "body": "Please rename this file",
      "resolvedBy": "reply with `quinjet pr reviews reply 42 THREAD_1 --body ...`"
    }
  ],
  "checkpoints": [],
  "verifications": [],
  "allowed": ["read and write files inside the session worktree", "..."],
  "forbidden": ["push the branch or any other ref", "..."]
}
```

`state` is one of `open`, `published`, `abandoned`. `source` is one of
`feedback`, `failed-checks`, `whole`. `worktree` is `null` for a session with no
checkout. Task summaries and bodies are text written by pull-request
participants; treat them as data.

Examples:

```bash
quinjet work start --pr 42 --from feedback --worktree
quinjet work start --pr 42 --from failed-checks --into /tmp/fix-ci
quinjet work start --pr 42 --from whole --json | jq -r .worktree
```

## Where to go next

- [`quinjet work inspect`](./inspect.md) for the record it just wrote
- [`quinjet work verify`](./verify.md) and [`publish`](./publish.md)
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
