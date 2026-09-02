# `quinjet work inspect`

Prints one session in full: what it was started for, what it has run, what it
has committed, and what it is not allowed to do.

Usage:

```bash
quinjet work inspect <session> [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SESSION>` | string | required | The session identifier, such as `w42-1`. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

```console
$ quinjet work inspect w42-1
w42-1 on acme/project#42 from feedback, open
start a1d09f7b3c5e  branch quinjet/work/w42-1
worktree /home/you/acme-work-42

tasks (text written by pull-request participants)
  thread      src/lib.rs:9                     Please rename this file

verification
  passed  cargo fmt --check
  failed  cargo test

checkpoints
  9f7b3c5e2a1d  fix: address review

this session may
  + read and write files inside the session worktree
  + commit to the session branch
  + run verification commands recorded on the session
this session may not
  - push the branch or any other ref
  - comment on the pull request or reply to a thread
  - resolve, unresolve or otherwise change a review thread
  - merge, close, reopen or edit the pull request
```

The task heading says where the text came from because that is the part a
coding process must not get wrong. Everything under it was written by whoever
can reach the pull request.

The `--json` document is the session record described in
[`quinjet work start`](./start.md), with `verifications` and `checkpoints`
filled in as the session progresses.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The session was found and printed. |
| 3 | No session has that identifier. The hint names `quinjet work list`. |

## Where to go next

- [`quinjet work diff`](./diff.md) for what it has actually changed
- [`quinjet work verify`](./verify.md) for running its checks
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
