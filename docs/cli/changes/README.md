# Changing a repository

This section documents seven top-level verbs that move the index, the working
tree, or `HEAD`. They use no network, `gh`, or cache. Branch and stash mutations
have their own command groups. If `git` is on `PATH` and the directory is a
repository, these work.

Reach for them when there is no terminal to hold: a script that stages a
generated file and commits it, a hook that resolves a lock file the same way
every time, a remote shell where the interface would be a nuisance. Each verb is
the same operation the Changes tab performs, so `quinjet stage src/main.rs` and
pressing `s` on that row build the same `GitOperation::Stage` and run the same
`git add`. The interface shows the answer in a toast; the command line prints it
on stdout. See [conventions and contracts](../conventions.md) for why that
cannot drift.

Underneath, each verb is a small, fixed argv. `stage` is `git add -- <paths>`
and `stage --all` is `git add -A`. `unstage` is `git restore --staged --
<paths>`, and `unstage --all` is `git reset --mixed --quiet HEAD --`. When
`HEAD` does not resolve, because the branch has no commits yet, both fall back:
`git rm --cached --ignore-unmatch -- <paths>` for named paths, and
`git rm --recursive --cached .` for `--all`, whose failure is tolerated only
when the resulting status is clean. `discard` routes each status row on its
own: a staged row goes to
`git restore --staged --worktree --source=HEAD -- <paths>`, every other tracked
row goes to `git restore --worktree -- <paths>`, and a file that is both staged
and modified is two rows and so reaches both commands. Untracked entries are
deleted with a filesystem removal rather than with Git. `commit` is
`git commit [--amend] --message <message>`. `resolve --ours` and
`resolve --theirs` are `git checkout --ours|--theirs -- <path>` followed by
`git add -- <path>`, and `resolve --stage` is `git add -- <path>` on its own.

Every one of those runs as `git -C <repository root> -c core.quotepath=false
...`, with `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0` and `GIT_TERMINAL_PROMPT=0` in the
environment, no shell in between, and stdin closed. Two consequences follow. The
first is that a path or a message containing a space, a quote or a semicolon is
one argument and nothing else. The second is that paths are resolved from the
repository root, not from the current directory: `-C` chooses the repository, it
does not change what a relative path means.

Two refusals happen in this process, before Git is asked anything. A commit
message that is empty or only whitespace is refused with
`Commit message cannot be empty`, so an unset shell variable cannot become a
commit. And `discard` without `--yes` prints what it would discard, changes
nothing, and exits 0, because a missing confirmation is a decision rather than
an error. Nothing else here has a confirmation gate: `stage`, `unstage`,
`commit` and `resolve` act the moment they are run.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet stage` | Adds paths, or every change, to the index. |
| `quinjet unstage` | Takes paths, or everything, back out of the index. |
| `quinjet discard` | Throws away changes to paths, permanently, behind `--yes`. |
| `quinjet commit` | Records what is staged, or replaces the previous commit. |
| `quinjet resolve` | Takes one side of a merge conflict, or marks a path resolved. |
| `quinjet cherry-pick` | Applies an existing commit, behind a preview and `--yes`. |
| `quinjet revert` | Records the inverse of an existing commit, behind a preview and `--yes`. |

## Commands

- [`quinjet stage`](./stage.md)
- [`quinjet unstage`](./unstage.md)
- [`quinjet discard`](./discard.md)
- [`quinjet commit`](./commit.md)
- [`quinjet resolve`](./resolve.md)
- [`quinjet cherry-pick`](./cherry-pick.md)
- [`quinjet revert`](./revert.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | The operation finished and its sentence printed. Also `discard` without `--yes`, which reports and changes nothing, and `discard` whose paths match no change, which prints `No changes match`. Also `--help` on any of the seven. |
| 1 | Git refused: nothing staged for `commit`, a pathspec that matches no file for `stage` or `unstage`, an ignored path for `stage`, a path with no such side for `resolve`, or `--amend` with no commit to amend. A blank commit message is also refused before Git runs. |
| 2 | clap rejected the command line: missing paths and `--all`, `commit` without `--message`, `resolve` without a side, a revision verb without a revision, `--all` together with paths, two conflict sides, a flag given twice, or an unknown flag. |
| 3 | `cherry-pick` or `revert` could not resolve the named revision. |

Code **4** never comes from this group because nothing here reads content that
can exist but be unavailable.

## Notes and gotchas

- There is no `quinjet changes` command. The seven are top-level verbs, so there
  is no group `--help` to run; `quinjet --help` lists them among the rest, and
  `quinjet stage --help` and friends work as usual.
- Paths are interpreted from the repository root, always. Running
  `quinjet stage README.md` from inside `src/` stages the README at the top of
  the repository, not `src/README.md`. Give root-relative paths, or absolute
  ones for `stage` and `unstage`, which hand them to Git.
- `discard` is the exception to that: it does not hand paths to Git at all. It
  reads `quinjet status`, keeps the changes whose path starts with one of your
  arguments, and acts on those. The match is by path component, so
  `quinjet discard src` covers `src/main.rs` and `quinjet discard sr` covers
  nothing. An absolute path never matches, because status paths are relative.
- The count in `1 change staged` and `3 changes unstaged` counts the arguments
  you gave, not the files Git touched. `quinjet stage src` says
  `1 change staged` whether `src` holds one file or four hundred.
- `discard` counts differently again: it counts status rows. A file that is both
  staged and modified is two rows, so `quinjet discard src/main.rs` reports two
  changes and names `src/main.rs` twice in its dry run. That is not a bug in the
  message; it is two pieces of work.
- `discard` skips conflicted paths entirely. During a conflicted merge,
  `quinjet discard --all` leaves every conflict exactly where it was, and if the
  conflicts are the only changes it prints `No changes match` and exits 0.
- `discard` deletes untracked files with `std::fs`, not with `git clean`. The
  content never enters Git's object store, so there is nothing to recover from:
  no reflog entry, no dangling blob, no stash. This is the one operation in
  Quinjet that is genuinely unrecoverable, which is why it is the one that
  insists on `--yes`.
- Content that was staged at some point does exist as a loose blob, so
  `git fsck --lost-found` can sometimes bring back a discarded staged version
  until `git gc` runs. Content that was only ever in the working tree cannot be
  recovered at all.
- `discard` is not atomic. Untracked removals happen first, one at a time, and
  the two `git restore` calls happen after. A failure partway through leaves
  everything already removed removed.
- `git restore --worktree` restores from the index, not from `HEAD`. Discarding
  the unstaged half of a file that also has staged changes gives you the staged
  version back, not the committed one.
- Discarding a staged addition removes the file from disk. `git restore --staged
  --worktree --source=HEAD` on a path that `HEAD` has never heard of drops the
  index entry and deletes the working-tree file, and Quinjet does not warn
  separately about that case.
- On an unborn branch, `discard` of anything staged fails with
  `Git command failed: fatal: could not resolve HEAD`, because
  `--source=HEAD` has nothing to read. Untracked removals in the same run have
  already happened by then.
- `unstage --all` is `git reset`, and `git reset` ends a merge. Run it during a
  conflicted merge and `MERGE_HEAD` is deleted, the conflict stages collapse to
  one index entry, and the conflict markers stay in your files as an ordinary
  modification. Git will no longer offer to help you finish.
- `stage --all` during a conflicted merge marks every conflict resolved,
  markers and all, because `git add -A` does not read the file. Stage
  conflicted paths one at a time, or better, use
  [`quinjet resolve`](./resolve.md).
- `stage` never passes `-f`, so an ignored path cannot be staged. Git's own
  refusal, `The following paths are ignored by one of your .gitignore files`,
  comes through as the error text.
- On an unborn branch, `unstage <path>` uses `git rm --cached
  --ignore-unmatch`, which succeeds even when the path was never staged. On a
  normal branch the same command is `git restore --staged`, which fails on an
  unmatched pathspec. The same invocation therefore has a different failure mode
  before and after the first commit.
- On an unborn branch, `unstage --all` runs `git rm --recursive --cached .`.
  Current Git accepts `-r` but not `--recursive`, so that call exits 129 with
  `error: unknown option 'recursive'`. Quinjet tolerates the failure only when
  the resulting status is clean, so on an unborn branch with anything staged the
  verb reports `Unable to unstage changes: ...` and exits 1. Unstage the paths
  by name until the first commit exists.
- `commit` never opens an editor and never reads stdin: `--message` is required
  by clap, and Git is spawned with stdin closed. Anything that wants to ask you
  a question, a `pre-commit` hook that prompts, a `pinentry-tty` for a signing
  key, sees end of file and fails.
- Hooks do run, because `--no-verify` is never passed. Their output is captured
  and thrown away when they succeed, and becomes part of the error text when
  they fail. A hook that prints progress prints it nowhere.
- When Git fails with nothing on stderr, Quinjet reports its stdout instead.
  That is why a `commit` with nothing staged prints Git's whole `On branch ...
  no changes added to commit` blurb after `error: Git command failed:`.
- `resolve --ours` and `resolve --theirs` do not check that the path is
  conflicted. On a path that merged cleanly, `git checkout --ours -- <path>`
  behaves like `git checkout -- <path>`: it overwrites your working-tree changes
  from the index and exits 0. Quinjet then stages the result and reports
  `Accepted --ours for <path>`. There is no `--yes` gate on that.
- `resolve` finishes one path. It does not continue the merge, and Quinjet has
  no `merge`, `rebase` or `--continue` verb at all. Finish with
  `quinjet commit --message ...` for a merge, and with `git rebase --continue`
  or `git cherry-pick --continue` for the others.
- Nothing in this group is serialized across processes. Inside the terminal
  interface every operation is queued onto one background worker lane, so two
  can never overlap; two `quinjet` processes have no such queue. Run
  `quinjet stage` and `quinjet commit` at the same moment and they race on
  `.git/index.lock`, and one of them reports
  `fatal: Unable to create '<root>/.git/index.lock': File exists`.
- `GIT_OPTIONAL_LOCKS=0` does not change that. It stops reads from taking the
  index lock opportunistically; a write still takes it, because a write must.
- A running terminal interface notices what the command line did. It watches the
  worktree recursively and refreshes when anything that is not `.git/objects`,
  `index.lock` or a Watchman cookie changes, so a `quinjet commit` in another
  window repaints the Changes tab within a redraw.
- On a detached `HEAD` all seven work, and `commit` makes a commit that only
  `HEAD` points at. Note the id with `quinjet log -n 1` before switching away,
  or it is reachable only through the reflog.
- None of these verbs takes `--watch`, `--expanded` or `--refresh`. They finish.
- Under `--json` each prints one object with a single `message` key, holding the
  same sentence the human form prints. On a non-zero exit stdout is empty.
- `quinjet cherry-pick` and `quinjet revert` act on commits rather than paths.
  Both resolve the revision, preview by default, and require `--yes` to mutate.
- Removal behavior follows the platform. `discard` unlinks a symlink rather
  than following it, because it reads the entry with `symlink_metadata`. On
  Windows a file another process holds open, or one marked read-only, cannot be
  removed, and the discard fails there rather than at the end.

## Where to go next

- [`quinjet status`, `diff`, `log`, `show`](../repository/README.md) to see what
  there is to stage, and what a commit would record
- [`quinjet stash`](../stash/README.md) for putting changes aside instead of
  throwing them away
- [`quinjet fetch`, `pull`, `push`, `sync`](../remotes/README.md) for getting a
  commit somewhere else
- [Conventions and contracts](../conventions.md) for the `--json` shape, the
  exit-code table and the environment every Git call runs in
- [All `quinjet` commands](../README.md)
