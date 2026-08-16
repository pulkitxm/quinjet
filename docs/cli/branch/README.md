# `quinjet branch`

`quinjet branch` is the local ref group: six verbs over the branches in one
checkout. Two of them read (`list` and `compare`) and four of them write
(`switch`, `create`, `rename`, `delete`). Nothing here touches GitHub, nothing
here needs `gh`, and nothing here is cached, so every invocation is a fresh read
of `.git` and costs no network at all. Reach for it when you want the branch
picker's answer without opening the terminal interface, or when a script needs
to know what exists before it decides what to do.

Listing is two different `git for-each-ref` reads, chosen by `--all`. The
default reads `refs/heads` alone with
`--format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:relative)%1f%(objectname:short)%1e`,
sorted by `--sort=-committerdate`, so the branch you last committed to is at the
top and every row can carry the upstream it tracks. `--all` reads `refs/heads`
and `refs/remotes` together with a different format that swaps `%(upstream:short)`
for `%(refname)` and `%(symref)`, so each row carries the full ref that
`compare` accepts, and any symbolic alias, `origin/HEAD` above all, is dropped
rather than listed as a branch that does not exist. The two reads produce
different columns and different JSON keys; they are not one command with a
filter.

The writing verbs are thin. `switch` is `git switch -- <name>`, `create` is
`git switch --create <name> [<start>]`, `rename` is
`git branch --move -- <old> <new>`, and `delete` is
`git branch --delete -- <name>`. Quinjet adds exactly three things on top:
`create` and `rename` refuse an empty name and run `git check-ref-format
--branch <name>` before anything is written, `rename` refuses a rename that
would change nothing, and `delete` refuses to run at all without `--yes`. There
is no force flag anywhere in this group, so Git's own refusals, an unmerged
branch, a name that already exists, a branch another worktree has checked out,
are what you see, verbatim, on stderr.

`compare` is the verb the others exist to make unnecessary. It resolves a name
against the `--all` listing, then diffs that branch's ref against `HEAD` and
prints the patch. It never checks anything out, never moves HEAD, never writes
a ref, and never touches your index or working tree, which is the whole point:
reading what a branch would bring is not a reason to disturb what you are
working on. The reads are `git diff --numstat`, `git diff --name-status` and
then one `git diff --patch` per changed file, all with `<reference>` as the
left side and `HEAD` as the right.

Everything in this group runs `git` with `-C <worktree root>` and
`-c core.quotepath=false`, under `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0` and
`GIT_TERMINAL_PROMPT=0`, with the arguments passed as an array rather than
through a shell. A branch name containing a space, a quote or a semicolon is
one argument and nothing else.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet branch list` | Lists local branches, newest commit first, with their upstreams. `--all` adds remote-tracking branches. |
| `quinjet branch switch` | Moves HEAD to an existing branch, and creates a tracking branch when the name only exists on a remote. |
| `quinjet branch create` | Validates a name, then creates a branch and switches to it, optionally from a start point. |
| `quinjet branch rename` | Renames a local branch and keeps its tracking configuration. |
| `quinjet branch delete` | Deletes a local branch, but only with `--yes`, and only if Git agrees it is merged. |
| `quinjet branch compare` | Prints the patch between another branch and HEAD without checking anything out. |

## Commands

- [`quinjet branch list`](./list.md)
- [`quinjet branch switch`](./switch.md)
- [`quinjet branch create`](./create.md)
- [`quinjet branch rename`](./rename.md)
- [`quinjet branch delete`](./delete.md)
- [`quinjet branch compare`](./compare.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | A listing printed, a switch, create, rename or delete succeeded, or `compare` printed a patch. Also `compare` when the two sides are identical, which prints `No file changes to display`. Also `delete` without `--yes`, which reports what it would delete and changes nothing. Also `--help` on the group or on any verb. |
| 1 | Git refused: `switch` was given a name that is not a branch, `create` was given an invalid name or a start point that does not resolve, `rename` was given a missing source or an existing target, `delete --yes` was given an unmerged branch or the current branch. Also `rename` with the same old and new name, and any verb run outside a Git repository. |
| 2 | The command line was wrong in clap's terms: `quinjet branch` with no verb, an unknown verb, an unknown flag, or a missing `<NAME>`, `<OLD>`, `<NEW>` or `<REFERENCE>`. |
| 3 | Only `compare`, and only when its `<REFERENCE>` matches neither the short name nor the full ref of any branch the `--all` listing knows. |

No verb in this group can exit 4. Nothing here reads GitHub, so there is nothing
that exists but cannot be read.

## Notes and gotchas

- The default listing reads `refs/heads` only. `--all` reads `refs/heads` and
  `refs/remotes`, and drops any ref whose `%(symref)` is non-empty, so
  `origin/HEAD` never appears as if it were a branch of its own. Rows that Git
  emits with fewer fields than the format asked for, and refs outside
  `refs/heads/` and `refs/remotes/`, are skipped in silence.
- Ordering differs between the two reads. Plain `list` is exactly Git's
  `--sort=-committerdate`, newest tip commit first, with the current branch
  wherever its date puts it. `list --all` re-sorts that by
  `(not current, remote)` first, so the current branch is hoisted to the top,
  then every local branch by date, then every remote-tracking branch by date.
- The two reads produce different JSON. Plain `list` gives `upstream`; `--all`
  gives `reference` and `remote` and no `upstream` at all. A script that wants
  both the upstream and the full ref has to run both.
- `relativeDate` is `%(committerdate:relative)` of the commit the ref points at,
  not when the branch was created, renamed or last checked out. Resetting a
  branch to an older commit makes it look old. `LC_ALL=C` keeps the wording
  stable regardless of your locale.
- Text columns are minimum widths, not truncations: 28 characters for the name
  in `list`, 40 in `list --all`. A longer branch name pushes the rest of its
  row to the right rather than being cut, so the columns are not guaranteed to
  line up. Parse `--json`, not the table.
- Nothing in this group contacts a network. Remote-tracking rows are exactly as
  fresh as your last fetch, so run [`quinjet fetch`](../remotes/README.md)
  first if you are about to trust them.
- On an unborn branch, before the first commit, `for-each-ref` finds nothing:
  `list` prints nothing and exits 0, `list --json` prints `[]`, and
  `compare main` exits 3 even though `git status` names `main`, because the ref
  does not exist yet.
- On a detached HEAD no row is marked with `*`, and `list --all` therefore
  hoists nothing to the top. `compare` still works, and titles the current side
  with the abbreviated commit id that `quinjet status` shows.
- `%(HEAD)` marks the branch checked out in *this* worktree. A branch checked
  out in another worktree of the same repository looks like any other row, but
  `switch` and `delete` both fail on it with Git's
  `cannot delete branch 'x' used by worktree at '<path>'` or the equivalent
  checkout refusal.
- `delete` is `git branch --delete`, never `--delete --force`. Git's refusal
  ends with advice to run `git branch -D`, which Quinjet has no verb for on
  purpose: force-deleting an unmerged branch is a thing to do with Git itself.
- `--yes` is the only confirmation gate in the group. `switch`, `create` and
  `rename` act the moment they are run. Forgetting `--yes` on `delete` is not
  an error: it prints one sentence and exits 0, so a script that omits it sees
  success and no deletion.
- `create` and `rename` validate the new name with `git check-ref-format
  --branch`, which resolves Git's own shorthands. `@{-1}` and `-` pass
  validation because Git expands them to a real branch name, and
  `git switch --create` then interprets them its own way, so a name that looks
  like a shorthand is not a literal name.
- A name that begins with `-` is a flag to clap before it is ever a name to
  Git. Put `--` first: `quinjet branch switch -- -weird`.
- Branch names are compared by Git exactly, but loose refs are files. On a
  case-insensitive filesystem, macOS and Windows by default, `feature` and
  `Feature` collide, and a rename between two case-only variants may fail.
  Quinjet normalizes nothing.
- `compare` runs one `git diff --patch` process per changed file, serially,
  after the two index reads. Comparing a branch that differs in 200 files means
  200 Git processes, so the cost scales with the size of the difference, not
  the size of the repository. The terminal interface only loads the file you
  are looking at; the command line always loads all of them, because it prints
  a whole document.
- Because those reads are separate processes, `compare` is not atomic. A commit,
  amend or rebase in another window between the branch listing, the status read
  and the per-file patches produces a document that mixes two states. Nothing
  detects it. For a stable answer, compare when nothing else is writing.
- Reads run with `GIT_OPTIONAL_LOCKS=0`, so `list` and `compare` never create
  `index.lock` and never fight a Git command running elsewhere. The writing
  verbs do take Git's normal locks, and fail the way Git fails when another
  process holds them.
- `compare` obeys the same caps as every other patch in Quinjet: 8 MiB per file
  patch, 8 MiB and 16,384 paths for the file index, and syntax highlighting is
  skipped above 512 KiB per patch or 32 KiB per line. A read that crosses a cap
  ends with `[output reached Quinjet's size cap and was truncated]` and sets
  `truncated` in the JSON. See [the size caps](../conventions.md#size-caps).
- What this group deliberately does not do: no remote branch creation or
  deletion, no force delete, no `--force` create or rename, no `--track` or
  `--no-track`, no `--detach`, no filtering by `--merged`, `--contains` or a
  glob, no custom `--sort`, no ahead and behind counts, and no `--watch`. Ahead
  and behind for the current branch live in
  [`quinjet status`](../repository/README.md).
- In the terminal interface, `b` opens the local branch picker, which is this
  group's default listing: Enter switches, Ctrl+N creates, Ctrl+R or F2
  renames, and Delete asks `Delete local branch ...? Git will refuse if it is
  not merged.` before running the same operation. The interface refuses to
  delete the current branch itself; the command line lets Git refuse instead.
  In the History view `b` picks whose history to read, and `n` creates a branch
  at the selected commit. In the Changes view `d` opens the compare picker,
  which is `quinjet branch compare` with the current branch filtered out.
- `--help` is generated for the group and for every verb, prints on stdout and
  exits 0. `quinjet branch help list` is the same page. `quinjet branch` with no
  verb prints that help on **stderr** and exits 2, because this group has no
  default subcommand.

## Where to go next

- [`quinjet branch compare`](./compare.md), the one verb here that reads a
  branch without moving anything
- [`quinjet status`, `diff`, `log`, `show`](../repository/README.md) for the
  branch you are on, its upstream, and how far ahead or behind it is
- [`quinjet stage`, `commit` and the rest](../changes/README.md) for the work a
  switch is going to carry, or refuse to carry, with it
- [Conventions and contracts](../conventions.md) for `--json`, the exit-code
  table and the size caps this page leans on
- [All `quinjet` commands](../README.md)
