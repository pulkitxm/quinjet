# `quinjet work publish`

Records a session's work as one commit on its own branch. It writes nothing
outside the machine.

Usage:

```bash
quinjet work publish <session> [--message <MESSAGE>] [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<SESSION>` | string | required | The session identifier. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-m, --message <MESSAGE>` | string | derived | The commit message. Without one, `work: <id> on #<number> from <source>`. |
| `--yes` | flag | off | Confirms. Without it the command reports what it would commit and stops. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

## What publishing does not do

It does not push. It does not comment. It does not resolve a thread. It does not
merge, close or edit the pull request. Those are the four things a session is
forbidden from, and publishing a session does not become an exception to them.

What it does instead is name them:

```console
$ quinjet work publish w42-1
Would commit 2 file(s) onto quinjet/work/w42-1 as:
  work: w42-1 on #42 from feedback
  src/lib.rs
  tests/lib.rs
nothing has been verified on this session yet

publishing writes one local commit and nothing else. To go further, run:
  git push origin quinjet/work/w42-1
  quinjet pr gate 42
  quinjet pr feedback 42 --unresolved

Pass --yes to record the commit.
```

Those commands are printed, not run, and running them is your decision. That is
the whole design: a coding process can produce a commit, and a person decides
whether it reaches anybody.

## What goes into the commit

Everything the session changed and everything it added: the publish plan lists
both the tracked files [`quinjet work diff`](./diff.md) reports and the
untracked files `git ls-files --others --exclude-standard` reports, and the
commit stages all of them. The preview lists exactly that set, so nothing
arrives in the commit that was not on screen first.

A session that has changed nothing is not committed, with or without `--yes`.
The verb says `Nothing to publish` and exits 0.

## Verification is reported, not enforced

If a recorded verification last failed, the preview names it:

```text
verification `cargo test` last failed
```

and if nothing has been recorded at all, it says that instead. Neither stops the
publish. A local commit on a branch nobody has pushed is not dangerous, and
refusing to record work because a test is red would just push people to commit
around Quinjet. What matters is that the state is stated rather than assumed.

`--json` before `--yes` is the plan:

```json
{
  "id": "w42-1",
  "branch": "quinjet/work/w42-1",
  "startOid": "a1d09f7b3c5e2a1d09f7b3c5e2a1d09f7b3c5e2a",
  "files": ["src/lib.rs", "tests/lib.rs"],
  "message": "work: w42-1 on #42 from feedback",
  "verified": false,
  "failing": "cargo test",
  "next": ["git push origin quinjet/work/w42-1", "quinjet pr gate 42"]
}
```

After `--yes` it is the session record, with the new commit in `checkpoints` and
`state` moved to `published`.

## Exit codes

| Code | When |
| --- | --- |
| 0 | The plan was printed, or the commit was recorded, or there was nothing to publish. |
| 1 | The session has no worktree, or its worktree has been deleted. |
| 3 | No session has that identifier. |

## Where to go next

- [`quinjet pr gate`](../pull-request/gate.md), the next thing to ask after pushing
- [`quinjet work abort`](./abort.md) to throw the session away instead
- [`quinjet work`](./README.md), the group and its boundary
- [All `quinjet` commands](../README.md)
