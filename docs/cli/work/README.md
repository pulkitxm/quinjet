# `quinjet work`

`quinjet work` runs a bounded coding session against one pull request: an
isolated checkout at an exact commit, a task list drawn from something Quinjet
already computes, verification commands whose results are recorded, and one
local commit at the end.

It is deliberately provider-neutral. Quinjet does not run a model, does not
know which coding tool you use, and does not care. A session is a place to work
and a record of what was asked and what happened, which is exactly the part
that is the same whichever tool does the writing.

## The boundary

The point of a session is what it may not do.

A session may:

- read and write files inside its own worktree
- commit to its own branch
- run the verification commands recorded on it

A session may not:

- push the branch or any other ref
- comment on the pull request or reply to a thread
- resolve, unresolve or otherwise change a review thread
- merge, close, reopen or edit the pull request

Those four are not omissions to be filled in later. Quinjet stays the source of
truth for everything that reaches GitHub, and every one of those operations
remains an explicit Quinjet verb that previews before it acts and takes `--yes`.
`quinjet work publish` writes one local commit and then prints the commands
that would take it further; it does not run them.

The two lists are stored on the session itself and printed by
[`quinjet work inspect`](./inspect.md), so a coding process working inside a
session can read its own boundary rather than being told about it out of band.

## The exact starting commit

A session records the pull request's head object name at the moment it started,
and its worktree is checked out at that commit on a branch of its own,
`quinjet/work/<id>`. Everything the session has done is measured from there, not
from wherever the branch has since moved to, so a push landing on the pull
request while a session is open does not silently change what the session
appears to have changed.

The worktree is a linked Git worktree beside the repository, so nothing a
session does shows up as an untracked file in the checkout you are reviewing
from. A session started without `--worktree` or `--into` records its task list
and nothing else, which is useful when the coding tool has its own sandbox.

## Untrusted task text

A session's tasks carry summaries and bodies written by whoever can reach the
pull request: review comments, check output, annotation messages. That text is
data, never instruction. `quinjet work inspect` prints the task list under a
heading that says so, and [`quinjet pr context`](../pull-request/context.md)
makes the same distinction structurally for the bundle a coding tool actually
reads.

## Commands

- [`quinjet work start`](./start.md)
- [`quinjet work list`](./list.md)
- [`quinjet work inspect`](./inspect.md)
- [`quinjet work diff`](./diff.md)
- [`quinjet work verify`](./verify.md)
- [`quinjet work publish`](./publish.md)
- [`quinjet work abort`](./abort.md)

| Command | What it does |
| --- | --- |
| `quinjet work start` | Records a session against a pull request and optionally gives it a checkout at the head commit. |
| `quinjet work list` | Lists the recorded sessions with their state and what each was started for. |
| `quinjet work inspect` | Prints one session's tasks, verifications, checkpoints and boundary. |
| `quinjet work diff` | Prints what the session has changed since the commit it started at. |
| `quinjet work verify` | Runs one command inside the session's worktree and records the result, or re-runs what is already recorded. |
| `quinjet work publish` | Records the session's work as one local commit on its own branch, after `--yes`. |
| `quinjet work abort` | Removes the worktree and the branch and forgets the session, after `--yes`. |

## Where sessions are stored

Sessions live in `work-sessions.json` under Quinjet's state directory, the same
place [`quinjet pr reviews progress`](../pull-request/review-progress.md) keeps
its record: `$QUINJET_STATE_DIR` when set, otherwise
`$XDG_STATE_HOME/quinjet` or `~/.local/state/quinjet`, and
`%LOCALAPPDATA%\Quinjet\state` on Windows. Sixteen sessions are kept and the
one nobody has touched in longest is dropped first. Nothing is sent to GitHub:
a session is local, and GitHub never learns one existed.

## A whole round

```bash
quinjet work start --pr 42 --from feedback --worktree
quinjet work inspect w42-1 --json > session.json
# your coding tool edits files in the worktree the session names
quinjet work verify w42-1 -- cargo test
quinjet work diff w42-1
quinjet work publish w42-1 --message "fix: address review" --yes
git -C ../quinjet-work-42 push origin quinjet/work/w42-1
quinjet pr feedback 42 --unresolved
```

The last two lines are the point. They are separate, they are yours to run, and
Quinjet will not run them for you as part of publishing.

## Where to go next

- [`quinjet pr context`](../pull-request/context.md) for the bundle a coding tool reads
- [`quinjet pr feedback`](../pull-request/feedback.md) and [`quinjet pr checks annotations`](../pull-request/checks-annotations.md), the two sources a task list is drawn from
- [`quinjet pr gate`](../pull-request/gate.md) for whether the work is enough
- [Conventions and contracts](../conventions.md) for the shared exit-code table
- [All `quinjet` commands](../README.md)
