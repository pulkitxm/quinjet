# `quinjet branch delete`

Deletes a local branch, once you have said `--yes`.

Usage:

```bash
quinjet branch delete <NAME> [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NAME>` | string | required | The local branch to delete. A short name, not a full ref. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--yes` | flag | off | Confirm. Without it the command reports what it would delete and changes nothing. |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Any directory inside the worktree works. |
| `--json` | flag | off | Prints `{"message": ...}` on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Without `--yes` nothing is read and nothing is run: Quinjet prints one sentence
naming the branch and exits **0**. It does not check that the branch exists, so
the dry run reports the same thing for a name that was never there. A script
that forgets `--yes` therefore sees success and no deletion, which is the
[`--yes` rule](../conventions.md#flags-and-values) everywhere it appears.

With `--yes` the whole verb is

```bash
git branch --delete -- <NAME>
```

`--delete`, never `--delete --force`, and that is the important part. Whether a
branch may be deleted is Git's judgment, not Quinjet's, and Git refuses a
branch whose commits are not reachable from its upstream or from HEAD:

```console
$ quinjet branch delete unmerged --yes
error: Git command failed: error: the branch 'unmerged' is not fully merged.
If you are sure you want to delete it, run 'git branch -D unmerged'
```

That advice is Git's, and it is the way out: Quinjet has no force delete, on
purpose. Note that "merged" is a question about commits, so a branch whose pull
request was squash-merged or rebase-merged on GitHub still looks unmerged here,
because none of its commits exist on `main` by object id.

Two other refusals come from Git, both exit 1:

- The branch you are on, or one another worktree has checked out:
  `error: cannot delete branch 'main' used by worktree at '<path>'`. The
  terminal interface catches this itself and refuses with
  `Cannot delete the current branch`; the command line lets Git answer.
- A remote-tracking name:
  `error: branch 'origin/main' not found.` followed by
  `Did you forget --remote?`. This verb only deletes local branches. Deleting a
  branch on a remote is not something Quinjet does at all.

Deleting a branch never touches your working tree, your index or your stashes,
and never deletes commits. The commits stay in the object database until Git
garbage-collects unreachable objects, and `git reflog` still knows where the
branch pointed, so an accidental delete is recoverable with Git for as long as
the reflog keeps the entry.

`--json` shape, an object with one key. The dry run and the real delete have the
same shape and the same exit code, so read the message rather than the status:

```json
{
  "message": "Would delete `topic`. Pass --yes to delete it."
}
```

```json
{
  "message": "Deleted topic"
}
```

Examples:

```bash
quinjet branch delete topic
quinjet branch delete topic --yes
quinjet branch delete topic --yes --json
quinjet branch delete -C ~/code/project stale/thing --yes
```

```console
$ quinjet branch delete topic
Would delete `topic`. Pass --yes to delete it.
```

```console
$ quinjet branch delete topic --yes
Deleted topic
```

## Where to go next

- [`quinjet branch`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
