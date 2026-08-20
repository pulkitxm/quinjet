# Refs, the Index, and Worktrees

Git's object store is immutable and content-addressed, which is what makes it cacheable forever;
everything mutable in a repository lives in a thin layer on top of it: references that name commits,
the index that stages the next commit, and worktrees that give one object store several checkouts.
This page explains that mutable layer byte by byte, then shows exactly how Quinjet reads it without
ever blocking it, locking it, or mutating it: `GIT_OPTIONAL_LOCKS=0` on every read, a filesystem
watcher that covers the Git common directory so linked worktrees refresh live, `refs/pull/N/head`
as a fetchable ref for pull-request diffs, and a hard guarantee that the opened repository receives
no checkout, branch, ref, index, or worktree mutation from the pull-request machinery.

## Contents

- [The mutable edge of an immutable store](#the-mutable-edge-of-an-immutable-store)
- [References: naming commits](#references-naming-commits)
- [packed-refs: the sorted flat file](#packed-refs-the-sorted-flat-file)
- [HEAD and symbolic refs](#head-and-symbolic-refs)
- [Reflogs](#reflogs)
- [How Quinjet reads refs](#how-quinjet-reads-refs)
- [The index: the third tree](#the-index-the-third-tree)
- [The index file format](#the-index-file-format)
- [The stat cache and racy Git](#the-stat-cache-and-racy-git)
- [index.lock and optional locks](#indexlock-and-optional-locks)
- [How Quinjet reads status without taking a lock](#how-quinjet-reads-status-without-taking-a-lock)
- [Linked worktrees and the common directory](#linked-worktrees-and-the-common-directory)
- [How Quinjet watches a repository](#how-quinjet-watches-a-repository)
- [Pull requests as fetchable refs](#pull-requests-as-fetchable-refs)
- [The no-mutation guarantee](#the-no-mutation-guarantee)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Design alternatives that lost](#design-alternatives-that-lost)
- [Related pages](#related-pages)

## The mutable edge of an immutable store

A Git repository is two very different data stores stapled together.

The first is the object database: blobs, trees, commits, and tags, each named by the hash of its
own content. An object can never change, because changing it would change its name. Everything in
[the object model page](./object-model.md) and
[the packfile page](./packfiles-and-deltas.md) follows from that immutability, and so does most of
Quinjet's caching: an answer keyed by object ids can be cached forever.

The second store is small, mutable, and heavily trafficked. It is what this page covers:

- References map human names (`refs/heads/main`) to object ids. They move on every commit, fetch,
  and branch switch.
- `HEAD` is a special reference selecting the current branch or commit for each worktree.
- Reflogs record where each reference used to point.
- The index is a single binary file describing the next commit and caching stat data for the whole
  working tree, so that `git status` does not have to re-read every file.
- Worktree metadata lets several working directories share one object store and one set of refs.

Every performance-relevant Quinjet behavior in this layer follows one rule: read the mutable layer
often and cheaply, and never write it except when the user explicitly asks for a mutation. The rest
of this page is the machinery on both sides of that rule.

## References: naming commits

A reference (ref) is a name under the `refs/` namespace whose value is an object id, almost always
a commit id. The name is hierarchical, using `/` as a separator, and the hierarchy is meaningful:

| Namespace | Contents | Moved by |
| --- | --- | --- |
| `refs/heads/<name>` | Local branches | `commit`, `switch`, `reset`, `fetch` into the current branch |
| `refs/remotes/<remote>/<name>` | Remote-tracking branches | `fetch`, `push` (with tracking updates) |
| `refs/tags/<name>` | Tags, lightweight or annotated | `tag`, `fetch --tags` |
| `refs/stash` | The stash stack's top entry | `stash push`, `stash pop`, `stash drop` |
| `refs/notes/<name>` | Notes trees | `notes` |
| `refs/pull/<n>/head` | GitHub's synthetic PR head refs (server side) | GitHub, on every PR push |
| `refs/quinjet/*` | Quinjet's private fetch targets in its disposable workspace | Quinjet fetches |

The last two rows preview the second half of this page. GitHub advertises `refs/pull/<n>/head` for
every pull request in a repository, and Quinjet fetches those refs into a private `refs/quinjet/*`
namespace inside a disposable bare repository so that nothing it does can ever collide with a real
branch or tag in any repository the user cares about.

### Loose refs: one file per name

The simplest storage for a ref is a loose ref: a file at `.git/refs/heads/main` whose entire
content is the object id in hexadecimal plus a newline.

```text
$ cat .git/refs/heads/main
1261472f3c1c07b0f47a1c0965a1b9bee7c56c07
```

The byte layout could not be simpler:

| Offset | Size | Content |
| --- | --- | --- |
| 0 | 40 (SHA-1) or 64 (SHA-256) | Lowercase hexadecimal object id |
| 40 or 64 | 1 | `\n` |

The directory hierarchy of the filename is the ref hierarchy: `refs/heads/feature/login` is the
file `.git/refs/heads/feature/login`. Two consequences fall out of using the filesystem as the
database:

**1. Directory/file conflicts are real.** `refs/heads/a` and `refs/heads/a/b` cannot coexist,
because `a` cannot be both a file and a directory. Git enforces this at creation time, which is one
of the rules behind `git check-ref-format`. Quinjet delegates exactly this class of validation to
Git rather than reimplementing it: `validate_branch_name` (src/git/mod.rs:1235-1241) trims the
name, rejects an empty result, and then runs `git check-ref-format --branch <name>` so the rules
can never drift from the Git binary actually creating the branch.

**2. Reading a loose ref is one `open` and one 41-byte read.** That is why enumerating a handful
of refs is essentially free, and why the cost model of ref reads is dominated by directory
traversal once a repository has thousands of them. The fix for that is `packed-refs`, covered in
the next section.

### Updating a ref safely

Refs are updated through a lock-file protocol, the same shape Git uses for the index:

1. Create `.git/refs/heads/main.lock` with `O_CREAT | O_EXCL`. Failure means someone else is
   updating the same ref right now; the update fails immediately rather than blocking.
1. Verify the old value if the caller supplied one (compare-and-swap semantics; this is what
   `git update-ref <ref> <new> <old>` checks).
1. Write the new object id into the lock file.
1. `rename` the lock file over the real ref file. Rename is atomic on POSIX filesystems, so any
   concurrent reader sees either the complete old value or the complete new value, never a torn
   write.

The plumbing for this is [`git update-ref`](https://git-scm.com/docs/git-update-ref), which also
supports transactional multi-ref updates through `--stdin` with `start`/`prepare`/`commit` verbs.
Porcelain commands (`commit`, `fetch`, `switch`) all go through the same machinery internally.

Two properties matter for a tool like Quinjet that reads refs from background threads:

- Readers never take a lock. Reading a ref is a plain file read, so a busy TUI polling refs can
  never make the user's own `git commit` fail.
- Writers fail fast instead of queuing. When Quinjet performs a user-requested mutation while the
  user runs `git` in another terminal, the two can collide exactly as two `git` processes would;
  one of them reports the lock and nothing is corrupted. Quinjet does not add its own locking on
  top, and its architecture document states this plainly (ARCHITECTURE.md, invariant 14):
  "Mutations are serialized by app state inside the terminal only; two concurrent processes can
  race on the index exactly as two `git` invocations would."

### Ref name resolution and why Quinjet refuses short names

A short name such as `main` is ambiguous. When resolving it, Git tries a fixed search path, in
order: the name itself as a top-level file (`HEAD`, `FETCH_HEAD`, `ORIG_HEAD`, `MERGE_HEAD`,
`CHERRY_PICK_HEAD` and friends), then `refs/<name>`, `refs/tags/<name>`, `refs/heads/<name>`,
`refs/remotes/<name>`, and finally `refs/remotes/<name>/HEAD`. A tag and a branch with the same
name resolve to the tag, with a warning. Scripts that guess wrong here produce diffs of the wrong
thing without any error.

Quinjet never relies on that search path. Every revision a user can type is first resolved by
`Repository::resolve_revision` (src/git/mod.rs:299-317), which:

- trims the input and rejects an empty string or anything starting with `-`, so option injection
  through a revision argument is impossible;
- passes `HEAD` through unchanged;
- otherwise asks `git rev-parse --symbolic-full-name --verify --quiet <revision>` and accepts the
  answer only if it starts with `refs/heads/`, `refs/remotes/`, or `refs/tags/`;
- failing that, falls back to `git rev-parse --verify --quiet <revision>^{commit}` and errors if
  the peel fails.

The result handed to every downstream command is therefore always a full, unambiguous ref or a
commit object id. `Repository::history` (src/git/mod.rs:330-357) re-validates even that: the
revision must be `HEAD`, start with `refs/heads/`, `refs/remotes/`, `refs/tags/`, or be a full
object id (`is_full_oid`, src/git/mod.rs:1588-1590: length 40 or 64, all ASCII hex). A test at
src/git/mod.rs:1723 pins that `history("--all", ...)` fails, so a crafted branch name can never
reach `git log` as a flag. The same discipline appears in `validate_history_reference`
(src/git/mod.rs:1388-1394), which requires a `refs/heads/` or `refs/remotes/` prefix before a
branch-comparison diff runs.

This is a correctness measure that doubles as a performance one: because the revision is already a
full ref or an OID, Git spends no time on disambiguation, and Quinjet's caches can use the resolved
value as a stable key.

## packed-refs: the sorted flat file

Loose refs scale badly. A repository with 30,000 tags would need 30,000 files, and enumerating them
means walking a directory tree with 30,000 `lstat` calls. Git's answer is
[`git pack-refs`](https://git-scm.com/docs/git-pack-refs), which folds refs into a single sorted
text file, `.git/packed-refs`:

```text
# pack-refs with: peeled fully-peeled sorted
2f3c1c07b0f47a1c0965a1b9bee7c56c071261472 refs/heads/main
56c071261472f3c1c07b0f47a1c0965a1b9bee7c5 refs/remotes/origin/main
7b0f47a1c0965a1b9bee7c56c071261472f3c1c07 refs/tags/v1.0.0
^965a1b9bee7c56c071261472f3c1c07b0f47a1c0
```

Format details worth knowing:

**1. The header line declares traits.** `peeled` and `fully-peeled` mean that annotated tag refs
are followed by a `^<oid>` line carrying the commit the tag points at, so a reader can resolve
`v1.0.0^{commit}` without opening the tag object at all. `sorted` (a trait Git has written since
version 2.15) means the entries are sorted by refname bytes, so a reader can memory-map the file
and binary-search it rather than scanning linearly.

**2. Lines are `<oid> SP <refname> LF`.** No quoting is needed because ref names cannot contain
spaces, control bytes, or several other characters (`check-ref-format` bans them at creation).

**3. The file is a snapshot, not the truth.** Packing does not stop refs from moving. When
`refs/heads/main` is updated after packing, the new value is written as a loose file, and the
lookup rule becomes: loose wins over packed. Deleting a packed ref is the expensive case, because
the whole `packed-refs` file must be rewritten (under `packed-refs.lock`) in addition to removing
any loose file, so that the deleted name does not resurrect from the packed snapshot.

The precedence rule creates a subtle read race Git itself has to defend against: between reading
the loose directory and reading `packed-refs`, a concurrent `pack-refs` may move a ref from one to
the other. Git re-checks stat data on the files it read and retries when they changed mid-read.
This matters to Quinjet only indirectly, and the indirection is the point: Quinjet never parses
`.git/refs` or `packed-refs` itself. Every ref enumeration is a `git for-each-ref` subprocess, so
whatever locking, retrying, and format evolution Git implements is inherited for free. The reftable
backend (selectable with `git init --ref-format=reftable` since Git 2.45) replaces the whole
loose-plus-packed scheme with a binary block-compressed structure, and Quinjet needs zero changes
to work with it, because the plumbing output is identical.

## HEAD and symbolic refs

`HEAD` is a file at the top of the Git directory, and it comes in exactly two shapes:

```text
$ cat .git/HEAD
ref: refs/heads/main
```

or, detached:

```text
$ cat .git/HEAD
1261472f3c1c07b0f47a1c0965a1b9bee7c56c07
```

The first shape is a symbolic ref: its value is the name of another ref, prefixed with `ref: `.
Committing while `HEAD` is symbolic moves the branch it names; `HEAD` itself does not change,
which is why branch switching is a one-file write. The second shape is a detached `HEAD`: the value
is a raw commit id, and committing moves `HEAD` itself. Symbolic refs are read and written with
[`git symbolic-ref`](https://git-scm.com/docs/git-symbolic-ref); in practice `HEAD` is the only one
most repositories ever have, though `refs/remotes/origin/HEAD` is a second common example, storing
the remote's default branch.

Three states of `HEAD` matter to a status-reading client:

| State | `HEAD` content | What `git status --porcelain=v2 --branch` reports |
| --- | --- | --- |
| On a branch | `ref: refs/heads/main` | `# branch.head main` plus `# branch.oid <oid>` |
| Detached | raw object id | `# branch.head (detached)` |
| Unborn branch | `ref: refs/heads/main`, but the ref does not exist | `# branch.oid (initial)` |

The unborn case is a fresh `git init`: `HEAD` names a branch that has no commit yet. Quinjet's
status parser handles all three: `parse_branch_header` (src/git/status.rs:172-199) skips the
literal `(initial)` oid, and for `(detached)` substitutes the first 8 characters of the oid as the
displayed head, or the literal `detached` when no oid was reported. The unborn state also changes
mutation strategy: `Repository::unstage` (src/git/mod.rs:1199-1206) picks
`git restore --staged -- <paths>` when `has_head` (a `git rev-parse --verify HEAD` probe,
src/git/mod.rs:1243-1246) succeeds, and falls back to `git rm --cached --ignore-unmatch -- <paths>`
in an unborn repository where there is no `HEAD` to restore from.

Crucially for worktrees, `HEAD` is per-worktree state. Every linked worktree has its own `HEAD`
file, which is what allows two worktrees to sit on different branches of the same repository. The
[worktree section](#linked-worktrees-and-the-common-directory) maps out exactly which files split
that way.

## Reflogs

Every time a ref moves, Git can append a line to that ref's reflog, a per-ref journal under
`.git/logs/`. The file for `refs/heads/main` is `.git/logs/refs/heads/main`, and `HEAD` has its
own at `.git/logs/HEAD`. One line per movement:

```text
<old-oid> SP <new-oid> SP <name> SP <email> SP <timestamp> SP <tz> TAB <message> LF
```

A concrete line, with the single hard tab between the timezone and the message rendered here as
`<TAB>`:

```text
9bee7c5 1261472 Pulkit <p@example.com> 1755648000 +0530<TAB>commit: feat: viewport-first loading
```

(Real reflog lines carry full 40-character ids; they are shortened above only for page width.)

The reflog is what gives `main@{1}` (where `main` pointed one move ago) and `main@{yesterday}`
(where it pointed at a time) their meaning. It is local-only, never transferred by fetch or push,
and expired by `git reflog expire` on gc's schedule.

### The stash is a ref plus its reflog

`git stash` is the one porcelain feature built entirely out of this machinery, and Quinjet leans on
its exact structure. A stash entry is a commit object with two or three parents:

| Parent | Content |
| --- | --- |
| `stash^1` | The commit `HEAD` pointed at when the stash was created |
| `stash^2` | A commit capturing the index (staged state) at stash time |
| `stash^3` | Only with `--include-untracked`: a commit holding the untracked files |

The ref `refs/stash` points at the newest entry, and the stack below it lives in the reflog of
that ref: `stash@{0}` is the ref itself, `stash@{1}` the previous reflog entry, and so on.
Dropping an entry rewrites the reflog. That is why stash selectors look like reflog selectors:
they are reflog selectors.

Quinjet reads the stack in one call (`Repository::stashes`, src/git/mod.rs:876-907):

```text
git stash list --format=%gd%x1f%gs%x1f%cI%x1f%h%x1e
```

`%gd` is the reflog selector (`stash@{0}`), `%gs` the reflog subject, and the fields are joined
with the ASCII unit separator 0x1f and terminated with the record separator 0x1e, the same
delimiter discipline as the history format described in
[the plumbing page](./plumbing-and-porcelain.md). Before any selector is ever passed back to Git,
`valid_stash_reference` (src/git/mod.rs:1396-1401) requires the exact shape `stash@{<digits>}`
with a non-empty all-digit body; entries that fail it are skipped while listing
(src/git/mod.rs:892-895) and `validate_stash_reference` (src/git/mod.rs:1403-1409) bails before
every stash operation and stash diff. A reflog selector is user-influenced text that will end up
in an argv, so it gets the same paranoia as a branch name.

The three-parent anatomy drives the stash preview read directly. `stash_diff_file`
(src/git/mod.rs:671-729) reads the tracked half as `git diff {ref}^1 {ref}`, then probes
`git rev-parse --verify --quiet {ref}^3` and, only when that commit exists and the first read was
not truncated, appends `git show --format= ... {ref}^3 -- <paths>` bounded by whatever remains of
the shared 8 MiB budget (`MAX_DIFF_BYTES.saturating_sub(output.len())`). A test named
`tracked_only_stash_preview_does_not_require_an_untracked_parent` (src/git/mod.rs:2047-2083) pins
that a stash without `^3` still previews. The subject strings Git writes into the stash reflog
(`WIP on <branch>: <message>` and `On <branch>: <message>`) are split back into branch and message
by `parse_stash_subject` (src/git/mod.rs:1411-1421), splitting on the first `": "`; anything else
degrades to an empty branch and the whole subject as the message.

## How Quinjet reads refs

Quinjet never opens a single file under `.git/refs`. Every ref read is a
[`git for-each-ref`](https://git-scm.com/docs/git-for-each-ref) subprocess with a delimiter-safe
format string, which buys three things at once: the loose/packed/reftable storage question
disappears, sorting happens inside Git, and the output is unambiguous bytes.

### The branch list

`Repository::branches` (src/git/mod.rs:801-831) runs:

```text
git for-each-ref --sort=-committerdate
  --format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:iso-strict)%1f%(objectname:short)%1e
  refs/heads
```

One process yields, for every local branch: the short name, a `*` marker when it is `HEAD`'s
branch, the upstream short name, the tip's committer date, and the abbreviated tip id. The fields
are joined with 0x1f and each record ends with 0x1e, so a branch name containing anything printable
parses correctly; those two control bytes cannot appear in ref names. Parsing is a byte-level
split: records on 0x1e, `trim_ascii` each, fields on 0x1f, destructure
`[name, head, upstream, relative_date, short_id, ..]`, with `current` being `*head == b"*"`.

Sorting by `-committerdate` happens in Git, not in Rust, so the sidebar shows most recently active
branches first without Quinjet ever comparing dates.

### The history branch list

`Repository::history_branches` (src/git/mod.rs:833-874) runs the same command shape over both
`refs/heads` and `refs/remotes`, with one extra field: `%(symref)`. Records whose symref field is
non-empty are skipped, which is precisely the filter that drops `refs/remotes/origin/HEAD`: it is
a symbolic ref pointing at the remote's default branch, not a branch anyone wants to inspect
twice. References that start with neither `refs/heads/` nor `refs/remotes/` are also skipped, and
`remote` is derived from the `refs/remotes/` prefix. The final ordering happens in one line
(src/git/mod.rs:872):

```rust
branches.sort_by_key(|branch| (!branch.current, branch.remote));
```

The sort is stable, so within each group Git's `-committerdate` order survives: the current branch
first, then local branches by recency, then remote branches by recency.

The `HistoryBranch` type documents its own safety contract (src/git/mod.rs:41-52): "A local or
remote-tracking branch that can be inspected without changing HEAD. `reference` is always a full
ref emitted by Git and is used only as a revision." That doc comment is the refs story of this
whole page in one sentence: full refs only, reads only, `HEAD` untouched.

### Decorations without a second ref walk

The history pane needs to know which refs point at which commits so it can draw `main`,
`origin/main`, and `tag: v1` badges. Quinjet does not correlate a ref list against a commit list;
it lets `git log --decorate=short` do it. The `%D` field in `LOG_FORMAT`
(src/git/history.rs:22-23) carries each commit's decorations, and `parse_record`
(src/git/history.rs:32-79) splits them on `,`, trims, and preserves entries such as
`HEAD -> main`, `origin/main`, and `tag: v1` (pinned by the test at src/git/history.rs:106-122).
One process, one pass, and the ref-to-commit join is done by the tool that owns the ref store.

### Where ref updates come from and who notices

Quinjet's own mutations move refs only when the user asks: `Checkout` runs
`git switch -- <branch>`, `CreateBranch` runs `git switch --create <name> [<start>]` after
`check-ref-format` validation, `RenameBranch` runs `git branch --move -- <old> <new>` (which
preserves the branch's tracking configuration, pinned by a test at src/git/mod.rs:1790-1839), and
`DeleteBranch` runs `git branch --delete -- <branch>` (src/git/mod.rs:945-1116). Everything else
that moves refs happens outside the process: the user's own `git` commands, another worktree,
an editor's Git integration. Quinjet notices all of them the same way, through the filesystem
watcher covered in [its own section](#how-quinjet-watches-a-repository): `.git/HEAD` and
`.git/refs/**` are deliberately not filtered as noise, so any ref movement triggers one coalesced
refresh, and the refresh re-runs the `for-each-ref` reads above.

## The index: the third tree

Git commands constantly compare three trees:

| Tree | Where it lives | Named in diffs as |
| --- | --- | --- |
| `HEAD` | The commit the current branch points at | the "staged" base |
| The index | `.git/index`, one binary file | what `commit` will snapshot |
| The working tree | Real files on disk | what the editor sees |

`git status` is two comparisons: `HEAD` versus index (staged changes) and index versus working
tree (unstaged changes). `git diff` with no arguments is the second comparison; `git diff --cached`
is the first. `git commit` writes the index's content as a tree object and seals it in a commit.

The index is more than a list of staged paths. For every tracked file, it caches the stat data of
the working-tree copy it last knew to be clean. That cache is the reason `git status` on a
100,000-file repository is an `lstat` walk rather than 100,000 file reads and hashes: a file whose
cached stat data still matches the filesystem is presumed unchanged without opening it. The
details, including the one race in this scheme, are in
[the stat cache section](#the-stat-cache-and-racy-git).

The index also holds merge state. During a conflicted merge, a path can appear at up to three
non-zero stages simultaneously: stage 1 is the common ancestor's version, stage 2 is "ours", and
stage 3 is "theirs". Stage 0 is the normal, resolved state. `git checkout --ours -- <path>` reads
stage 2 into the working tree; `git add <path>` collapses all stages back to a single stage-0
entry. This is exactly the pair of commands behind Quinjet's conflict resolution:
`ResolveConflict { path, choice }` runs `git checkout --ours|--theirs -- <path>` and then
`git add -- <path>` (src/git/mod.rs:1098-1106), a direct manipulation of index stages through
porcelain.

## The index file format

The on-disk format is specified in
[the gitformat-index manual](https://git-scm.com/docs/gitformat-index). It is a single file,
`.git/index` (or `.git/worktrees/<name>/index` for a linked worktree), with a fixed header, a
sorted array of entries, optional extensions, and a trailing checksum.

### Header

| Offset | Size | Field | Value |
| --- | --- | --- | --- |
| 0 | 4 | Signature | The bytes `DIRC` (for "directory cache") |
| 4 | 4 | Version | Big-endian 32-bit: 2, 3, or 4 |
| 8 | 4 | Entry count | Big-endian 32-bit number of entries that follow |

### One entry, version 2

Entries are sorted ascending by path bytes, then by stage number, which makes the whole file a
sorted flat map and lets readers binary-search it after memory-mapping.

| Offset | Size | Field | Meaning |
| --- | --- | --- | --- |
| 0 | 4 | ctime seconds | Last metadata change of the working-tree file |
| 4 | 4 | ctime nanoseconds | Fractional part, when the filesystem provides it |
| 8 | 4 | mtime seconds | Last content modification |
| 12 | 4 | mtime nanoseconds | Fractional part |
| 16 | 4 | dev | Device number from `stat` |
| 20 | 4 | ino | Inode number, truncated to 32 bits |
| 24 | 4 | mode | Object type bits plus Unix permission bits |
| 28 | 4 | uid | Owner user id |
| 32 | 4 | gid | Owner group id |
| 36 | 4 | file size | On-disk size, truncated to 32 bits |
| 40 | 20 | object id | SHA-1 of the blob this entry stages (32 bytes under SHA-256) |
| 60 | 2 | flags | See the bit table below |
| 62 | variable | path | Path bytes relative to the repository root, NUL-terminated |

The entry is then padded with NUL bytes so its total length is a multiple of 8 (versions 2 and 3).

The 16-bit flags word packs four fields:

| Bits | Field | Meaning |
| --- | --- | --- |
| 15 | assume-valid | User promised this file does not change (`update-index --assume-unchanged`) |
| 14 | extended | A second 16-bit flags word follows (version 3 and up) |
| 13-12 | stage | 0 normal; 1 base, 2 ours, 3 theirs during a merge |
| 11-0 | name length | Path length, or 0xFFF when the path is 4095 bytes or longer |

Version 3 adds the extended flags word when bit 14 is set, carrying the `skip-worktree` bit (used
by sparse checkout) and the `intent-to-add` bit (`git add -N`). Version 4 changes path storage:
instead of NUL-padded full paths, each entry stores a varint saying how many bytes to strip from
the previous entry's path, followed by the differing suffix. Because entries are sorted, adjacent
paths share long prefixes and the compression is substantial in deep trees.

### Extensions

After the entries come optional extensions, each `<4-byte signature> <32-bit size> <data>`.
A reader may ignore any extension whose signature starts with an uppercase letter; signatures
starting with a lowercase letter mark data the reader must understand to use the index at all.

| Signature | Name | What it caches |
| --- | --- | --- |
| `TREE` | Cache tree | Tree object ids for whole directories, so `commit` can reuse unchanged subtrees instead of rebuilding them |
| `REUC` | Resolve undo | The stage 1/2/3 entries removed when a conflict was resolved, so `checkout --merge` can recreate them |
| `link` | Split index | Marks this file as a delta against a shared base index |
| `UNTR` | Untracked cache | Per-directory mtimes and untracked listings, so `status` can skip `readdir` in unchanged directories |
| `FSMN` | File system monitor | A token and a bitmap of potentially-dirty entries, filled from an fsmonitor daemon |
| `EOIE` | End of index entry | The offset where entries end, so extensions can be located without parsing every entry |
| `IEOT` | Index entry offset table | Block offsets that let a multi-threaded reader parse entry ranges in parallel |
| `sdir` | Sparse directory | Marks the index as containing collapsed sparse-directory entries |

The closing element of the file is a hash over everything before it, which is how a torn or
corrupted index is detected on read.

Two of these extensions are pure performance features that shape what a status poll costs. The
untracked cache exists because `--untracked-files=all` (which Quinjet passes; see
[the status section](#how-quinjet-reads-status-without-taking-a-lock)) must otherwise `readdir`
every directory on every status. The fsmonitor extension exists to skip even the `lstat` walk by
asking a daemon what changed since the last token. Both are written into the index file itself,
which means both only help when something is allowed to write the index; that observation becomes
important one section down.

### Worked example: what one staged file looks like

Take an empty repository, one file, one `git add`:

```bash
printf 'hello\n' > greeting.txt
git add greeting.txt
```

The index now has one entry. Reading it with `git ls-files --debug` prints the cached stat fields:

```text
greeting.txt
  ctime: 1755648000:123456789
  mtime: 1755648000:123456789
  dev: 64769  ino: 8675309
  uid: 1000  gid: 1000
  size: 6  flags: 0
```

And `git ls-files --stage` shows the mode, staged blob id, and stage number:

```text
100644 ce013625030ba8dba906f756967f9e9ca394464a 0   greeting.txt
```

(The gap before the path in real `ls-files --stage` output is a single hard tab; it is rendered
with spaces here.) The blob id `ce01362...` is the hash of
`blob 6\0hello\n`, as explained in [the object model page](./object-model.md). The stat fields are
the cache: as long as `lstat("greeting.txt")` returns the same size, mtime, ctime, dev, ino, uid,
and gid, Git will report the file clean without reading its content.

## The stat cache and racy Git

The stat cache turns `git status` from O(total bytes) into O(files), and it is the single most
important performance property of the index for a tool that polls status. It also has one famous
correctness hole, and Git's fix for that hole is visible in status timings, so both halves are
worth understanding precisely.

### The fast path

For each index entry, `status` runs `lstat` on the path and compares the result against the cached
fields: mtime (seconds and, when `core.checkStat` is `default`, nanoseconds), ctime, size, dev,
ino, uid, gid, and mode. All equal means the file is presumed unchanged: no open, no read, no
hash. Any mismatch means the file is suspect, and Git reads and hashes its content to decide
whether it truly differs from the staged blob or was merely touched (`touch` changes mtime without
changing content; such an entry is refreshed rather than reported modified).

### The race

The cache compares timestamps with the granularity the filesystem provides, historically whole
seconds. Consider this sequence inside a single second, at timestamps shown as fractional seconds:

| Time | Event |
| --- | --- |
| 100.10 | `echo one > file` (mtime of `file` becomes 100) |
| 100.20 | `git add file` writes the index; entry caches mtime 100, size 4 |
| 100.30 | `echo two > file` (mtime is still 100, size is still 4) |

At 100.30 the file's content no longer matches the staged blob, but every cached stat field still
matches: same second, same size. A naive cache would call the file clean forever. This is the
"racy git" problem: an entry is *racily clean* when the file was modified within the same
timestamp granule in which the index was written.

### The two-part fix

Git closes the hole without giving up the cache:

**1. At read time, suspect the racy window.** When an entry's cached mtime is not older than the
index file's own mtime, the stat match is not trusted; Git reads and hashes the file's content
anyway. Only files modified in the same granule as the index write pay this cost, which is
normally a handful right after a commit or add.

**2. At write time, smudge racy entries.** Re-hashing the same racy entries on every future read
would be a permanent tax, so whenever Git writes a new index it detects entries that are racily
clean with respect to the new file's timestamp and smudges them: the cached size is zeroed. A
zero size can never match a non-empty file's `lstat` size, so the next read takes the slow
content-comparison path exactly once, then caches honest stat data.

Nanosecond timestamps shrink the racy window from a second to a nanosecond on filesystems that
store them, which makes the whole mechanism nearly free on modern Linux. The reason to understand
it here is the interaction with the next section: the write-time half of the fix, and every other
index-refresh benefit (updated stat data after a `touch`, a filled untracked cache, a fresh
fsmonitor token), only happens when the status-reading process is allowed to write the index.
Quinjet deliberately runs status in a mode where it never is, and accepts the cost knowingly.

## index.lock and optional locks

### How the index is written

The index has one writer at a time, enforced by a lock file:

1. Create `.git/index.lock` with `O_CREAT | O_EXCL`. If the file already exists, fail immediately
   with the familiar message about being unable to create the lock; Git never waits for it.
1. Write the entire new index into the lock file: header, all entries, extensions, checksum.
1. `rename(".git/index.lock", ".git/index")`.

Because the rename is atomic, readers are never blocked and never see a partial index: a reader
opens `.git/index` and gets either the complete old file or the complete new file. The lock is
exclusively a writer-versus-writer mechanism. This is the same protocol as
[ref updates](#updating-a-ref-safely), applied to a bigger file, and it has the same two
properties: reads are free, and contention surfaces as an immediate, harmless error instead of a
deadlock or a corruption.

A crashed writer leaves a stale `.git/index.lock` behind, since only a successful writer renames
it away. Git detects nothing automatically; the next writer fails with the lock error, and modern
Git prints a hint about removing the file if no other process is running. This failure mode is why
transient lock files matter to any tool that watches a repository: they appear and vanish
constantly during normal use, they mean nothing to a reader, and reacting to them is pure noise.
Quinjet's watcher filters `index.lock` explicitly, as
[the watcher section](#how-quinjet-watches-a-repository) shows.

### The opportunistic write inside git status

Here is the subtle part. `git status` is conceptually a read, but by default it is also a write.
While computing its answer, status refreshes the index: it re-stats every entry, hashes racily
clean files, and produces an updated in-memory index with fresh stat data, an updated untracked
cache, and smudge repairs. Throwing that work away would make the next status pay it again, so by
default status takes `index.lock` and writes the refreshed index back, as a side effect of a
read-only question.

Git calls this an *optional lock*: the command can do its job without it, and the write is merely
an optimization for the next command. [The git-status manual](https://git-scm.com/docs/git-status)
documents the switch that disables it, `--no-optional-locks`, along with the environment variable
`GIT_OPTIONAL_LOCKS=0` (available since Git 2.15), and its guidance is aimed squarely at
background tooling: processes that poll status should not take locks the user's own commands might
collide with.

What goes wrong without the switch is concrete:

- The user runs `git rebase` or `git commit`, which holds `index.lock` at the moment the
  background poll fires; or the reverse, the poll's opportunistic write holds the lock at the
  moment the user's command needs it. Either order produces a spurious "unable to create
  index.lock" failure in a command that should have succeeded.
- Every poll writes `.git/index`, and every write is a filesystem event. A tool that both watches
  the repository and polls status on watcher events builds itself a feedback loop: poll, write,
  event, poll.

### What GIT_OPTIONAL_LOCKS=0 changes and what it costs

With the variable set to `0`, status still refreshes the index in memory and still answers with
full accuracy; it simply never takes the lock and never writes the refreshed result back. The cost
is that all refresh work becomes repeatable: stat data updated by the poll is not persisted, a
racily clean file is re-hashed by every subsequent status instead of being smudged once, and the
untracked cache and fsmonitor extensions never get updated by the polling process. In exchange,
the poll is invisible: it cannot collide with the user, it cannot generate index writes, and it
cannot perturb the very filesystem events it is watching.

That trade is exactly right for a status poll that runs on a timer from a background thread, which
is why Quinjet buys it globally.

## How Quinjet reads status without taking a lock

### The environment on every Git invocation

Quinjet has one function that constructs every ordinary Git command, and the optional-locks
setting is baked into it, not sprinkled per call site. `Repository::run` in src/git/mod.rs
(lines 1292-1309):

```rust
fn run<I, S>(&self, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    let _ = command
        .arg("-C")
        .arg(&self.root)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
        .output()
        .with_context(|| format!("failed to execute Git in {}", self.root.display()))
}
```

Every wrapper goes through this: `checked` (src/git/mod.rs:1280-1290) adds exit-code handling,
and `checked_bounded` (src/git/mod.rs:1258-1278) builds the same command but streams stdout
through a capped pipe. The same environment, including `GIT_OPTIONAL_LOCKS=0`, is applied to Git
run in other directories by `run_repository_git` (src/git/github/mod.rs:2192-2209), which serves
the disposable pull-request workspace. There is no code path that spawns Git without it. This is
invariant 13 in ARCHITECTURE.md: "Read operations set `GIT_OPTIONAL_LOCKS=0`; `gh` runs with
prompts, paging, color, and update checks disabled on the worker thread."

Each of the other fixed pieces has its own job, described fully in
[the plumbing page](./plumbing-and-porcelain.md): `-C <root>` avoids any reliance on the process
working directory, `core.quotepath=false` makes non-ASCII paths come out as raw bytes for the
byte-oriented parsers, `LC_ALL=C` forbids localized output, and `GIT_TERMINAL_PROMPT=0` turns a
would-be credential prompt into an immediate error rather than a frozen worker thread.

Setting the variable on mutating commands too is deliberate and harmless: optional locks are, by
definition, the ones a command can skip without changing its outcome. `git add` still takes
`index.lock`, because for `add` the index write is the point, not an optimization.

### The status invocation

`Repository::status` (src/git/mod.rs:287-297) issues:

```text
git status --porcelain=v2 --branch -z --untracked-files=all --ignore-submodules=none
```

With `GIT_OPTIONAL_LOCKS=0` in the environment, this is a pure read of `.git/index` plus an
`lstat` walk of the working tree. Every flag earns its place:

- `--porcelain=v2` selects the stable machine format whose records expose index stages, rename
  scores, and both index and worktree status codes per path.
- `--branch` folds the `# branch.*` headers into the same read, so head, oid, upstream, and
  ahead/behind counts cost no second process.
- `-z` terminates records with NUL, so arbitrary path bytes survive; a rename's original path
  arrives as the following NUL-terminated field.
- `--untracked-files=all` lists each untracked file rather than collapsing directories, because
  staging and discard operate per file.
- `--ignore-submodules=none` keeps submodule changes visible.

The reply is parsed by `parse_porcelain_v2` (src/git/status.rs:123-170) without a single string
allocation until field extraction, walking NUL records and dispatching on the first byte: `1` for
ordinary changes, `2` for renames and copies (which consume the following record as the original
path), `u` for unmerged paths, `?` for untracked. The index-stage machinery from
[the index section](#the-index-the-third-tree) surfaces directly here: a `u` record is a path
whose index holds stage 1, 2, and 3 entries, and Quinjet maps it to `ChangeArea::Conflict`, which
sorts first because `ChangeArea` derives `Ord` with `Conflict` declared before `Staged` and
`Unstaged` (src/git/status.rs:8-22). An ordinary record whose staged code and worktree code are
both non-dot yields two `Change` entries, one per area (`push_xy_changes`,
src/git/status.rs:236-266), matching how the index actually holds a staged version distinct from
the working tree.

#### Record layouts, field by field

The porcelain v2 format deserves its layouts spelled out, because Quinjet's parser is positional:
each record type is split into a fixed number of space-separated fields with `splitn_bytes`, and
the field count is what protects paths containing spaces. First the branch headers, one per line,
parsed by `parse_branch_header` (src/git/status.rs:172-199):

| Header | Value | Notes |
| --- | --- | --- |
| `# branch.oid` | Commit id or `(initial)` | `(initial)` marks an unborn branch and is skipped |
| `# branch.head` | Branch name or `(detached)` | Detached substitutes the oid's first 8 chars |
| `# branch.upstream` | Upstream short name | Absent when no upstream is configured |
| `# branch.ab` | `+<ahead> -<behind>` | Parse failures default both counts to 0 |

An ordinary change record (type `1`) has nine fields, so `parse_ordinary`
(src/git/status.rs:201-210) uses `splitn_bytes(record, b' ', 9)` and the ninth field, the path,
keeps any further spaces intact:

| Field | Content |
| --- | --- |
| 0 | The literal `1` |
| 1 | `XY`: staged status letter, then worktree status letter; `.` means unchanged on that side |
| 2 | Submodule state (`N...` for a plain file) |
| 3, 4, 5 | Octal file modes in `HEAD`, in the index, in the working tree |
| 6, 7 | Object ids of the `HEAD` version and the index version |
| 8 | The path, raw bytes to the record's NUL |

A rename or copy record (type `2`) adds a tenth field, the similarity score (`R100`, `C87`), and
in `-z` mode the *original* path arrives as the entire next NUL-terminated record rather than as
a tab-separated suffix; `parse_renamed` (src/git/status.rs:212-221) splits to 10 fields and the
caller consumes the following record as the pre-image path. An unmerged record (type `u`) is the
index's stage machinery printed directly:

| Field | Content |
| --- | --- |
| 0 | The literal `u` |
| 1 | `XY` conflict classification (`UU`, `AA`, `DU`, ...) |
| 2 | Submodule state |
| 3, 4, 5, 6 | Octal modes of stage 1, stage 2, stage 3, and the working tree |
| 7, 8, 9 | Object ids of stages 1, 2, and 3 |
| 10 | The path |

`parse_unmerged` (src/git/status.rs:223-234) splits to 11 fields and takes the eleventh as the
path. Those three mode fields and three oid fields are the same stage 1/2/3 entries described in
[the index section](#the-index-the-third-tree); porcelain v2 is the only status format that
exposes them, which is one reason Quinjet refuses to parse anything older. Finally, an untracked
record is just `? <path>`: the path starts at byte offset 2. Because the whole stream is `-z`
NUL-terminated and `core.quotepath=false` is forced, `bytes_to_path`
(src/git/status.rs:284-286) can convert path bytes with a single lossy UTF-8 step and no
unquoting logic at all.

### How often status runs, and why lock-free matters at that rate

The status read is not rare. It fires from four independent triggers:

- a 10-second periodic tick in the main loop (src/main.rs:114), the repository heartbeat;
- every filesystem watcher signal, coalesced per event storm;
- the completion of every user mutation, so the UI reconciles immediately;
- explicit refresh requests from app logic.

An interactive session in a busy repository can easily run status thousands of times. With
optional locks on, each of those runs would have attempted an index write; any one of them landing
during the user's own `git commit`, `git rebase`, or an IDE's Git operation is a visible failure
in someone else's tooling, caused by a viewer. With `GIT_OPTIONAL_LOCKS=0` the entire class of
collision is impossible by construction. It cannot be rate-limited into being rare; it is zero.

The watcher interaction is just as important. Because the poll never writes `.git/index`, a status
run generates no filesystem events, so the watcher-to-refresh path cannot feed itself. The noise
filter's `index.lock` entry (next section) then handles the other half: index writes by other
processes announce themselves through the final rename of `.git/index`, not through their
transient lock file.

### Working-tree diffs against the index

The two-sided nature of the index shows up again in how Quinjet counts working-tree changes.
`apply_worktree_counts` (src/git/mod.rs:469-509) fills per-file `+n -n` totals for the Changes
view with at most two extra Git calls regardless of file count, and the two calls are precisely
the two index comparisons:

- `git diff --numstat -z --find-renames --` for the unstaged side (index versus working tree),
  issued only when some change belongs to the unstaged area;
- the same with `--cached` for the staged side (`HEAD` versus index), issued only when some
  change is staged.

Its doc comment (src/git/mod.rs:469-471) states the budget: "Working-tree changes are already
known from the status snapshot, so the index needs only their totals. One `--numstat` read per
populated area keeps that to at most two extra Git calls regardless of file count." Untracked
files get no counts from either read, because the index has never heard of them; the test at
src/git/mod.rs:1876-1923 asserts exactly that, and their patch is synthesized without Git at all
by `untracked_patch` (src/git/mod.rs:1118-1165), which reads the file directly (path-guarded by
`safe_worktree_path`, capped at the 8 MiB patch budget, any NUL byte meaning binary) and fabricates
a `new file mode 100644` unified diff.

Per-file patch reads for staged entries use `git diff --cached --unified=N -- <path>` and for
conflicted entries `git diff --cc -- <path>` (`raw_diff_for_change`, src/git/mod.rs:748-788), the
combined-diff form that reads stages 2 and 3 out of the index and shows both sides against the
working tree. Every one of these runs under the same no-optional-locks environment: a reader can
scroll through a conflicted merge in Quinjet while a rebase holds `index.lock`, and neither side
notices the other.

### Index-first workspaces: one listing, then lazy patches

Quinjet's local diff previews are built "index-first", and the term needs disambiguating on this
page of all pages: the index in question is Quinjet's `DiffIndex` (a title, a file list with
per-file counts, and a truncation flag), not Git's `.git/index`. The naming collision is not an
accident, though; both structures exist for the same reason. Git's index lets `status` answer
"what changed" without reading file contents; Quinjet's diff index lets a preview render every
file header without producing a single patch. Cheap metadata first, expensive content lazily.

Opening any local diff (the working tree, a commit, a branch comparison, a stash) runs
`prepare_local_diff` (src/git/mod.rs:359-369), which builds the index in one pass and captures
everything needed to serve patches later. The stored workspace, `PreparedLocalDiff` in
src/git/mod.rs (lines 129-144):

```rust
pub(crate) struct PreparedLocalDiff {
    repository: Repository,
    request: LocalDiffRequest,
    index: DiffIndex,
}
impl PreparedLocalDiff {
    pub(crate) fn index(&self) -> DiffIndex { self.index.clone() }
    pub(crate) fn diff_file(&self, path: &Path) -> Result<DiffDocument> {
        self.repository.local_diff_file(&self.request, &self.index, path)
    }
}
```

Each request variant maps onto the ref and index machinery from earlier sections
(`local_diff_index`, src/git/mod.rs:375-467):

- `Changes` builds its file list with *no Git call at all*: the status snapshot already names
  every changed path, so the index is assembled from `Vec<Change>` directly and only the two
  per-area `--numstat` reads follow for totals.
- `Commit` runs `git diff --name-status -z --find-renames <parent> <id> --`, or the `diff-tree
  --root` form for a parentless commit.
- `Branch` first passes `validate_history_reference` (full `refs/heads/` or `refs/remotes/` refs
  only), then diffs `<reference>` against `HEAD`, both resolved names, neither ever checked out.
- `Stash` runs `git stash show --name-status -z --include-untracked <ref> --` after the selector
  validation described in [the reflog section](#reflogs).

Every revision-based index also runs the same argv with exactly one token swapped,
`--name-status` for `--numstat` (`numstat_args`, src/git/mod.rs:1324-1339), whose doc comment
states the invariant: "Reuse an index command's own revision range for its totals by swapping the
listing option. This keeps the two reads describing exactly the same diff." That is invariant 8a
in ARCHITECTURE.md: "Every index also reads `git diff --numstat` over the same range, so a header
shows its real `+n -n` before that file has a patch. A file's totals never depend on whether its
patch has loaded." A branch-comparison test (src/git/mod.rs:1961-1978) asserts every entry has
counts before any patch is read. And because counts are decoration, not correctness, a failed or
capped numstat read degrades to placeholder counts rather than an error (`numstat_counts`,
src/git/mod.rs:513-517: "Counts are a rendering enhancement, never a correctness requirement").

After the index, each file's patch is one bounded Git call made only when that file becomes
visible, against the same captured request, with the whole listing capped at 8 MiB and 16,384
entries and every patch read capped at 8 MiB with kill-on-overflow pipes. The document model and
the caps live in [the diff pipeline page](../diff/pipeline.md); what matters here is the
refresh economics: a periodic status snapshot compares the new `LocalDiffRequest` against the
stored one and skips rebuilding when they are equal (src/app.rs:6541-6544), with the
`changes_diff_version` stamp from the watcher path being what forces re-preparation after a real
filesystem change. Invariant 8 summarizes the whole scheme: "A bounded name/status index produces
collapsed headers first ... Periodic status snapshots do not rebuild an unchanged comparison."

## Linked worktrees and the common directory

### One store, many checkouts

[`git worktree`](https://git-scm.com/docs/git-worktree) (in Git since 2.5) lets one repository
have several working directories at once. The design splits repository state into two classes:
state that describes *the repository* (objects, refs, configuration) and state that describes
*one checkout* (which commit is checked out, what is staged, where a rebase is paused). The first
class lives once, in the *common directory*; the second is duplicated per worktree.

The main worktree is the one created by `clone` or `init`; its `.git` is a real directory and
doubles as the common directory. A linked worktree, created with `git worktree add <path> <ref>`,
gets a `.git` *file* instead:

```text
$ cat /mnt/sandisk/codingAndFun/samaan/quinjet-wt/docs-optimization/.git
gitdir: /mnt/sandisk/codingAndFun/samaan/quinjet/.git/worktrees/docs-optimization
```

That one line redirects Git: the worktree's private directory is
`<main>/.git/worktrees/<name>/`, and inside it a `commondir` file (containing `../..`) points back
up to the shared `.git`. The layout, specified in
[the gitrepository-layout manual](https://git-scm.com/docs/gitrepository-layout), looks like this
for a repository with one linked worktree:

```text
main/.git/
├── HEAD                    per-worktree (the main worktree's)
├── index                   per-worktree (the main worktree's)
├── config                  shared
├── objects/                shared
├── refs/                   shared
├── packed-refs             shared
├── logs/
│   ├── HEAD                per-worktree (the main worktree's)
│   └── refs/               shared
└── worktrees/
    └── docs-optimization/
        ├── HEAD            this worktree's checked-out ref
        ├── index           this worktree's staging area
        ├── commondir       contains "../.."
        ├── gitdir          absolute path back to the worktree's .git file
        ├── ORIG_HEAD       per-worktree operation state
        ├── logs/HEAD       this worktree's HEAD reflog
        └── refs/           per-worktree refs (bisect state and similar)
```

The precise split:

| State | Scope | Why |
| --- | --- | --- |
| `objects/`, `packed-refs`, `refs/*` (most) | Shared | History and names are properties of the repository |
| `config`, `hooks/`, `info/` | Shared | One configuration for all checkouts (unless `extensions.worktreeConfig`) |
| `HEAD`, `logs/HEAD` | Per-worktree | Each checkout sits on its own branch or commit |
| `index` | Per-worktree | Each checkout stages independently |
| `MERGE_HEAD`, `ORIG_HEAD`, sequencer state | Per-worktree | A rebase in one worktree must not confuse another |
| `refs/bisect/*`, `refs/worktree/*`, `refs/rewritten/*` | Per-worktree | The exceptions inside the shared ref namespace |

Git resolves the two scopes through `$GIT_DIR` (the per-worktree directory) and
`$GIT_COMMON_DIR` (the shared one): a path like `refs/heads/main` is looked up under the common
directory, while `HEAD` or `index` resolves under the per-worktree directory. Because each
worktree has its own `HEAD` and index but shares refs, Git refuses to check out a branch that is
already checked out in another worktree; two `HEAD`s pointing at the same moving branch would
corrupt each other's expectations on commit.

Two attributes decorate a worktree's lifecycle. A worktree can be *locked* (a `locked` file in its
private directory, optionally containing a reason), which exempts it from pruning; that is meant
for worktrees on removable or network storage whose paths disappear legitimately. And a worktree
becomes *prunable* when its working directory has vanished: `git worktree prune` deletes the
orphaned `worktrees/<name>/` directory by checking whether the path in its `gitdir` file still
exists.

### Finding the common directory

Everything Quinjet does with worktrees hangs off one plumbing call. `Repository::git_common_dir`
in src/git/mod.rs (lines 923-939):

```rust
pub(crate) fn git_common_dir(&self) -> Result<PathBuf> {
    let output = self.checked([
        OsString::from("rev-parse"),
        OsString::from("--git-common-dir"),
    ])?;
    let raw = text(trim_ascii(&output));
    if raw.is_empty() {
        bail!("Git returned an empty common directory");
    }
    let path = Path::new(&raw);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        self.root.join(path)
    };
    Ok(fs::canonicalize(&resolved).unwrap_or(resolved))
}
```

`git rev-parse --git-common-dir` answers relative to the working directory when it can (typically
the literal `.git` in a main worktree), so the result is joined onto the repository root and then
canonicalized when possible. Canonicalization matters because this path becomes an identity: two
worktrees of the same repository must produce byte-identical common directories for the
deduplication below to work, even when one of them reached the repository through a symlink.

The value is used for three distinct jobs:

**1. Project identity.** The recent-projects list (src/state.rs) keys every entry by common
directory, so all worktrees of one repository collapse into a single project. From
src/state.rs (lines 24-42):

```rust
pub(crate) fn record_recent_project(root: &Path) {
    let Ok(repository) = Repository::discover(root) else {
        return;
    };
    let Ok(common_dir) = repository.git_common_dir() else {
        return;
    };
    let mut entries = read_entries();
    entries.retain(|entry| entry.common_dir != common_dir);
    entries.insert(
        0,
        RecentEntry {
            path: repository.root().to_path_buf(),
            common_dir,
        },
    );
    entries.truncate(MAX_RECENT_PROJECTS);
    write_entries(&entries);
}
```

Opening the same repository from a second worktree does not create a second project entry; the
`retain` removes the old entry for that common directory and the fresh one goes to the front, up
to `MAX_RECENT_PROJECTS = 20`. Loading the list back (`load_recent_projects`,
src/state.rs:50-71) re-opens each repository, lists its worktrees, and groups them into a
`ProjectGroup { name, common_dir, worktrees }` (src/git/mod.rs:98-104), so the Projects modal
shows one row per repository with all its checkouts beneath it. Dead entries whose paths no
longer exist are silently skipped, and the same common-directory key drives `forget_recent_project`
when the user deletes an entry.

**2. The extra watch root.** A linked worktree's ref updates happen in the common directory,
outside the worktree root, so the watcher adds a second recursive watch there. The
[next section](#how-quinjet-watches-a-repository) covers it in full.

**3. Object borrowing for the pull-request workspace.** The disposable bare repository writes
`<common_dir>/objects` into its own `objects/info/alternates`, so objects already on local disk
resolve without any network transfer. That mechanism belongs to
[the packfile page](./packfiles-and-deltas.md) and
[the PR workspace page](../github/pr-workspace.md), but the path it borrows is the common
directory found here, and borrowing is read-only by construction: an alternates file lives in the
borrower, never in the lender.

### Listing worktrees

`Repository::worktrees_relative_to` (src/git/mod.rs:913-921) runs:

```text
git worktree list --porcelain -z
```

The porcelain format is a sequence of stanzas, one per worktree, each a series of
attribute records; with `-z` every record is NUL-terminated and stanzas are separated by an empty
record, that is, two NULs in a row. `-z` is not decoration here: a lock reason is free text that
may contain newlines, and a worktree path may contain anything the filesystem allows, so the
newline-based format is ambiguous in exactly the cases a parser must not guess about. A concrete
stream, written with `\0` standing for the NUL byte, is pinned verbatim by the test at
src/git/mod.rs:2159-2171:

```text
worktree /tmp/repo\0HEAD abcdef0123456789\0branch refs/heads/main\0\0
worktree /tmp/repo-topic\0HEAD fedcba9876543210\0branch refs/heads/topic\0locked busy\0\0
worktree /tmp/repo-hot\0HEAD 0123456789abcdef\0detached\0prunable\0\0
```

(The line breaks above are for the page; the real stream is continuous bytes.) The parser,
`parse_worktrees` (src/git/mod.rs:1423-1444), splits on NUL and treats an empty field as the end
of a stanza; `worktree_from_fields` (src/git/mod.rs:1446-1482) then reads each record by prefix:

```rust
for field in fields {
    if let Some(value) = field.strip_prefix(b"worktree ") {
        path = Some(parse_worktree_path(value));
    } else if let Some(value) = field.strip_prefix(b"HEAD ") {
        head = text(value);
    } else if let Some(value) = field.strip_prefix(b"branch ") {
        branch = Some(heads_branch_name(&text(value)));
    } else if *field == b"detached" {
        detached = true;
    } else if *field == b"bare" {
        bare = true;
    } else if let Some(value) = field.strip_prefix(b"locked") {
        locked = Some(text(value).trim().to_owned());
    } else if let Some(value) = field.strip_prefix(b"prunable") {
        prunable = Some(text(value).trim().to_owned());
    }
}
```

Note the two prefix styles: `locked` and `prunable` are matched without a trailing space because
they are legal both bare (no reason) and with a reason; the test asserts that a bare `prunable`
yields `Some("")`, distinguishing "prunable with no stated reason" from "not prunable". The
`branch` value arrives as a full ref (`refs/heads/topic`) and is shortened for display by
`heads_branch_name` (src/git/mod.rs:1484-1489). Unknown attribute records fall through silently,
which is what makes the parser forward-compatible with new porcelain attributes.

One field the porcelain does not provide is computed locally: which listed worktree is *this*
session. `current` is decided by `same_path(&path, session_root)` (src/git/mod.rs:1503-1511),
which compares the raw paths and falls back to comparing both `fs::canonicalize` results, so a
worktree reached through a symlink still matches itself. On Windows, `parse_worktree_path`
(src/git/mod.rs:1491-1501) rewrites the forward slashes Git prints into backslashes so the
comparison uses native separators. A test at src/git/mod.rs:2183 verifies the property in the
function's name: `lists_a_linked_worktree_without_changing_head`. Listing is a read.

The resulting `Worktree` struct (src/git/mod.rs:64-96) carries `path`, `head`, `branch`,
`current`, `bare`, `detached`, `locked`, and `prunable`, with `short_head()` producing the first
8 characters of the head oid and `branch_label()` degrading to `bare`, `detached`, or a dash when
there is no branch. The sidebar's project list is rendered straight from these fields; no second
Git call is needed per row.

## How Quinjet watches a repository

Polling alone cannot make a Git TUI feel live: a 10-second heartbeat means up to 10 seconds of
lying about status after every `git commit` the user runs elsewhere. Quinjet pairs the heartbeat
with a filesystem watcher, and the watcher's design is where the refs, index, and worktree
knowledge from this page all lands at once. The whole implementation is 87 lines, src/watch.rs.

### Wiring: the root watch plus the common-directory watch

The watcher is constructed at terminal startup, right after repository discovery. From
src/main.rs (lines 100-104):

```rust
let repository = Repository::discover(&options.path)?;
state::record_recent_project(repository.root());
let common_dir = repository.git_common_dir().ok();
let mut worker = GitWorker::start(repository.clone());
let mut watcher = RepoWatcher::with_extra(repository.root(), common_dir.as_deref()).ok();
```

Both `.ok()` calls are policy, not sloppiness: a repository whose common directory cannot be
resolved still opens, and a platform where the watcher cannot start (inotify limits exhausted, an
unsupported filesystem) degrades to the 10-second heartbeat instead of failing the launch. The
same wiring is repeated when the user switches projects inside the TUI (src/main.rs:242-273), so
a newly opened repository gets its own watcher pair.

The constructor, `RepoWatcher::with_extra` in src/watch.rs (lines 13-27):

```rust
pub(crate) fn with_extra(root: &Path, extra: Option<&Path>) -> Result<Self> {
    let (sender, receiver) = bounded(1);
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };
        if should_refresh(&event) && sender.try_send(()).is_err() {}
    })
    .context("failed to create filesystem watcher")?;
    watcher
        .watch(root, RecursiveMode::Recursive)
        .with_context(|| format!("failed to watch {}", root.display()))?;
    if let Some(extra) = extra.filter(|path| path.exists() && !path.starts_with(root)) {
        drop(watcher.watch(extra, RecursiveMode::Recursive));
    }
    ...
}
```

The [notify crate](https://docs.rs/notify) provides the platform backend (inotify on Linux,
FSEvents on macOS, ReadDirectoryChangesW on Windows). Two recursive watches are registered:

**1. The worktree root.** This covers every working-tree file the user edits, and, in a main
worktree, the entire `.git` directory too, since it sits inside the root.

**2. The Git common directory, only when it is elsewhere.** The `filter` on `extra` encodes the
worktree layout precisely: the extra watch is added only when the common directory exists and is
*not* under the root. In a main worktree, `.git` starts with the root and the recursive root
watch already covers it, so the extra watch would be a duplicate. In a linked worktree, the
common directory is the main checkout's `.git`, somewhere else entirely, and without watching it
Quinjet would be blind to every ref update: a commit made in another worktree, a `git fetch` run
anywhere, a branch switch in the main checkout, and even this worktree's own `HEAD` movement,
because a linked worktree's `HEAD` lives at `<common>/worktrees/<name>/HEAD`, inside the common
directory rather than the worktree root.

That last point deserves emphasis, because it is easy to get wrong: in a linked worktree,
*nothing* under `.git` is under the worktree root except the one-line `.git` redirect file. Every
piece of repository state a TUI cares about (refs, `packed-refs`, all `HEAD`s, all indexes, all
logs) physically lives under the common directory. The extra watch is not an enhancement for an
edge case; in a linked worktree it is the only source of Git-state events. ARCHITECTURE.md lists
it as part of the module's contract: "repository watcher, extra watch on the Git common directory
so linked worktrees refresh, and event-storm coalescing" (src/watch.rs entry in the Layers list).

A failure to add the extra watch is deliberately ignored (`drop(...)`): a repository whose common
directory disappears mid-session is still usable, just less live, and the heartbeat still covers
it.

### The noise filter: what a Git write storm looks like

A recursive watch on `.git` is a firehose. A single `git commit` in a busy repository produces
events for: new loose object files under `.git/objects/` (or a pack under `objects/pack/`),
`index.lock` appearing, `.git/index` being replaced, the branch's loose ref lock and rename, two
reflog appends, and assorted temporary files. A `git fetch` of a large remote can write thousands
of object files. Reacting to each event with a status poll would melt the CPU for nothing: the
poll's answer only changes when the *visible* state changes.

Quinjet's filter is two small functions. `should_refresh` (src/watch.rs:40-45):

```rust
fn should_refresh(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|path| !is_noisy_git_path(path))
}
```

Access events (reads) are dropped wholesale; reading a repository never changes what status would
say. Then an event counts only if at least one of its paths is not a noisy Git path, where
`is_noisy_git_path` (src/watch.rs:47-68) implements a three-rule classification:

```rust
fn is_noisy_git_path(path: &Path) -> bool {
    let components: Vec<_> = path.components().collect();
    let Some(git_index) = components
        .iter()
        .position(|component| component.as_os_str() == ".git")
    else {
        return false;
    };
    let tail = components.get(git_index + 1..).unwrap_or_default();
    if tail
        .first()
        .is_some_and(|component| component.as_os_str() == "objects")
    {
        return true;
    }
    tail.last().is_some_and(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "index.lock" || name.starts_with(".watchman-cookie-")
    }) || components
        .iter()
        .any(|component| matches!(component, Component::ParentDir))
}
```

Reading it against the formats earlier in this page:

- A path with no `.git` component is never noise: it is a working-tree edit, the most legitimate
  refresh trigger there is.
- Everything under `.git/objects/` is noise. Object writes are the storm during fetch, commit,
  and gc, and by themselves they change nothing visible: an object is invisible until a ref or the
  index points at it, and *that* write produces its own event.
- A final component of `index.lock` is noise. As
  [the lock section](#indexlock-and-optional-locks) showed, the lock file is a writer-private
  scratch file; the meaningful moment is the rename onto `.git/index`, and the rename's event is
  for the path `.git/index`, which the filter does not match and therefore passes through.
- A final component starting with `.watchman-cookie-` is noise. Watchman, the file-watching
  service used by Git's fsmonitor hook among others, synchronizes its event stream by dropping
  short-lived cookie files into the tree it watches and waiting to observe them; when a repository
  has fsmonitor tooling active, these cookies appear under `.git` at arbitrary moments and mean
  nothing to anyone but Watchman.
- Any path containing a `..` component is treated as noise, refusing to reason about paths that
  escape the watched tree.

What deliberately passes the filter is just as informative. The unit test at src/watch.rs:76-86:

```rust
assert!(is_noisy_git_path(Path::new("/repo/.git/objects/ab/cdef")));
assert!(is_noisy_git_path(Path::new("/repo/.git/index.lock")));
assert!(!is_noisy_git_path(Path::new("/repo/.git/HEAD")));
assert!(!is_noisy_git_path(Path::new("/repo/.git/refs/heads/main")));
assert!(!is_noisy_git_path(Path::new(
    "/repo/.git/worktrees/topic/gitdir"
)));
assert!(!is_noisy_git_path(Path::new("/repo/src/main.rs")));
```

`.git/HEAD` and `.git/refs/heads/main` must not be noise: they are exactly the branch switches and
commits, from any process and any worktree, that the sidebar has to reflect. `.git/worktrees/*`
paths must not be noise either, since per-worktree `HEAD`s and worktree lifecycle files live
there, and thanks to the common-directory watch those paths are observed for every worktree, not
just the session's own. Note also what the filter does not need to list: `.git/index` (the real
one) passes and triggers the refresh that reconciles staged state after any `git add` performed
outside Quinjet.

The layering with `GIT_OPTIONAL_LOCKS=0` is worth restating as a system property. Quinjet's own
status polls write nothing, so they generate no events at all; other processes' index writes are
filtered down from a lock-churn storm to the single decisive `index` rename; object storms are
filtered to the single ref or index write that makes them visible. The result is that one logical
repository change becomes approximately one watcher signal, which the channel then reduces to
exactly one refresh.

### Coalescing: a thousand events, one refresh

The callback in `with_extra` sends into a `crossbeam_channel::bounded(1)` channel of `()`, and the
send is a `try_send` whose error is explicitly discarded:

```rust
if should_refresh(&event) && sender.try_send(()).is_err() {}
```

This is the entire coalescing algorithm, and it is worth spelling out why it is correct rather
than merely convenient. The channel carries no payload: a signal does not say *what* changed, only
*that* something did, because the response is always the same full re-read (status, and from
status whatever else follows). Given that, a second pending signal adds no information, so a
capacity of one loses nothing: when the slot is full, `try_send` fails, the error is ignored, and
the thousandth event of a rebase costs one failed enqueue on the watcher thread. The main loop
drains the channel with `watcher_changed` (src/main.rs:193-202), which loops
`while receiver.try_recv().is_ok()` into a single boolean per loop iteration, so even the
one-plus-in-flight case collapses to one refresh. ARCHITECTURE.md states the invariant as number
4: "Watcher signals are lossy by design: one full status snapshot subsumes all preceding file
events."

On the app side, `filesystem_changed` (src/app.rs:3067-3074) does three things: bumps
`changes_diff_version` (a monotonically increasing stamp embedded in working-tree diff requests,
so an identical-looking request after a filesystem change still compares unequal and re-runs);
invalidates the current preview only when the Changes view is showing the working tree; and
requests a status refresh. The scoping is deliberate: a watcher event never restarts a branch or
stash comparison the reader is studying, because those compare immutable commits that a
working-tree edit cannot affect. A test at src/app.rs:10053 pins it: "background status and
collapse do not restart a branch comparison."

The webhook listener (src/webhook.rs) is the same pattern one level up: a forwarded GitHub
delivery is treated purely as a signal that something changed on the pull-request side, drained
with the same boolean-collapse loop, and answered by re-reading through authenticated channels.
The shared philosophy across both: external signals are hints to read, never sources of data.
Details live in [the concurrency page](../rendering/concurrency.md).

### Worked example: one external commit, end to end

Put the whole chain together. The session has Quinjet open in a linked worktree at
`/work/topic`, whose common directory is `/main/.git`. In another terminal, the user commits from
the *main* worktree. What the mutable layer does, and what Quinjet sees:

| Step | Filesystem effect | Watch that sees it | Filter verdict |
| --- | --- | --- | --- |
| Objects written | `/main/.git/objects/...` created | common-dir watch | Noise (`objects` after `.git`) |
| Index locked | `/main/.git/index.lock` created | common-dir watch | Noise (`index.lock` tail) |
| Index replaced | rename onto `/main/.git/index` | common-dir watch | Signal |
| Branch ref locked | `/main/.git/refs/heads/main.lock` | common-dir watch | Signal (only `index.lock` is filtered) |
| Branch ref renamed | `/main/.git/refs/heads/main` | common-dir watch | Signal |
| Reflogs appended | `/main/.git/logs/...` | common-dir watch | Signal |

Several events pass the filter, and it does not matter how many: each one attempts
`try_send(())` into the capacity-1 channel, the first fills the slot, and the rest fail silently.
On its next iteration the main loop drains the channel to a single boolean and calls
`filesystem_changed`, which bumps `changes_diff_version` and requests one status refresh. The
refresh is a generation-tagged `WorkerCommand::Refresh` occupying the mailbox's `refresh` slot
(one more layer of coalescing, described in [the concurrency page](../rendering/concurrency.md)),
executed on the background worker thread as the lock-free status invocation from
[the status section](#how-quinjet-reads-status-without-taking-a-lock).

The reply carries the new branch state. Selection is restored by (path, area) rather than by row
number (`restore_change_selection`, src/app.rs:6716-6743), the checked-path set is pruned to
paths that still exist, and one comparison closes the loop: if the branch head oid changed, the
app reloads history (src/app.rs:3126-3131), which re-runs the `git log` read whose `%D`
decorations re-join refs to commits. The sidebar now shows the new commit, seconds after a
command that ran in a different worktree of a different checkout, and the total cost was one
status process plus one log page: no polling burst, no per-event work, no lock taken anywhere.

Had the same commit happened in a *third* worktree, nothing changes in the analysis: that
worktree's `HEAD` lives under `/main/.git/worktrees/<name>/HEAD`, still inside the common
directory, still covered by the same extra watch, and not filtered (the test pins
`.git/worktrees/topic/gitdir` as not-noise). The one blind spot would be a repository watched
without its common directory, which is exactly why `with_extra` exists.

## Pull requests as fetchable refs

### GitHub's synthetic ref namespace

Refs are the unit of fetch. A fetch negotiation starts from names the server advertises, and
GitHub advertises more names than the branches and tags a repository's owner created: for every
pull request `N` it maintains `refs/pull/N/head`, pointing at the current tip of the PR branch,
updated on every push to the PR. (A companion `refs/pull/N/merge`, holding a test merge against
the base branch, exists for some pull requests; Quinjet does not use it, because the diff it wants
is against the merge base, not against a merge GitHub computed at its own moment.)

These synthetic refs solve a real distribution problem. The PR branch itself lives in the
contributor's fork, a different repository that may be private, renamed, or deleted; the base
repository's maintainers may have no fetch access to it at all. `refs/pull/N/head` re-exports the
contribution through the base repository, so one remote suffices to fetch any PR's code. The refs
are read-only from the client's perspective and are not fetched by default, because the standard
clone refspec maps only `refs/heads/*`; fetching one is an explicit act:

```bash
git fetch origin '+refs/pull/55/head:refs/quinjet/head'
```

### Refspec anatomy

That command line is a refspec, the little language of fetch: `[+]<src>:<dst>`. The source names
a ref on the server; the destination names where to store it locally; the leading `+` permits a
non-fast-forward update of the destination, which matters here because a force-push to a PR
branch moves `refs/pull/N/head` backwards or sideways, and the local mirror must follow
unconditionally. Quinjet constructs exactly two refspec shapes for a PR fetch
(src/git/github/mod.rs:1800-1801):

```rust
let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);
```

The destination namespace `refs/quinjet/*` is a private invention, and it can be one because the
ref namespace is open: any `refs/<something>` hierarchy is legal, and Git tooling ignores
hierarchies it does not know. Landing fetched tips under `refs/quinjet/base`, `refs/quinjet/head`,
and `refs/quinjet/merge-base` means the workspace can never collide with a real branch name, and
since these refs exist only inside a disposable bare repository created for the purpose, even that
is defense in depth. The workspace itself (creation, alternates borrowing, `Drop` cleanup) is
documented in [the PR workspace page](../github/pr-workspace.md); this section follows only its
refs.

### The fetch command

Every workspace fetch goes through one function, `fetch_ref` in src/git/github/mod.rs
(lines 1876-1886):

```rust
fn fetch_ref(temporary: &Path, remote: &str, refspec: &str, depth: usize) -> Result<()> {
    let args = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from("--filter=blob:none"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
```

`--filter=blob:none` (partial clone: commits and trees only, blobs fetched lazily on demand) and
`--depth=N` (shallow: bounded history) are the transfer optimizations, explained end to end in
[the shallow and partial clone page](./shallow-and-partial-clone.md). The ref-level flags are this
page's business: `--no-tags` disables automatic tag following, which would otherwise drag any tag
reachable from the fetched commits, and their history, into a clone that wants exactly one tip;
`--force` restates the refspec's `+` at the command level so a re-fetch after a force-push always
succeeds. When a server does not permit filtered fetches, the same command is retried without the
filter (src/git/github/mod.rs:1892-1901); shallowness is kept either way.

### The choreography, ref by ref

`fetch_pull_request` (src/git/github/mod.rs:1781-1864) turns PR metadata into a merge base and a
head commit using at most a handful of fetches:

**1. Point origin at the base repository.** `git remote add origin <base_repo_url>` in the
disposable repository. Remotes are just configuration; this writes the temp repo's `config`, not
the opened repository's.

**2. Fetch the PR head through the synthetic ref.** The first attempt is
`+refs/pull/{number}/head:refs/quinjet/head` at depth 64 from origin. The synthetic ref is
preferred over the fork branch because it survives fork branch renames and requires no second
remote. Only when that fetch fails does the fallback engage (src/git/github/mod.rs:1806-1831):
add the fork as a second remote called `head` (its URL derived host-preservingly from the base
repository's URL by `repository_url_for_name`) and fetch
`+refs/heads/{head_ref}:refs/quinjet/head` from there. A PR whose base repository does not expose
the ref *and* whose fork is deleted is a hard error with that exact diagnosis: "the base
repository no longer exposes the PR head and its fork was deleted."

**3. Try to fetch the merge base as a single commit.** When the GitHub compare API has already
answered with the merge-base commit id (the hint, cached forever under the immutable OID-pair
key; see [the merge-base page](./merge-bases-and-history.md)), the workspace fetches it directly,
from src/git/github/mod.rs (lines 1835-1843):

```rust
if let Some(hint) = merge_base_hint {
    let hint_refspec = format!("+{hint}:refs/quinjet/merge-base");
    if fetch_ref(temporary, "origin", &hint_refspec, 1).is_ok() {
        let head =
            preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
        if head == pull_request.head_oid {
            return Ok((hint.to_owned(), head));
        }
    }
}
```

Two ref-system facts make this the fastest path. First, the *source* side of that refspec is not
a ref name at all but a raw commit id; fetching by id only works when the server permits wanting
unadvertised objects, a capability GitHub's upload-pack enables for reachable commits. Second,
`--depth=1` on a single commit means the transfer is one commit object plus its trees: no history
from either side of the PR ever crosses the network. When this path succeeds, and the fetched head
still equals the head oid the metadata advertised, the function returns immediately: the base
branch is never fetched at all.

**4. Otherwise, deepen until a merge base appears.** Without a usable hint, the base ref is
fetched at depth 64 and the ladder runs (src/git/github/mod.rs:1848-1860):

```rust
for depth in [64_usize, 256, 1_024, 4_096, 16_384] {
    if depth != 64 {
        fetch_ref(temporary, "origin", &base_refspec, depth)?;
        fetch_ref(temporary, &head_remote, &head_refspec, depth)?;
    }
    let base =
        preferred_fetched_commit(temporary, &pull_request.base_oid, "refs/quinjet/base")?;
    let head =
        preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
    if let Some(merge_base) = try_merge_base(temporary, &base, &head)? {
        return Ok((merge_base, head));
    }
}
bail!(
    "Unable to find the PR merge base within 16,384 commits; refusing an unbounded history fetch"
)
```

A shallow clone cannot compute a merge base older than its history horizon, so each iteration
deepens both sides and asks `git merge-base` again; a non-zero exit from `merge-base` means
"deepen more", not failure (`try_merge_base`, src/git/github/mod.rs:1967-1979). The ceiling of
16,384 commits is the refusal to let one pathological PR turn into an unbounded history download.

`preferred_fetched_commit` (src/git/github/mod.rs:1949-1965) resolves each side with a priority
that is pure ref semantics: verify the exact object id the metadata advertised
(`git rev-parse --verify {oid}^{commit}`), and only fall back to the `refs/quinjet/*` name when
that id is absent. The reason is the race between reading metadata and fetching: a force-push in
that window moves the ref, and diffing whatever the ref *now* points at would silently show a
different PR state than the one whose metadata, counts, and caches were just loaded. Pinning to
the advertised oid makes the whole pipeline consistent, and the immutability of commits makes the
pin safe.

### What flows through the refs afterwards

Once `refs/quinjet/head` and the merge base exist in the workspace, everything downstream is
OID-addressed and cache-friendly: the changed-file index is one
`git diff --name-status -z --find-renames <merge_base> <head> --`, patches are path-scoped diffs
over the same pair, and every cached artifact is keyed by the merge-base and head ids, immutable
by construction (ARCHITECTURE.md, invariant 12). The scheduling of those patch reads evolved
across the optimization stack, and the current shape is worth stating precisely because an older
one is still described in older PR discussions: PR #50 introduced size-tiered prefetch that read
the smallest files first on huge pull requests; PR #55 replaced that ordering with
viewport-anchored streaming, which walks the file index starting from the first file visible in
the Files tree and wraps around, batching up to 32 files per Git invocation under a 6 MiB
estimated-byte budget (80 bytes per line of counted change plus 4096 per file, 512 KiB for a file
with unknown counts), stopping after 4,096 prefetched files. Smallest-first is the documented
evolution step; viewport-anchored wrap-around is the current behavior. The full treatment is in
[the prefetch page](../github/prefetch.md) and
[the progressive loading page](../rendering/progressive-loading.md).

### The local fast path: no refs needed at all

Before any of the workspace machinery runs, `prepare_pull_request_diff`
(src/git/github/mod.rs:767-822) checks whether the opened repository already contains both ends:

```rust
let (repository, merge_base, head, api_counts) =
    if self.has_commit(&pull_request.base_oid) && self.has_commit(&pull_request.head_oid) {
        progress(PullRequestProgress::FindingMergeBase);
        (
            PreparedRepository::Opened(self.root().to_path_buf()),
            self.merge_base(&pull_request.base_oid, &pull_request.head_oid)?,
            pull_request.head_oid.clone(),
            None,
        )
    } else {
```

`has_commit` (src/git/mod.rs:790-799) accepts only full object ids and probes with
`git cat-file -e <oid>^{commit}`, a pure existence check that prints nothing. When the PR is the
user's own branch, both commits are already local, and the entire preview is served from the
opened repository with zero network: one `merge-base`, one file listing, path-scoped diffs. This
is the first clause of invariant 9 ("PR patches first use immutable base/head OIDs already
present in the opened repository, which makes local-branch PR previews network-free"), and note
what it does *not* do: it does not create any ref, not even a temporary one. Raw ids are enough
for every read Git offers, so the fast path leaves the opened repository's ref store untouched.

## The no-mutation guarantee

Invariant 9 ends with a sentence that this page has been circling the whole time: "The opened
repository receives no checkout, branch, ref, index, or worktree mutation." This is a checkable
property, so here is the check, category by category, against every Git invocation Quinjet's
pull-request machinery issues.

| Mutation class | Would look like | What the PR path actually runs |
| --- | --- | --- |
| Checkout | `switch`, `checkout`, `restore` in the opened repo | Nothing; diffs are read by id, no working tree is involved |
| Branch | `branch`, `switch --create` | Nothing; `refs/quinjet/*` exists only in the disposable repo |
| Ref | `update-ref`, `fetch` into the opened repo | Fetches run with `-C <temp>`, never `-C <root>` |
| Index | `add`, `reset`, `stash` | Nothing; reads run with `GIT_OPTIONAL_LOCKS=0`, so even `status` never writes |
| Worktree | `worktree add/remove/prune` | Nothing; `worktree list --porcelain -z` is the only worktree verb spawned |

The opened repository participates in PR preparation in exactly three read-only ways: `cat-file
-e` existence probes, `merge-base` and diff reads on the local fast path, and lending its objects
directory through the *disposable* repository's `objects/info/alternates` file
(`borrow_local_objects`, src/git/github/mod.rs:1732-1745), which is a write into the borrower,
never the lender. Everything that must be written lands in one of two places designed to absorb
writes: the temporary bare repository under the cache root (deleted on `Drop`, with a sweeper for
leaked ones) and the on-disk cache described in [the caching page](../github/caching.md).

Why a documentation page in an optimization section cares: the guarantee is what makes several
optimizations sound rather than merely fast.

- The watcher stays quiet. If PR preparation fetched into the opened repository, every PR view
  would trigger object and ref events in the watched tree, refreshing status for no user-visible
  reason. Because all PR writes land outside the watched roots, browsing pull requests generates
  zero watcher traffic.
- Caches keyed by the opened repository's state cannot be invalidated by Quinjet itself. The
  status snapshot, branch list, and history pages change only when the user or an external
  process acts.
- The user's mental model holds. `git status`, `git branch`, `git stash list` in a terminal
  beside Quinjet show exactly what they would have shown had Quinjet never run. A viewer that
  leaves droppings in `refs/` or the index is a viewer people stop trusting with production
  checkouts.

User-invoked mutations are the deliberate exception, and they are fenced differently: each
`GitOperation` is explicit, labeled, serialized by the app's busy flag so at most one runs at a
time, and routed through the ordered `operations` queue rather than the coalescing slots
(src/git/worker.rs:228-267). The boundary is intent: the user's mutations happen because the user
asked; the machinery's never happen at all.

## Failure modes and edge cases

The mutable layer is where repositories get weird: crashed writers, unborn branches, deleted
checkouts, filesystems with coarse clocks. This section collects the cases that shaped the code,
each with the general failure and the specific Quinjet defense.

**A crashed writer leaves a stale index.lock.** Only a successful writer renames the lock away,
so a killed `git commit` strands `.git/index.lock` forever. For Quinjet's reads this is a
non-event: `GIT_OPTIONAL_LOCKS=0` status never wants the lock, so browsing, previews, and history
keep working in a repository other tools refuse to touch. A user-invoked mutation does want it,
fails, and surfaces Git's own stderr through `command_error` (src/git/mod.rs:1517-1526, preferring
trimmed stderr over stdout over a generic exit-status message), which is the right outcome: the
message names the lock file, and removing it is a human decision because only a human knows
whether another process is still alive.

**The watcher cannot start.** Inotify watch limits run out; network filesystems lie about events;
containers mount strange things. `RepoWatcher::with_extra(...).ok()` (src/main.rs:104) converts
every such failure into `None`, and the session runs without a watcher: the 10-second heartbeat
still polls status, and every user mutation still refreshes immediately on completion. Liveness
degrades from seconds to the heartbeat interval; nothing else changes. The same posture applies
one level down, where a failure to add the *extra* watch is individually swallowed
(`drop(watcher.watch(extra, ...))`), keeping the root watch alive even when the common directory
is unwatchable.

**The common directory disappears.** Deleting the main checkout out from under a linked worktree
orphans it; `git rev-parse --git-common-dir` may fail or point at nothing. Quinjet threads
`Option` through the whole path: `git_common_dir().ok()` at startup, then
`extra.filter(|path| path.exists() && !path.starts_with(root))` before watching. The session
opens, the extra watch is skipped, and the recent-projects loader has its own fallback: when the
recorded worktree path no longer opens, it retries discovery from the recorded common directory
(src/state.rs:90-94), and entries that fail both ways are silently skipped rather than shown as
dead rows.

**Unborn branches break HEAD-relative commands.** In a fresh `git init`, `HEAD` names a branch
with no commit, so `git restore --staged --source=HEAD` and `git reset HEAD` have nothing to
restore from. Quinjet probes with `has_head` (`git rev-parse --verify HEAD`) and switches
strategies: `git rm --cached --ignore-unmatch -- <paths>` to unstage individual paths and
`git rm --recursive --cached .` to unstage everything, the latter tolerating a non-zero exit as
long as a follow-up status shows nothing staged (src/git/mod.rs:1199-1218). The status parser
meanwhile reports the branch normally, because porcelain v2 marks the state explicitly with
`(initial)` instead of failing.

**Racy-clean files cost more under a read-only poll.** The write-time half of the racy-git fix
(smudging) only happens when someone writes the index. Quinjet never does, so a file modified in
the same timestamp granule as the last index write is re-hashed by every poll until the user's
next Git command persists refreshed stat data. On nanosecond-timestamp filesystems the window is
vanishingly small; on coarse ones it is one file's content hash per poll in the worst case. The
trade was accepted with eyes open: the alternative is a background process that takes locks and
writes the index, and the whole previous section is the argument against that.

**Paths are bytes, not strings.** A path can be any byte sequence except NUL and `/` semantics,
including invalid UTF-8. The pipeline is byte-clean end to end (`-z` records,
`core.quotepath=false`, prefix parsing on byte slices) until the final display step, where
`bytes_to_path` (src/git/status.rs:284-286) and `text` (src/git/mod.rs:1576-1578) apply
`String::from_utf8_lossy`: invalid sequences degrade to replacement characters on screen rather
than failing a parse. Full non-UTF-8 path preservation through display and back into argv is
explicitly listed in ARCHITECTURE.md's Deliberate Next Steps, which is the honest way to hold an
edge case a design has not finished absorbing.

**Truncated listings must still parse as whole records.** When a `--name-status` listing crosses
its 8 MiB cap, the child is killed mid-record. The repair is format-aware: NUL-record output is
cut back to the byte after the last NUL (src/git/mod.rs:525-531), so the parser sees only
complete records, and the `truncated` flag rides the index into the UI, where the header count
stays honest instead of pretending the listing was complete. The 16,384-entry cap ends parsing
the same way: `truncated = true`, stop, render what exists. A rename record needing two more
path records that never arrived also sets the flag rather than mis-pairing paths
(src/git/mod.rs:539-542 and the walk in `diff_index_files`).

**Reflogs can contain garbage selectors.** A corrupted or hand-edited stash reflog can emit
subjects and selectors that do not match the expected shapes. Listing skips any record whose
selector fails `valid_stash_reference` (src/git/mod.rs:892-895), and every operation re-validates
before building an argv, so the worst a bogus entry can do is not appear. The same defensive
posture covers `origin/HEAD`: it is a symref, not a branch, and the `%(symref)` filter drops it
rather than letting a pseudo-branch row invite operations that would fail.

**Worktree listings have optional everything.** A worktree can be bare (no working tree at all),
detached (no branch record), locked with an empty reason, or prunable with an empty reason; paths
come back with forward slashes on Windows; the session's own root may be a symlink to the listed
path. The parser handles each without special cases at the call site: booleans for `bare` and
`detached`, `Option<String>` for both reasons with `Some("")` meaning "flagged, no reason given",
separator rewriting in `parse_worktree_path`, and canonicalize-fallback comparison in
`same_path`. The pinned porcelain fixture test covers a locked worktree, a detached prunable one,
and the session-root match in a single byte string.

**The deepening ladder can exhaust its ceiling.** A pull request whose merge base sits more than
16,384 commits behind either tip (or a criss-cross history whose bases keep receding) ends with
the explicit bail: "Unable to find the PR merge base within 16,384 commits; refusing an unbounded
history fetch". This is a designed failure. The alternative, fetching unbounded history into a
temp repository on a background thread, is strictly worse than telling the user this one PR needs
a full local clone. In practice the ladder rarely runs at all, because the compare-API hint turns
the whole problem into one depth-1 fetch; the ladder is the fallback for a failed or missing
hint, and its bound is what keeps the fallback safe.

## Design alternatives that lost

Every mechanism above displaced an alternative someone would reasonably reach for first. Recording
why they lost is as useful as documenting what won.

**Linking a Git library instead of spawning processes.** An in-process libgit2 or gitoxide would
save process spawns and parsing. It lost on authority and isolation. Authority: Git's behavior
around hooks, config stacking, `check-ref-format` rules, index extensions, and ref-backend
evolution (loose, packed, reftable) is the ground truth users' other tools follow, and the
research notes in ARCHITECTURE.md name "Git CLI delegation for hooks, credentials, config, refs,
and index semantics" as an adopted parity finding. A library tracks that truth with a lag and a
second implementation's bugs; the CLI *is* the truth. Isolation: a subprocess can be killed the
instant its output crosses a cap, so an 8 MiB budget bounds not just memory but the work Git does
producing a pathological diff; an in-process call cannot be abandoned mid-computation. The spawn
cost is real but bounded and paid off the render thread; the batching design elsewhere in this
section exists precisely to amortize it.

**Reading .git files directly.** Parsing `packed-refs`, loose refs, and even the index in Rust is
tempting; the formats are documented and stable. It lost to the concurrency section of this page:
correct lock-free reading of the loose/packed pair requires the stat-and-retry dance Git
implements internally, the index adds versioned entries plus a dozen extensions, and the reftable
migration would have been a rewrite. One `for-each-ref` process per refresh, with sorting done by
Git, is fast enough that correctness was never worth trading for it.

**Leaving optional locks on.** The refreshed index that a default `git status` writes back is a
genuine optimization, and giving it up means Quinjet's polls repeat refresh work forever. Keeping
it lost the moment background polling entered the design: a poll that takes `index.lock` even
briefly will eventually collide with the user's own commands, and a tool whose presence makes
`git commit` fail intermittently is broken in the way users least forgive. The collision is not
rate-limitable to zero; only the lock-free mode makes it structurally impossible.

**Watching fewer places, or more.** Watching only the working tree misses every ref update in a
linked worktree; watching only `.git` misses every edit the user makes; watching each interesting
file individually (`HEAD`, `index`, `refs/`) breaks silently when Git renames files into place,
because renames replace the watched inode on some platforms. Two recursive watches, root plus
common directory when distinct, with a byte-cheap noise filter, is the smallest set that observes
everything and survives Git's write patterns.

**Reacting to events with event-shaped work.** A watcher that maps "src/foo.rs changed" to
"re-diff src/foo.rs" promises efficiency and delivers staleness bugs: renames, directory moves,
and lock-protected multi-file updates all decompose into event sequences whose intermediate
states must not be rendered. Quinjet's channel deliberately erases the payload: every signal
means "re-read everything cheap", and the cheap reads (status, ref listings) are designed to be
cheap precisely so this erasure is affordable. Invariant 4's phrasing ("one full status snapshot
subsumes all preceding file events") is the contract.

**Diffing against refs/pull/N/merge.** GitHub also publishes a merge ref that would seem to save
the merge-base work entirely. It lost on availability and meaning: the merge ref exists only
while GitHub considers the PR mergeable and is refreshed on GitHub's schedule, not the reader's,
so a diff against it is a diff against an artifact of unknown freshness. The PR diff Quinjet
wants is the three-dot diff against the merge base, resolved through the compare API and pinned
by immutable ids; [the merge-base page](./merge-bases-and-history.md) covers why that is the
right question.

**Fetching PR refs into the opened repository.** Several Git tools materialize `pr/N` branches in
the user's repository. It lost to the no-mutation guarantee: fetched refs and their objects would
churn the watched `.git`, collide with user branch names, survive as clutter after the PR closes,
and turn a viewer into a writer. The disposable bare workspace costs one `init --bare` and a
config file, gets the opened repository's objects for free through alternates, and deletes
itself; the opened repository never knows it was consulted.

## Related pages

- [Git internals overview](./README.md): how this page fits the group.
- [The object model](./object-model.md): the immutable store the refs point into.
- [Packfiles and deltas](./packfiles-and-deltas.md): what actually crosses the wire on fetch.
- [Shallow and partial clone](./shallow-and-partial-clone.md): `--depth`, `--filter=blob:none`,
  and the protocol underneath the workspace fetches.
- [Plumbing and porcelain](./plumbing-and-porcelain.md): the full catalog of Git invocations and
  their byte-oriented parsers.
- [Merge bases and history](./merge-bases-and-history.md): why the PR diff is `base...head` and
  how the compare API resolves it.
- [The PR workspace](../github/pr-workspace.md): the disposable bare repository's lifecycle.
- [Prefetch](../github/prefetch.md): the viewport-anchored batch scheduling that fills a PR's
  patches after the refs land.
- [Caching](../github/caching.md): immutable keys versus TTLs for everything read through refs.
- [Concurrency](../rendering/concurrency.md): lanes, mailboxes, and the generation tags that
  keep stale replies off the screen.
- [Optimization techniques](../techniques.md): the whole section's pattern catalog.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record time to first useful rows |
| 2 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record steady frame cost |
| 3 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record Git and gh process count |
| 5 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record stale reply rejection |
| 8 | Check latency for Refs, the Index, and Worktrees in a small local repository | Record visible state after failure |
| 9 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Refs, the Index, and Worktrees in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for Refs, the Index, and Worktrees in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record time to first useful rows |
| 34 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record steady frame cost |
| 35 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record Git and gh process count |
| 37 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record maximum retained document bytes |
| 38 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record cache disposition and complete key |
| 39 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record stale reply rejection |
| 40 | Check latency for Refs, the Index, and Worktrees in an unavailable network | Record visible state after failure |
| 41 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record time to first useful rows |
| 42 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record steady frame cost |
| 43 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record bytes accepted from child output |
| 44 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record Git and gh process count |
| 45 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record cache disposition and complete key |
| 47 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record stale reply rejection |
| 48 | Check latency for Refs, the Index, and Worktrees in rapid keyboard navigation | Record visible state after failure |
| 49 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record time to first useful rows |
| 50 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record steady frame cost |
| 51 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record bytes accepted from child output |
| 52 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record Git and gh process count |
| 53 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record maximum retained document bytes |
| 54 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record cache disposition and complete key |
| 55 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record stale reply rejection |
| 56 | Check latency for Refs, the Index, and Worktrees in a linked worktree | Record visible state after failure |
| 57 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record time to first useful rows |
| 58 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record steady frame cost |
| 59 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record bytes accepted from child output |
| 60 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record Git and gh process count |
| 61 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record maximum retained document bytes |
| 62 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record cache disposition and complete key |
| 63 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record stale reply rejection |
| 64 | Check latency for Refs, the Index, and Worktrees in cold and warm cache states | Record visible state after failure |
| 65 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record time to first useful rows |
| 66 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record steady frame cost |
| 67 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record bytes accepted from child output |
| 68 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record Git and gh process count |
| 69 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record maximum retained document bytes |
| 70 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record cache disposition and complete key |
| 71 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record stale reply rejection |
| 72 | Check peak memory for Refs, the Index, and Worktrees in a small local repository | Record visible state after failure |
| 73 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record steady frame cost |
| 75 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record bytes accepted from child output |
| 76 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record Git and gh process count |
| 77 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record cache disposition and complete key |
| 79 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record stale reply rejection |
| 80 | Check peak memory for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record visible state after failure |
| 81 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record time to first useful rows |
| 82 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record steady frame cost |
| 83 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record bytes accepted from child output |
| 84 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record Git and gh process count |
| 85 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record maximum retained document bytes |
| 86 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record cache disposition and complete key |
| 87 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record stale reply rejection |
| 88 | Check peak memory for Refs, the Index, and Worktrees in a pull request containing generated files | Record visible state after failure |
| 89 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record time to first useful rows |
| 90 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record steady frame cost |
| 91 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record bytes accepted from child output |
| 92 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record Git and gh process count |
| 93 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record cache disposition and complete key |
| 95 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record stale reply rejection |
| 96 | Check peak memory for Refs, the Index, and Worktrees in a deeply diverged branch | Record visible state after failure |
| 97 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record time to first useful rows |
| 98 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record steady frame cost |
| 99 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record bytes accepted from child output |
| 100 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record Git and gh process count |
| 101 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record maximum retained document bytes |
| 102 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record cache disposition and complete key |
| 103 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record stale reply rejection |
| 104 | Check peak memory for Refs, the Index, and Worktrees in an unavailable network | Record visible state after failure |
| 105 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record time to first useful rows |
| 106 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record steady frame cost |
| 107 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record bytes accepted from child output |
| 108 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record Git and gh process count |
| 109 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record cache disposition and complete key |
| 111 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record stale reply rejection |
| 112 | Check peak memory for Refs, the Index, and Worktrees in rapid keyboard navigation | Record visible state after failure |
| 113 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record time to first useful rows |
| 114 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record steady frame cost |
| 115 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record bytes accepted from child output |
| 116 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record Git and gh process count |
| 117 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record maximum retained document bytes |
| 118 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record cache disposition and complete key |
| 119 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record stale reply rejection |
| 120 | Check peak memory for Refs, the Index, and Worktrees in a linked worktree | Record visible state after failure |
| 121 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record time to first useful rows |
| 122 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record steady frame cost |
| 123 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record bytes accepted from child output |
| 124 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record Git and gh process count |
| 125 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record maximum retained document bytes |
| 126 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record cache disposition and complete key |
| 127 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record stale reply rejection |
| 128 | Check peak memory for Refs, the Index, and Worktrees in cold and warm cache states | Record visible state after failure |
| 129 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record time to first useful rows |
| 130 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record steady frame cost |
| 131 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record bytes accepted from child output |
| 132 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record Git and gh process count |
| 133 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record maximum retained document bytes |
| 134 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record cache disposition and complete key |
| 135 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record stale reply rejection |
| 136 | Check network transfer for Refs, the Index, and Worktrees in a small local repository | Record visible state after failure |
| 137 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record steady frame cost |
| 139 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record bytes accepted from child output |
| 140 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record Git and gh process count |
| 141 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record cache disposition and complete key |
| 143 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record stale reply rejection |
| 144 | Check network transfer for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record visible state after failure |
| 145 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record time to first useful rows |
| 146 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record steady frame cost |
| 147 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record bytes accepted from child output |
| 148 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record Git and gh process count |
| 149 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record maximum retained document bytes |
| 150 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record cache disposition and complete key |
| 151 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record stale reply rejection |
| 152 | Check network transfer for Refs, the Index, and Worktrees in a pull request containing generated files | Record visible state after failure |
| 153 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record time to first useful rows |
| 154 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record steady frame cost |
| 155 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record bytes accepted from child output |
| 156 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record Git and gh process count |
| 157 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record cache disposition and complete key |
| 159 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record stale reply rejection |
| 160 | Check network transfer for Refs, the Index, and Worktrees in a deeply diverged branch | Record visible state after failure |
| 161 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record time to first useful rows |
| 162 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record steady frame cost |
| 163 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record bytes accepted from child output |
| 164 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record Git and gh process count |
| 165 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record maximum retained document bytes |
| 166 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record cache disposition and complete key |
| 167 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record stale reply rejection |
| 168 | Check network transfer for Refs, the Index, and Worktrees in an unavailable network | Record visible state after failure |
| 169 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record time to first useful rows |
| 170 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record steady frame cost |
| 171 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record bytes accepted from child output |
| 172 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record Git and gh process count |
| 173 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record cache disposition and complete key |
| 175 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record stale reply rejection |
| 176 | Check network transfer for Refs, the Index, and Worktrees in rapid keyboard navigation | Record visible state after failure |
| 177 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record time to first useful rows |
| 178 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record steady frame cost |
| 179 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record bytes accepted from child output |
| 180 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record Git and gh process count |
| 181 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record maximum retained document bytes |
| 182 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record cache disposition and complete key |
| 183 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record stale reply rejection |
| 184 | Check network transfer for Refs, the Index, and Worktrees in a linked worktree | Record visible state after failure |
| 185 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record time to first useful rows |
| 186 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record steady frame cost |
| 187 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record bytes accepted from child output |
| 188 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record Git and gh process count |
| 189 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record maximum retained document bytes |
| 190 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record cache disposition and complete key |
| 191 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record stale reply rejection |
| 192 | Check network transfer for Refs, the Index, and Worktrees in cold and warm cache states | Record visible state after failure |
| 193 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record time to first useful rows |
| 194 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record steady frame cost |
| 195 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record bytes accepted from child output |
| 196 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record Git and gh process count |
| 197 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record maximum retained document bytes |
| 198 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record cache disposition and complete key |
| 199 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record stale reply rejection |
| 200 | Check subprocess count for Refs, the Index, and Worktrees in a small local repository | Record visible state after failure |
| 201 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record steady frame cost |
| 203 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record bytes accepted from child output |
| 204 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record Git and gh process count |
| 205 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record cache disposition and complete key |
| 207 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record stale reply rejection |
| 208 | Check subprocess count for Refs, the Index, and Worktrees in a monorepo with many changed paths | Record visible state after failure |
| 209 | Check subprocess count for Refs, the Index, and Worktrees in a pull request containing generated files | Record time to first useful rows |
| 210 | Check subprocess count for Refs, the Index, and Worktrees in a pull request containing generated files | Record steady frame cost |
| 211 | Check subprocess count for Refs, the Index, and Worktrees in a pull request containing generated files | Record bytes accepted from child output |
| 212 | Check subprocess count for Refs, the Index, and Worktrees in a pull request containing generated files | Record Git and gh process count |
| 213 | Check subprocess count for Refs, the Index, and Worktrees in a pull request containing generated files | Record maximum retained document bytes |
