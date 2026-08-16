# `quinjet stash`

`quinjet stash` is the whole of Git's stash reflog behind seven verbs: read it
with `list`, add to it with `push`, take work back out with `apply` or `pop`,
throw an entry away with `drop`, empty it with `clear`, and read one entry as a
patch with `show`. It is the group to reach for when a branch switch, a pull or
a review needs the working tree clean for a moment, and for the far more common
case of finding out what is already in there before deciding whether it is safe
to delete.

Nothing in this group touches the network, GitHub or the cache. Every verb is
one or a few `git` invocations against the worktree root that `-C` resolves to,
run with `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0` and
`-c core.quotepath=false`, with arguments handed to `git` as an array rather
than through a shell. A message or a reference containing a space, a quote or a
semicolon is one argument and nothing else.

Reading the list is a single call:
`git stash list --format=%gd%x1f%gs%x1f%cr%x1f%h%x1e`. Records are separated by
`0x1e` and fields by `0x1f`, so no delimiter can appear inside a message. The
four fields are the reflog selector `%gd`, the reflog subject `%gs`, the
committer date relative `%cr` and the abbreviated hash `%h`. A record with fewer
than four fields is skipped, and a record whose selector is not literally
`stash@{N}` with an all-digit `N` is dropped rather than passed on. That filter
is what makes the rest of the group safe: every reference Quinjet later hands to
Git is checked against the same `stash@{N}` shape by `validate_stash_reference`,
and anything else fails with `refusing to use an invalid stash reference`
before `git` is executed.

The reflog subject is where the branch and the message come from. Git writes
`WIP on <branch>: <commit> <subject>` for a stash with no message and
`On <branch>: <message>` for one with a message, so Quinjet strips whichever of
the two prefixes matches and splits the rest on its first colon and space. What is left of
that split is the branch, what is right of it is the message. A subject with
neither prefix leaves the branch empty, and a stash taken with a detached HEAD
reports the branch Git wrote, which is `(no branch)`.

Writing is deliberately thin. `push` builds `git stash push` and appends
`--include-untracked` or `--staged` when asked and `--message <text>` only when
the trimmed message is not empty. `apply` and `pop` both run with `--index`, so
what was staged when the stash was taken is staged again. `drop` and `clear`
refuse to act without `--yes`: without it they print the sentence describing
what they would have done, run no destructive Git command at all, and exit 0.

`show` is the one verb doing real work of its own. A stash commit's first parent
is the HEAD it was taken from, so the tracked half of the patch is
`git diff <ref>^1 <ref>`. Untracked files, when `--include-untracked` was used,
live in a third-parent root commit instead, and older Git cannot path-filter
`git stash show`, so the untracked half is read separately with
`git show <ref>^3` and appended to the same document. Read
[`quinjet stash show`](./show.md) before relying on it: the probe for that third
parent misbehaves on current Git, and the verb only succeeds today for stashes
that actually have an untracked half.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet stash list` | Prints every stash, newest first, with its reference, hash, age, branch and message. |
| `quinjet stash push` | Stashes the current changes, optionally with a message, including untracked files, or the index alone. |
| `quinjet stash apply` | Applies one stash and keeps it in the list, restoring the index too. |
| `quinjet stash pop` | Applies one stash and drops it. With no reference, pops the newest. |
| `quinjet stash drop` | Deletes one stash. Needs `--yes`. |
| `quinjet stash clear` | Deletes every stash. Needs `--yes`. |
| `quinjet stash show` | Prints one stash as a patch, tracked changes and untracked additions together. |

## Commands

- [`quinjet stash list`](./list.md)
- [`quinjet stash push`](./push.md)
- [`quinjet stash apply`](./apply.md)
- [`quinjet stash pop`](./pop.md)
- [`quinjet stash drop`](./drop.md)
- [`quinjet stash clear`](./clear.md)
- [`quinjet stash show`](./show.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | `list` printed, including the empty list. `show` printed a patch. `push`, `apply`, `pop`, `drop --yes` and `clear --yes` succeeded. `drop` and `clear` without `--yes` reported what they would do and changed nothing. `--help` on the group or any verb. |
| 1 | Git refused: `pop` with an empty stash list (`No stash entries found.`), `drop stash@{9}` where there is no ninth entry (`stash@{9} is not a valid reference`), `push` on a branch with no commits (`You do not have the initial commit yet`), an `apply` or `pop` whose index could not be restored, `show` failing on `<ref>^3`. Also a reference of valid shape rejected by `validate_stash_reference`, and `-C` pointing somewhere that is not a repository. |
| 2 | clap rejected the command line: `quinjet stash` with no verb, `apply`, `drop` or `show` with no `<REFERENCE>`, an unknown flag, or `push --staged --include-untracked`, which reports `the argument '--staged' cannot be used with '--include-untracked'`. |
| 3 | `show` was given a reference that is not in `stash list`, including a well-formed one such as `stash@{9}` in a repository with two stashes. The hint always names `quinjet stash list` as the way to see what exists. |

This group never exits 4. Nothing here can name a thing that exists and cannot
be read.

## Notes and gotchas

- Stash references are positions, not names. `stash@{0}` is always the newest
  entry, and every `push`, `pop` and `drop` renumbers everything after it. A
  reference read in one command and used in the next is a race if anything else
  is touching the repository: another terminal, an editor's Git integration, or
  the Quinjet interface open in another window. Read
  [`stash list`](./list.md) and act in one go, and prefer `pop` with no
  reference when you mean "the last thing I stashed".
- The `shortId` in `stash list` is the stash commit itself and is stable across
  renumbering, but no verb in this group accepts it. Only `stash@{N}` is
  accepted, by `apply`, `pop`, `drop` and `show` alike.
- `list` has no limit, no filter and no `--all`. The entire stash reflog is read
  and printed in Git's own order, newest first, with no sorting of Quinjet's
  own. Unlike a diff read, it is not bounded by a byte cap.
- An empty stash list is not an error. `quinjet stash list` prints nothing at
  all and exits 0; `--json` prints `[]`.
- The branch column comes from the reflog subject rather than from any ref, so
  it is whatever the branch was called when the stash was taken. Renaming or
  deleting that branch afterwards does not change it, and applying a stash onto
  a different branch is allowed: Git only cares about the trees.
- A stash written by something other than `git stash push`, for example
  `git stash store -m "custom label"`, has neither the `WIP on` nor the `On` prefix, so its
  branch parses as the empty string and the row reads
  `stash@{0}    94eaeda    0 seconds ago  on : custom label`. The `branch` key
  in `--json` is `""` in that case, never `null`.
- `push` with a message that is empty or only whitespace omits `--message`
  entirely rather than passing an empty one, so Git writes its own
  `WIP on <branch>: <commit> <subject>` subject. Messages are trimmed before
  they are passed, so leading and trailing spaces never reach the reflog.
- `push` on a clean working tree is not a failure. Git prints
  `No local changes to save` and exits 0, so Quinjet prints `Changes stashed`
  and exits 0 even though nothing was stashed. Check with `stash list`
  afterwards if it matters.
- `push --staged` needs Git 2.35 or newer. On anything older Git rejects the
  option and the verb exits 1 with Git's own message. `--staged` and
  `--include-untracked` are mutually exclusive, and clap rejects the pair before
  any Git runs.
- `push` implements three variants and nothing else. There are no pathspecs, no
  `--keep-index`, no `--all` for ignored files, and no `--patch`. There is also
  no `quinjet stash branch`, no `create` and no `store`.
- `apply` and `pop` always pass `--index`. There is no way to ask for the
  working-tree half alone. When Git cannot reinstate the index it fails and the
  verb exits 1, and Git may already have written some of the changes to the
  working tree by then, so a failed `pop` can leave both the stash and a dirty
  tree behind. Re-read [`quinjet status`](../repository/README.md) rather than
  assuming.
- `apply` requires a reference. Only `pop` makes it optional, and only `pop`
  with no reference reports `Popped latest stash` instead of naming one.
- `drop` and `clear` follow the same `--yes` rule as `quinjet discard` and
  `quinjet branch delete`, described in
  [conventions and contracts](../conventions.md). Without `--yes` they exit 0
  having changed nothing, so a missing confirmation is never an error to
  handle. `drop` without `--yes` does not check that the reference exists
  either: it prints its sentence for any string at all.
- `clear` without `--yes` does read the list, to count it, and its sentence is
  never pluralized: one stash produces `Would drop 1 stashes. Pass --yes to
  drop them.` `clear --yes` on an empty list still succeeds and still says
  `Dropped all stashes`.
- A dropped stash is not gone from the object database immediately, but Quinjet
  offers nothing to get it back. Recovery means `git fsck --unreachable` and
  `git stash store`, outside Quinjet.
- `show` looks its reference up in `stash list` first, which is why an unknown
  reference exits 3 with a hint rather than reaching Git. The shape check that
  produces exit 1 elsewhere is therefore unreachable from `show`.
- `show` builds the file list with
  `git stash show --name-status -z --include-untracked <ref> --`, and asks for
  the same listing again as `--numstat` to fill in the `+n -n` counts. The
  numstat read is best-effort: if it fails the counts render as `+? -?` rather
  than failing the verb.
- `show` then reads one patch per file, so a stash touching twenty files costs
  roughly twenty `git diff` calls, twenty `git rev-parse` probes and up to
  twenty `git show` calls, plus the two index reads. It is not one `git stash
  show -p`.
- Because the tracked half is `<ref>^1 <ref>`, the patch is the stash's whole
  change against the commit it was taken from: staged and unstaged work appear
  together with no way to tell them apart. A `--staged` stash is the exception,
  since its tree only ever held the index.
- `show` currently fails for a stash that has no untracked half. The existence
  probe is `git rev-parse --verify "<ref>^3^{commit}"`, and on Git 2.43 the
  nested braces make Git read the whole thing as a reflog date rather than a
  parent, so the probe succeeds for every stash. The following
  `git show <ref>^3` then fails and the verb exits 1 with
  `error: Git command failed: fatal: bad revision 'stash@{0}^3'`. Stashes taken
  with `--include-untracked` are unaffected. See [`show`](./show.md).
- The size caps in [conventions and contracts](../conventions.md) apply to
  `show`: 8 MiB per file patch and 8 MiB or 16,384 paths for the file listing.
  The untracked half is read with whatever is left of the 8 MiB after the
  tracked half, and is skipped entirely when the tracked half already hit the
  cap. Crossing a cap appends
  `[output reached Quinjet's size cap and was truncated]` rather than pretending
  the patch is complete.
- `--expanded` on `show` means `--unified=1000000` instead of `--unified=3`, on
  both halves. It is the `t` key of the terminal interface, and it does not add
  files: a file with no change in this stash is still absent.
- Nothing in this group is cached, watched or refreshed. There is no `--watch`
  and no `--refresh` on any stash verb, and `--json` is the global flag
  described in [conventions](../conventions.md), so
  `quinjet --json stash list` and `quinjet stash list --json` are the same run.
- On an unborn branch, `list` works and prints nothing, and `push` fails with
  `You do not have the initial commit yet`. On a detached HEAD everything works
  and the stash records `(no branch)`.
- A bare repository has no worktree, so `push`, `apply` and `pop` fail in Git's
  own terms. `list` will happily print an empty list.
- The mutating verbs run through the same command layer the terminal interface
  uses, so the sentence on stdout is the same string the interface puts in a
  toast. In the Changes view, `S` opens the Stashes modal; inside it Ctrl-N,
  Ctrl-U and Ctrl-S are `push`, `push --include-untracked` and `push --staged`,
  Alt-A is `apply`, Alt-P is `pop`, Delete is `drop` behind a confirmation,
  Ctrl-Delete is `clear` behind a confirmation, and Enter previews the selected
  stash with exactly what `stash show` prints.
- Platform differences come from Git, not from Quinjet. The one thing worth
  knowing is that a stash records file modes, so a tree stashed on a filesystem
  that keeps the executable bit and applied on one that does not will differ in
  the mode Git reports.

## Where to go next

- [`quinjet status`, `diff`, `log`, `show`](../repository/README.md) for reading
  the working tree a stash came from or landed in
- [`quinjet stage`, `unstage`, `discard`, `commit`](../changes/README.md) for
  the other verbs that move the index, including the `--yes` guard `drop` and
  `clear` share with `discard`
- [`quinjet branch`](../branch/README.md) for the switch a stash usually exists
  to make possible
- [Conventions and contracts](../conventions.md) for `--json`, the exit-code
  table and the size caps this page refers to
- [All `quinjet` commands](../README.md)
