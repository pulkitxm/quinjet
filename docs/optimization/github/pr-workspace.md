# The Pull-Request Workspace

This page documents `PreparedPullRequest`, the handle behind every pull-request diff Quinjet
shows: how a PR is reduced to two immutable commit IDs, how the network-free fast path decides
whether the opened repository can answer alone, how the disposable bare workspace is created,
fed by shallow blob-less fetches, pointed at the local object store through an alternates file,
and torn down again, and how session ownership makes one preparation pay for every file the
reader opens afterwards. Everything here lives in `src/git/github/mod.rs` with its consumers in
`src/cli/command.rs`, `src/git/worker.rs`, and `src/app.rs`.

## Contents

- [Why a pull request needs its own workspace](#why-a-pull-request-needs-its-own-workspace)
- [Two commits name the whole diff](#two-commits-name-the-whole-diff)
- [The PreparedPullRequest handle](#the-preparedpullrequest-handle)
- [The Opened fast path](#the-opened-fast-path)
- [The disposable bare workspace](#the-disposable-bare-workspace)
- [Borrowing objects through alternates](#borrowing-objects-through-alternates)
- [The fetch ladder](#the-fetch-ladder)
- [The merge base through the compare API](#the-merge-base-through-the-compare-api)
- [Enumerating changed files](#enumerating-changed-files)
- [Path-scoped diffs](#path-scoped-diffs)
- [Staying alive between selections](#staying-alive-between-selections)
- [Session ownership](#session-ownership)
- [Lifecycle and cleanup](#lifecycle-and-cleanup)
- [Design evolution and alternatives](#design-evolution-and-alternatives)
- [Measured behavior on a million-line pull request](#measured-behavior-on-a-million-line-pull-request)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Reference tables](#reference-tables)
- [Related pages](#related-pages)

## Why a pull request needs its own workspace

A local diff is easy: both sides of the comparison already live in the opened repository, so a
`git diff` walks trees that are on disk. A pull-request diff is structurally harder for three
reasons, and each one shapes the workspace design.

**1. The commits may not exist locally.** A PR head usually lives on a branch you never
fetched, often in a fork you have no remote for. Even a fully up-to-date clone of the base
repository can lack the head: when a project squash-merges, the PR's actual head commit never
becomes reachable from any branch, only from GitHub's synthetic `refs/pull/N/head`. The
workspace must therefore be able to materialize commits on demand, and must be able to tell,
cheaply and exactly, when it does not need to.

**2. The diff base is not the base branch tip.** GitHub renders a pull request as the
three-dot diff `base...head`: the changes the head branch introduces relative to the merge
base, the last commit the two branches share. Computing that merge base requires enough shared
history on both sides, which a naive shallow fetch does not provide. The workspace has to
obtain a merge base without downloading an unbounded amount of history.

**3. The opened repository is off limits.** Quinjet's architecture contract (invariant 9 in
`ARCHITECTURE.md`) ends with a hard guarantee: "The opened repository receives no checkout,
branch, ref, index, or worktree mutation." Fetching PR refs into the user's clone would
pollute their ref namespace, grow their object store, and race with their own Git usage. So
everything a PR needs that is not already local must land somewhere else, and that somewhere
must be cheap to create, safe to delete, and invisible to the user's repository.

The answer to all three is one object: a `PreparedPullRequest` that either wraps the opened
repository directly (when both PR commits are already present) or wraps a disposable bare
repository under Quinjet's cache root, populated by shallow, blob-less, depth-limited fetches
and granted read access to the opened repository's objects through an alternates file. The
handle stays alive while the reader browses files, answers every per-file and batched diff
with a path-scoped `git diff`, and removes its on-disk workspace in `Drop`.

### What the baseline got wrong

The workspace did not start in this shape. Before the 2026-08-20 optimization stack (PRs #46
through #50, then #52, #54, #55) landed, the same module had the same skeleton with different
economics, and the benchmark PR that drove the rework, oven-sh/bun#30412 "Rewrite Bun in Rust"
(2,188 changed files, +1,009,257 added lines), exposed each weakness in turn:

- The merge base was found only by a deepening fetch ladder capped at 4,096 commits. Past that
  the whole PR load hard-failed with "Unable to find the PR merge base within 4,096 commits",
  after up to eight progressively deeper fetches had already been paid for. Long-lived rewrite
  branches on active repositories routinely exceed that divergence.
- Per-file line counts came from `git diff --numstat` run inside the blob-less workspace. A
  `--numstat` must inflate both blob versions of every changed file to count lines, and in a
  partial clone that means lazily downloading essentially every changed blob in one
  uninterruptible Git invocation while the UI sat at "Enumerating changed files". This was the
  dominant cold-load cost.
- A full local clone did not help a squash-merged PR: the head commit existed only on GitHub's
  PR ref, both OIDs were not locally present, and the workspace fell back to per-batch lazy
  blob downloads even though every file's content sat on local disk under other refs.

The current code fixes the first with API merge-base resolution plus a ladder extended to
16,384 as fallback (PR #47), the second by reading counts from the pull-request files endpoint
(PR #49), and the third by borrowing the opened repository's objects through
`objects/info/alternates` (PR #55). The rest of this page walks the machinery in its current
form and marks each of those evolution steps where it applies.

## Two commits name the whole diff

Everything the workspace does rests on one property of Git's object model: an object ID is a
cryptographic hash of the object's content, so a commit OID names one exact commit forever.
Two OIDs therefore name one exact diff forever. Quinjet leans on this in three distinct ways
before a single byte is fetched.

### Where the OIDs come from

PR metadata arrives from one `gh pr view` invocation (`pull_request_view_args`,
`src/git/github/mod.rs:1421-1433`) whose `--json` field list includes `baseRefOid` and
`headRefOid` alongside the ref names, counts, and repository identity:

```text
gh pr view <number> --repo <base repo url>
  --json number,title,body,author,state,isDraft,createdAt,updatedAt,url,
         baseRefName,baseRefOid,headRefName,headRefOid,headRepository,
         isCrossRepository,additions,deletions,changedFiles
  --jq <18-field @tsv template>
```

The parsed `PullRequest` (`src/git/github/mod.rs:93-116`) carries `base_oid` and `head_oid` as
plain strings. They are validated wherever they matter by `is_commit_oid`
(`src/git/github/mod.rs:1945-1947`): exactly 40 or 64 ASCII hex characters, the two lengths a
SHA-1 or SHA-256 object name can have. Anything else is treated as absent, never passed to a
subprocess.

### The metadata record, byte for byte

The `--jq` template flattens the JSON reply into one 18-field `@tsv` record
(`PULL_REQUEST_TSV_FIELDS = 18`), and the workspace's inputs are fields of that record, so
its exact shape is worth pinning. The TSV field order as destructured by
`parse_pull_request_fields` (`src/git/github/mod.rs:1456-1508`):

| Position | Field | Workspace relevance |
| --- | --- | --- |
| 1 | number | PR identity, cache keys |
| 2 | title | document headers (bounded to 16 KiB) |
| 3 | description | overview only (bounded to 256 KiB) |
| 4 | author | document headers |
| 5 | state | uppercased; settled states stop polling |
| 6 | draft | display |
| 7 | updated_at | conversation cache stamp |
| 8 | url | repository identity in cache keys |
| 9 | base_ref | the base branch refspec |
| 10 | head_ref | the fork-fallback refspec |
| 11 | head_repository_name | fork detection and URL derivation |
| 12 | cross_repository | display |
| 13 | additions | huge-PR heuristics, totals |
| 14 | deletions | huge-PR heuristics, totals |
| 15 | changed_files | honest `total_files` under truncation |
| 16 | base_oid | the fast-path probe and every cache key |
| 17 | head_oid | the fast-path probe and every cache key |
| 18 | created_at | display |

The `@tsv` filter escapes tabs, newlines, carriage returns, and backslashes inside field
values; `unescape_tsv` (`src/git/github/mod.rs:1534-1556`) reverses exactly those four, and
`parse_tsv_record` arity-checks the split ("expected N tab-separated fields, received M"), so
a PR title containing a literal tab cannot shift `base_oid` into the wrong column and send a
title fragment to `git fetch`. Title and description pass through `bounded_text`
(`src/git/github/mod.rs:1510-1519`), which cuts at a char boundary and appends an ellipsis
character, so a hostile 10 MB description costs 256 KiB of memory and nothing downstream.
TSV rather than JSON is a deliberate hot-path choice: the record parses with byte splits, and
a cached entry on disk remains directly readable.

### Why OIDs beat ref names

A ref name is a moving pointer; an OID is a value. Between the moment metadata is read and the
moment a fetch completes, the head branch can gain commits or be force-pushed. Quinjet
resolves this race in favor of the snapshot: `preferred_fetched_commit`
(`src/git/github/mod.rs:1949-1965`) always prefers the exact OID the metadata advertised over
whatever the fetched ref tip happens to be now, so the diff on screen describes the PR state
the metadata described, not a mixture of two moments. The same property makes every derived
artifact cacheable forever: a changed-file listing keyed by `(merge_base, head)`, a patch
keyed by `(merge_base, head, path)`, and a merge base keyed by `(base_oid, head_oid)` can
never become wrong, only evicted (invariant 12; the details live in [caching](./caching.md)).

### Two-dot versus three-dot

The general theory, briefly, because it decides what the workspace must fetch. For commits
`A` and `B`:

- `git diff A..B` (or plain `git diff A B`) compares the two trees directly. Used on a PR it
  would blame the head branch for every change that landed on the base branch since the
  branches diverged, which is exactly the confusing diff review tools learned not to show.
- `git diff A...B` compares `merge-base(A, B)` against `B`: only what the head branch itself
  introduces. This is what GitHub renders and what reviewers mean by "the PR diff".

Quinjet computes the merge base explicitly and then always runs the two-argument form against
it, `git diff <merge_base> <head>`, which is equivalent to the three-dot form but lets one
resolved merge base be reused across the file index, the numstat pass, and every patch read.
The full merge-base theory, including multiple merge bases and criss-cross histories, is in
[merge bases and history](../git-internals/merge-bases-and-history.md).

### The consequence: preparation is a pure function of two OIDs

Because the merge base is itself a commit OID derived deterministically from `base_oid` and
`head_oid`, the entire prepared state reduces to a value: `(merge_base, head)` plus a file
index derived from them. That is why `PreparedPullRequest` can be dropped and rebuilt at will
(the one-shot CLI does this on every invocation), why two Quinjet processes can prepare the
same PR concurrently without coordinating, and why the on-disk caches need no invalidation
protocol. A new push produces a new `head_oid`, which asks a different question rather than
aging an old answer.

## The PreparedPullRequest handle

The handle itself is small. From `src/git/github/mod.rs:371-391`:

```rust
enum PreparedRepository {
    Opened(PathBuf),
    Temporary(TemporaryBareRepository),
}

impl PreparedRepository {
    fn path(&self) -> &Path {
        match self {
            Self::Opened(path) => path,
            Self::Temporary(repository) => &repository.path,
        }
    }
}

pub(crate) struct PreparedPullRequest {
    repository: PreparedRepository,
    pull_request: PullRequest,
    merge_base: String,
    head: String,
    index: PullRequestDiffIndex,
}
```

Five fields, each with a precise job:

- `repository` is the polymorphism at the heart of the design. `Opened(PathBuf)` points at the
  root of the repository the user opened; `Temporary` owns a `TemporaryBareRepository` whose
  `Drop` deletes its directory. Every downstream diff runs against `repository.path()` and is
  identical code either way; the only difference between the two variants is where the objects
  come from and who cleans up.
- `pull_request` is a clone of the metadata snapshot, kept so each generated document can
  carry full `PullRequestDetails` (title, author, state, labels for the header) without a
  second lookup.
- `merge_base` and `head` are the two resolved OIDs that name the diff. They are fixed at
  preparation time and never re-resolved, which is what makes the per-file patch cache keys
  stable for the lifetime of the handle and beyond.
- `index` is the `PullRequestDiffIndex` (`src/git/github/mod.rs:198-204`): the bounded list of
  changed files with statuses and counts, `total_files`, and a `truncated` flag. `index()`
  returns a clone so the caller's copy is independent of the handle's lifetime.

The handle exposes exactly three operations: `index()`, `diff_file(path)`, and
`diff_files(paths)`. There is no "refetch" and no "update": a changed PR is a different pair
of OIDs and therefore a different handle, produced by running preparation again.

### The preparation entry point

`Repository::prepare_pull_request_diff` (`src/git/github/mod.rs:767-822`) is the single
constructor. Its body is the whole strategy in fourteen lines of decision:

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
        progress(PullRequestProgress::PreparingRepository);
        let merge_base_hint = self.merge_base_from_api(pull_request);
        let api_counts = self.pull_request_file_counts_from_api(pull_request);
        let temporary = TemporaryBareRepository::new()?;
        temporary.borrow_local_objects(self);
        let (merge_base, head) = fetch_pull_request(
            &temporary.path,
            pull_request,
            merge_base_hint.as_deref(),
            &mut progress,
        )?;
        (
            PreparedRepository::Temporary(temporary),
            merge_base,
            head,
            api_counts,
        )
    };
```

Both arms converge on the same tail: emit `EnumeratingFiles`, run
`changed_files_in_repository` over `(merge_base, head)` with whatever counts source the arm
chose, and assemble the handle. The `api_counts` value is the tell for which world you are in:
`None` on the Opened path (a local `--numstat` is cheap when blobs are on disk), `Some` on the
Temporary path (a local `--numstat` in a blob-less clone would be a download storm; see
[enumerating changed files](#enumerating-changed-files)).

### Progress phases

Preparation can take seconds on a cold fork PR, so it reports coarse phases through a callback
rather than running silently. `PullRequestProgress` (`src/git/github/mod.rs:237-269`) is a
fixed six-step scale:

| Phase | Percent | Label |
| --- | --- | --- |
| `LoadingMetadata` | 10 | Fetching pull-request metadata |
| `PreparingRepository` | 20 | Preparing an isolated diff workspace |
| `FetchingBase` | 35 | Fetching the destination commit |
| `FetchingHead` | 50 | Fetching the source commit |
| `FindingMergeBase` | 65 | Finding the merge base |
| `EnumeratingFiles` | 90 | Enumerating changed files |

The terminal renders these into its loading indicator; the CLI relabels its stderr spinner
with each phase (`Emitter::execute` in `src/cli/mod.rs` wires
`session.execute_with(command, &mut |event| self.set_progress(event.label()), &|| true)`).
Note the ordering quirk: the fast path jumps straight to `FindingMergeBase`, and the fetch
choreography emits `FetchingHead` before `FetchingBase` because the head is genuinely fetched
first. The percents are a monotone display scale, not a claim about elapsed time.

## The Opened fast path

The fastest network request is the one never made. Before any workspace exists, preparation
asks a question that Git can answer in a few milliseconds: are both PR commits already in the
opened repository's object store?

### Theory: object existence is a hash-table probe

Git's object store is content-addressed. Asking "does object X exist" needs no ref, no
history walk, and no working tree: Git checks its packed and loose objects, then any
alternates, for a name. The plumbing for the question is `git cat-file -e`, which produces no
output at all and answers purely through its exit code. The `^{commit}` peel suffix adds a
type assertion: the object must exist and must resolve to a commit, so a blob or tag that
happens to share a truncated name can never satisfy the probe. The manpage is
[git-cat-file](https://git-scm.com/docs/git-cat-file).

### Practice: has_commit

`Repository::has_commit` (`src/git/mod.rs:790-799`) is the probe, guarded so that only a full
OID ever reaches Git:

```rust
pub(crate) fn has_commit(&self, oid: &str) -> bool {
    is_full_oid(oid)
        && self
            .run([
                OsString::from("cat-file"),
                OsString::from("-e"),
                OsString::from(format!("{oid}^{{commit}}")),
            ])
            .is_ok_and(|output| output.status.success())
}
```

The `is_full_oid` guard (40 or 64 hex characters) matters twice over. First, correctness: an
abbreviated OID could be ambiguous, and a ref name here would probe the wrong question (the
current tip rather than the metadata snapshot). Second, safety: metadata fields are external
input, and the guard means no crafted string starting with `-` can ever be parsed as a flag.

When both probes succeed, the arm builds `PreparedRepository::Opened(self.root().to_path_buf())`
and resolves the merge base locally with `Repository::merge_base`
(`src/git/github/mod.rs:852-863`), a plain `git merge-base <base_oid> <head_oid>` that bails
with "Git did not return a pull-request merge base" if Git prints nothing. `head` is simply
`head_oid`. No remote is contacted at any point; the fetch ladder, the compare API, and the
files endpoint are all skipped.

### When the fast path applies

The probe succeeds in exactly the situations where a reader most expects instant answers:

- **Your own branch's PR.** You wrote the commits; they are in your clone by construction.
  Opening the PR view on them costs one local `merge-base` plus one local `--name-status`.
- **A merge-committed PR you have pulled.** A true merge makes the PR head an ancestor of the
  base branch, so a fetched clone contains it.
- **A colleague's branch you fetched to review locally.** Any path by which both commits
  arrived counts; Git does not care which ref brought them.

Invariant 9 states the resulting guarantee: "PR patches first use immutable base/head OIDs
already present in the opened repository, which makes local-branch PR previews network-free."

The test that pins this is worth naming because of how it proves the property.
`locally_available_pr_objects_avoid_disposable_fetches` (`src/git/github/mod.rs:2946-2986`)
constructs a PR whose base repository URL is deliberately unreachable
(`https://invalid.example.test/...`), makes both OIDs locally present, and asserts that
preparation plus a file diff completes in under 2 seconds. If the code touched the network at
all, the unreachable host would stall or fail the test; finishing fast is the proof of zero
network I/O.

### When the fast path deceives

The notable case where the probe fails against intuition is the squash-merged PR. Squashing
synthesizes a new commit from the PR's cumulative change; the PR's own head commit never
becomes reachable from the base branch, so even a complete, fresh clone of the base repository
does not contain it. During the optimization session this was diagnosed live: a full local
clone of oven-sh/bun at `~/Desktop/bun` still crawled through per-file loads on bun#30412
because the head commit `ed1a70f8` existed only on GitHub's `refs/pull/30412/head`. Quinjet
refuses on principle to fetch that ref into the user's clone, so the Temporary path was taken,
and before PR #55 every batch of patches lazily downloaded blobs from GitHub that were already
sitting on local disk under other names. Two remedies exist and both are documented below: the
alternates borrow (automatic, [borrowing objects through alternates](#borrowing-objects-through-alternates))
and a one-time manual fetch of the PR ref into the clone, after which the fast path applies
forever for that PR:

```bash
git fetch origin +refs/pull/30412/head:refs/remotes/origin/pr-30412
```

## The disposable bare workspace

When either OID is missing locally, preparation needs a repository to fetch into. The shape it
chooses is a bare repository created fresh for this preparation, named uniquely, wired to the
right remotes, and deleted the moment the handle drops.

### Theory: what bare buys

A bare repository is a Git object database and ref store with no working tree and no index.
`git init --bare` creates just the skeleton: `HEAD`, `config`, `objects/` with its `info/` and
`pack/` subdirectories, and `refs/` (see
[gitrepository-layout](https://git-scm.com/docs/gitrepository-layout)). For a fetch-and-diff
scratch area this is exactly the right amount of repository:

- No checkout ever happens, so no file content is written twice (once as an object, once as a
  working-tree file) and no filesystem watcher sees thousands of file creations.
- No index exists, so nothing here can ever contend with `index.lock` semantics.
- Deletion is trivially safe: a bare directory holds nothing but derived data.
- `git diff <oid> <oid>` works identically in a bare repository because tree-versus-tree diffs
  never consult a working tree.

The alternative shapes lose on these axes. A temporary worktree would force a checkout of one
side (paying inflation and disk for every file, not just the changed ones). Fetching into the
opened repository would violate the no-mutation invariant. An in-process object reader would
mean linking a Git implementation; Quinjet deliberately spawns `git` for all repository
semantics (the reasoning is covered in
[plumbing and porcelain](../git-internals/plumbing-and-porcelain.md)).

### Creation: sixteen attempts at a unique name

`TemporaryBareRepository::new` (`src/git/github/mod.rs:1690-1726`) creates the directory:

```rust
fn new() -> Result<Self> {
    let preferred_parent = cache_root().map(|root| root.join("tmp"));
    let parent = match preferred_parent {
        Some(parent) if create_private_directory(&parent).is_ok() => {
            remove_stale_temporary_repositories(&parent);
            parent
        }
        // nosemgrep: rust.lang.security.temp-dir.temp-dir
        _ => env::temp_dir(),
    };
    for _ in 0..16 {
        let id = TEMPORARY_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("pr-{}-{id}.git", std::process::id()));
        if path.exists() {
            continue;
        }
        let mut command = Command::new("git");
        let _ = command
            .args(["init", "--bare", "--quiet"])
            .arg(&path)
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0");
        let output = run_bounded_command(&mut command, 64 * 1024, 64 * 1024)
            .context("failed to initialize a disposable Git repository")?;
        if !output.status.success() {
            bail!(
                "{}",
                bounded_command_error(
                    "unable to initialize disposable Git repository",
                    &output
                )
            );
        }
        return Ok(Self { path });
    }
    bail!("unable to allocate a unique disposable Git repository")
}
```

Reading it as a set of decisions:

- **Where.** The preferred parent is `<cache_root>/tmp`, so workspaces live beside the GitHub
  response cache under the per-user cache root (`~/.cache/quinjet` on Linux unless
  `QUINJET_CACHE_DIR` or `XDG_CACHE_HOME` redirects it; the full resolution order is in
  [caching](./caching.md)). `create_private_directory` makes the parent with mode `0700` on
  Unix, because a workspace will shortly contain repository content that may be private. Only
  if that fails does the code fall back to the system temp dir.
- **What name.** `pr-{pid}-{id}.git`: the process ID isolates concurrent Quinjet processes
  from each other, and `TEMPORARY_REPOSITORY_ID`, a process-wide `AtomicU64` bumped with
  `fetch_add`, isolates concurrent preparations inside one process. The `.git` extension and
  `pr-` prefix are load-bearing: the reaper recognizes candidates by exactly that shape.
- **Why sixteen attempts.** A collision requires a leftover directory from a previous process
  that happened to have the same PID and a matching counter value, which is already unlikely;
  sixteen retries with a monotonically increasing counter make persistent collision
  effectively impossible while still refusing to loop forever. Exhausting all sixteen bails
  with "unable to allocate a unique disposable Git repository" rather than picking an unsafe
  name.
- **How bounded.** Even `git init` runs through the capped runner with 64 KiB stdout and
  stderr limits, with `LC_ALL=C` for parseable errors and `GIT_TERMINAL_PROMPT=0` so nothing
  in this path can ever block a worker thread on a credential prompt.

### Deletion: Drop is the cleanup protocol

The entire teardown story is four lines (`src/git/github/mod.rs:1748-1752`):

```rust
impl Drop for TemporaryBareRepository {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}
```

Ownership does the scheduling. `TemporaryBareRepository` lives inside
`PreparedRepository::Temporary`, which lives inside `PreparedPullRequest`, which lives inside
the `Session`'s `pull_request_diff` slot. Whichever event ends the handle's life, the
directory goes with it:

- The reader opens a different PR: `Session` stores the new prepared handle into the slot,
  dropping the old one.
- A one-shot subcommand finishes: the `Session` drops at the end of `dispatch`, taking the
  handle and its workspace along.
- The terminal exits: worker threads wind down, sessions drop, workspaces vanish.

The result of `remove_dir_all` is deliberately discarded (`drop(...)`) because a destructor
must not panic and there is nothing useful to do about a failed removal at this point; the
reaper below is the second line of defense. The test
`temporary_bare_repository_is_removed_on_drop` (`src/git/github/mod.rs:3080-3087`) pins the
basic contract, and the larger integration test
`disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source`
(`src/git/github/mod.rs:2989-3077`) additionally asserts that after preparing and diffing a
21-file PR through a temporary workspace, the workspace path no longer exists and the source
repository's branch, status, and refs are byte-identical to before: the no-mutation half of
invariant 9 as an executable check.

### The 24-hour reaper

`Drop` cannot run if the process is killed, crashes, or loses power mid-preparation. Leaked
workspaces are handled by `remove_stale_temporary_repositories`
(`src/git/github/mod.rs:1754-1779`), which runs every time a new workspace is about to be
created in the cache parent:

```rust
fn remove_stale_temporary_repositories(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.filter_map(Result::ok).take(256) {
        let path = entry.path();
        let is_quinjet_pr = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| {
                name.starts_with("pr-") && Path::new(name).extension() == Some(OsStr::new("git"))
            });
        if !is_quinjet_pr {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= TEMPORARY_REPOSITORY_MAX_AGE);
        if stale {
            drop(fs::remove_dir_all(path));
        }
    }
}
```

Its conservatism is the point:

- It scans at most 256 directory entries, so a pathological tmp directory cannot make
  workspace creation slow.
- It only touches names matching `pr-*.git`, so nothing else that ends up in the cache tmp
  parent is ever at risk.
- It only removes entries whose modification time is at least `TEMPORARY_REPOSITORY_MAX_AGE`
  (24 hours, `src/git/github/mod.rs:50`) old. A workspace younger than that might belong to a
  live process; one older than a day is a leak by definition, since preparations run in
  seconds and handles live for a viewing session at most.
- Every failure mode (unreadable dir, missing metadata, clock skew) degrades to "skip", never
  to "delete".

The 24-hour window is the same constant family as the repository-identity cache TTL, and both
express the same judgment: a day is long enough that nothing legitimate is still running, and
short enough that leaked disk is reclaimed promptly.

## Borrowing objects through alternates

The alternates borrow is the smallest file in the whole design, and it removed the worst
user-visible slowness the workspace ever had.

### Theory: how Git resolves an object

When Git needs object `abc123...`, it looks in a fixed sequence of places: the repository's
own object database (pack files, then loose objects under `objects/ab/c123...`), then every
object database listed in `objects/info/alternates`, and only if all of those miss does a
partial clone attempt a lazy fetch from its promisor remote. The alternates file is a plain
text list, one object-directory path per line; relative paths resolve against the repository's
own `objects` directory, and alternates may themselves have alternates up to a small nesting
depth. The mechanism exists for exactly this kind of sharing: many repositories reading one
object store without copying it.

Two properties make alternates safe here and dangerous elsewhere. Reading through an alternate
never writes to it, so the lender cannot be corrupted by the borrower. But a lender that
prunes or repacks aggressively can delete objects a borrower still references, which is why
long-lived borrowing (as `git clone --shared` warns about) is risky. Quinjet's use is the
benign extreme: the borrower lives for one PR viewing session and holds no refs the lender
knows about, and Quinjet itself never runs `gc` or `prune` against the opened repository.

### Practice: borrow_local_objects

The implementation (`src/git/github/mod.rs:1732-1745`) with its doc comment
(`src/git/github/mod.rs:1728-1731`):

```rust
/// Let the disposable workspace read the opened repository's objects. A
/// merged or locally built pull request usually already has most of its
/// blobs on disk under other refs, so lazy blob reads resolve from the
/// local store instead of the network. The opened repository is only read.
fn borrow_local_objects(&self, repository: &Repository) {
    let Ok(common) = repository.git_common_dir() else {
        return;
    };
    let objects = common.join("objects");
    if !objects.is_dir() {
        return;
    }
    let info = self.path.join("objects").join("info");
    drop(fs::write(
        info.join("alternates"),
        format!("{}\n", objects.display()),
    ));
}
```

Details worth noticing:

- The lender path comes from `git rev-parse --git-common-dir`, not from the worktree root, so
  linked worktrees lend the shared object store they actually use rather than a per-worktree
  `.git` file.
- The whole function is best-effort: every failure path returns silently, and the write result
  is discarded. A missing alternates file merely means the workspace fetches lazily from the
  network as before; correctness never depends on the borrow.
- The written file is the complete alternates mechanism in one line: the absolute path of the
  opened repository's `objects` directory plus a newline.

### The squash-merged-PR slowness it fixed

This function is the second checkpoint commit of PR #55 ("perf: borrow local objects in the
PR workspace and keep pure-rename counts") and it answers a specific complaint from the
session: "Everything is local. Why is it taking so much time to load this for each of the
files here?" The user was viewing bun#30412 from a full local bun clone. The causal chain:

1. bun squash-merged the rewrite PR, so its head commit `ed1a70f8` is reachable only from
   GitHub's `refs/pull/30412/head`, never from `main`.
2. `has_commit(head_oid)` therefore failed, and the Temporary path was taken, as it must be:
   Quinjet never fetches into the user's clone.
3. The workspace's fetches are `--filter=blob:none`, so the fetched history contained no file
   contents. Every expanded file and every prefetch batch triggered lazy blob downloads from
   GitHub.
4. Yet nearly every one of those blobs already existed in the local clone: a squash merge
   lands the same file contents as new objects reachable from `main`, and identical content
   means identical OIDs. The network was re-downloading bytes the disk already had.

With the alternates borrow in place, step 3's lazy reads short-circuit at step 4: Git finds
the blobs through the alternate before ever consulting the promisor remote. The session
verified the fix "end to end on another merged bun PR whose head commit is absent from your
clone". The equivalence is exact because object names are content hashes: a blob borrowed from
the local store is bit-identical to the one GitHub would have served.

The borrow also quietly improves the plain fork-PR case. Any object shared between the fork
branch and the base repository history you already have (which for a typical PR is almost all
of the tree) resolves locally, so the lazy-fetch traffic reduces to roughly the blobs the PR
actually changed and that you do not already have.

## The fetch ladder

When the workspace must fetch, it fetches the minimum that can possibly answer the question,
and widens only on evidence. The choreography lives in `fetch_pull_request`
(`src/git/github/mod.rs:1781-1864`) with the single-fetch primitive `fetch_ref` below it.

### Theory: shallow, partial, and synthetic refs

Three protocol features combine here; each is covered in depth in
[shallow and partial clone](../git-internals/shallow-and-partial-clone.md), and the pack-level
consequences in [packfiles and deltas](../git-internals/packfiles-and-deltas.md).

- **Shallow fetch** (`--depth=N`): the client asks for the tip commit plus at most N-1
  generations of ancestors. The server sends a pack cut off at that boundary and the client
  records the boundary commits in its `shallow` file. History beyond the boundary simply does
  not exist locally, which is why merge-base computation can fail and deepening exists.
- **Partial clone** (`--filter=blob:none`): the server omits all blobs from the pack, sending
  only commits and trees. The remote is recorded as a promisor remote, and any later command
  that needs a missing blob fetches it lazily, on demand, one round trip at a time. A PR
  workspace fetched this way costs commits plus trees on the wire; file contents are paid for
  only for files the reader actually opens (and, after the alternates borrow, only when the
  local store cannot supply them).
- **Synthetic PR refs**: GitHub advertises `refs/pull/N/head` on the base repository for every
  PR, pointing at the PR's current head commit, even when the head branch lives in a fork and
  even after the fork branch was renamed. This is what lets the workspace fetch a fork PR
  without knowing anything about the fork in the common case. (GitHub also exposes a
  `refs/pull/N/merge` test-merge ref; Quinjet never fetches it, because the PR diff is defined
  against the merge base, not against a hypothetical merge result.)

### The fetch primitive

`fetch_ref` (`src/git/github/mod.rs:1876-1909`) issues every fetch in the module:

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
    let output = run_temp_git(temporary, &args, 128 * 1024, MAX_GH_ERROR_BYTES)?;
    if output.status.success() {
        return Ok(());
    }

    let fallback = [
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--force"),
        OsString::from("--no-tags"),
        OsString::from(format!("--depth={depth}")),
        OsString::from(remote),
        OsString::from(refspec),
    ];
    let output = run_temp_git(temporary, &fallback, 128 * 1024, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() {
        bail!(
            "{}",
            bounded_command_error("unable to fetch a pull-request ref", &output)
        );
    }
    Ok(())
}
```

Flag by flag:

- `--filter=blob:none` is the transfer optimization described above. The fallback arm exists
  because not every server permits partial-clone filters (`uploadpack.allowFilter` is opt-in
  outside github.com); when the filtered fetch fails, the identical command runs once more
  without the filter. Shallowness is preserved either way, so the worst case is "blobs of a
  depth-limited history", never "the whole repository".
- `--depth={depth}` makes every fetch depth-limited; no invocation in this module can transfer
  unbounded history.
- `--no-tags` prevents tag following, which would otherwise drag unrelated history into the
  shallow clone whenever a tag points into the fetched range.
- `--force` pairs with the `+`-prefixed refspecs: `refs/quinjet/*` refs are scratch pointers
  and must always update, fast-forward or not.
- `--quiet` plus the caps (128 KiB stdout, 256 KiB stderr through `MAX_GH_ERROR_BYTES`) keep a
  chatty or failing fetch from ballooning worker memory; fetch progress goes to stderr and is
  bounded like every other subprocess stream.

`run_temp_git` applies the same environment discipline as every repository invocation:
`git -C <workspace> -c core.quotepath=false` with `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, and
`GIT_TERMINAL_PROMPT=0`, so a fetch against an authentication-requiring remote fails
immediately instead of freezing a worker thread on a hidden prompt.

### Step 1: name the remotes and refspecs

`fetch_pull_request` first validates that metadata carried both ref names (bailing with "Pull
request metadata does not contain complete base/head refs" otherwise), then wires the base
repository as `origin` and builds two refspecs (`src/git/github/mod.rs:1800-1801`):

```rust
let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);
```

Everything fetched lands under `refs/quinjet/*`, a namespace no real repository uses, so even
inside the disposable workspace nothing can collide with a genuine ref. The refspec anatomy,
piece by piece:

| Piece | Meaning |
| --- | --- |
| `+` | Allow non-fast-forward updates of the destination |
| `refs/pull/30412/head` | Source: the synthetic PR head ref on the server |
| `:` | Separator between source and destination |
| `refs/quinjet/head` | Destination: a fixed local scratch name |

### Step 2: fetch the head, with a fork fallback

The head is fetched first (progress `FetchingHead`), at depth 64, from `origin` via the
synthetic PR ref. If that fails, the code distinguishes two situations
(`src/git/github/mod.rs:1804-1832`):

- `head_repository` is `None`: the fork was deleted, and GitHub no longer serves the PR ref
  either. The error is contextualized as "the base repository no longer exposes the PR head
  and its fork was deleted" and preparation fails; there is nothing left to fetch from.
- A fork exists: a second remote named `head` is added, pointing at a URL constructed by
  `repository_url_for_name` (`src/git/github/mod.rs:1866-1874`), which reuses the base
  repository URL's scheme and host with the fork's `owner/name` path, so a GitHub Enterprise
  PR resolves its fork on the same host rather than on github.com. The fetch retries with
  `+refs/heads/{head_ref}:refs/quinjet/head` at depth 64; a second failure bails with "unable
  to fetch PR #N from either the base PR ref or its fork".

Whichever arm succeeded, the chosen `(head_remote, head_refspec)` pair is remembered, because
the deepening ladder may need to re-fetch the head deeper later.

### Step 3: the merge-base hint short-circuit

Before fetching any base history at all, the code tries to skip it entirely
(`src/git/github/mod.rs:1834-1844`):

```rust
progress(PullRequestProgress::FindingMergeBase);
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

The hint is the merge-base OID the GitHub compare API reported (next section). Fetching it
uses a refspec whose source is a raw OID, which servers with `allow-reachable-sha1-in-want` or
tip-reachable OIDs accept, at `--depth=1`: exactly one commit plus its trees, no history. If
that lands, the whole base-branch fetch and the entire deepening ladder are skipped; the
common-case network cost of preparing a fork PR is one depth-64 head fetch plus one depth-1
merge-base fetch.

The guard on the last line is a correctness fix from the adversarial review of PR #47. The
hint was computed from the metadata snapshot's `(base_oid, head_oid)`; the fetch just
resolved whatever `refs/pull/N/head` points at now. If the branch was force-pushed between
metadata read and fetch, pairing the old merge base with the new head would produce a wrong
file list, and worse, cache it immutably under the `(M_old, H_new)` key pair. The fix accepts
the hint only when `preferred_fetched_commit` proves the fetched head is still exactly the
snapshot's `head_oid`; any mismatch falls through to the ladder, which recomputes the merge
base against whatever is actually fetched.

### Step 4: the deepening ladder

Only when there is no hint, the hint fetch failed, or the head moved does the base branch get
fetched at all (progress `FetchingBase`). The ladder (`src/git/github/mod.rs:1846-1863`):

```rust
progress(PullRequestProgress::FetchingBase);
fetch_ref(temporary, "origin", &base_refspec, 64)?;
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

The mechanism relies on how Git handles a repeated shallow fetch with a larger `--depth`: the
server extends the shallow boundary, transferring only the commits between the old boundary
and the new one. Each rung therefore pays only the delta, not the whole prefix again. The
rungs quadruple: 64, 256, 1,024, 4,096, 16,384. Geometric growth keeps the total transfer
within a small constant factor of what the final rung alone would have cost, while giving
shallow-divergence PRs (the overwhelming majority) an answer on the first rung.

Two helpers close the loop:

- `preferred_fetched_commit` (`src/git/github/mod.rs:1949-1965`) runs
  `git rev-parse --verify {oid}^{commit}` inside the workspace and returns the metadata OID if
  the fetch actually brought it; otherwise it falls back to the scratch ref name. So the ladder
  computes the merge base of the advertised commits when it can, and of the current tips when
  the advertised commits are unreachable (for example, a force-push made the snapshot head
  unreachable from any ref; showing the current PR state is the only remaining honest answer).
- `try_merge_base` (`src/git/github/mod.rs:1967-1979`) runs `git merge-base base head` and
  maps a non-zero exit to `Ok(None)`, because in a shallow repository "no merge base" usually
  means "not deep enough yet", an instruction to deepen, not an error. Only the exhausted
  ladder converts persistent absence into a failure, and the bail message names the bound
  explicitly: 16,384 commits, chosen as the refusal point for an unbounded history fetch. The
  pre-stack ladder stopped at 4,096 and this bail was a real failure mode on long-lived
  rewrite branches; PR #47 both added the API short-circuit that usually makes the ladder
  irrelevant and extended the last rung.

### A worked example: cold fork PR, hint available

Putting the whole choreography together for the common case. Suppose PR #30412 on
`oven-sh/bun`, no local objects, compare API reachable. The workspace runs exactly these Git
commands, in order:

```text
git init --bare --quiet <cache>/tmp/pr-51234-0.git
git remote add origin https://github.com/oven-sh/bun
git fetch --quiet --force --no-tags --filter=blob:none --depth=64 \
    origin +refs/pull/30412/head:refs/quinjet/head
git fetch --quiet --force --no-tags --filter=blob:none --depth=1 \
    origin +<merge-base-oid>:refs/quinjet/merge-base
git rev-parse --verify <head-oid>^{commit}
```

The `rev-parse` confirms the fetched head equals the metadata snapshot, and
`fetch_pull_request` returns `(hint, head)` without ever fetching `refs/heads/main`. Total
history transferred: at most 64 commits of the PR branch plus one merge-base commit, all
without blobs. The base branch's tens of thousands of commits are never requested. Then
enumeration runs `git diff --name-status -z` against the two OIDs, whose trees are present;
blob content is still absent and stays absent until a patch is actually generated.

### A worked example: the ladder engages

Now suppose the compare API was unreachable (offline, rate-limited, or an enterprise host
without it). After the head fetch, the base is fetched at depth 64 and the ladder walks:

```text
git fetch ... --depth=64  origin +refs/heads/main:refs/quinjet/base
git merge-base <base> <head>          (fails: histories do not meet yet)
git fetch ... --depth=256   origin +refs/heads/main:refs/quinjet/base
git fetch ... --depth=256   origin +refs/pull/30412/head:refs/quinjet/head
git merge-base <base> <head>          (fails)
git fetch ... --depth=1024  origin +refs/heads/main:refs/quinjet/base
git fetch ... --depth=1024  origin +refs/pull/30412/head:refs/quinjet/head
git merge-base <base> <head>          (succeeds at this depth)
```

Both sides deepen together because the merge base may be far behind either tip; deepening only
one side can never connect histories that diverged long ago on the other. The first rung that
connects returns immediately, so the ladder's cost is adaptive: a PR one day old pays one
64-deep fetch of each side, a six-month rewrite branch pays a few thousand blob-less commits,
and only a pathological divergence walks to the refusal point.

### What each fetch carries

It is worth being concrete about what these commands put on the wire, because the whole
design is an argument about transfer classes. A Git fetch negotiates wants and haves, then
the server sends one pack. What that pack contains differs sharply per step:

| Fetch | Commits | Trees | Blobs | Tags |
| --- | --- | --- | --- | --- |
| Head, depth 64, `blob:none` | up to 64 | those commits' trees | none | none |
| Merge-base hint, depth 1 | exactly 1 | that commit's trees | none | none |
| Ladder rung, depth N | boundary delta only | new commits' trees | none | none |
| Filter-fallback retry | as above | as above | reachable at depth | none |
| Lazy blob read (later) | none | none | exactly the missing ones | none |

Three observations tie the table to the design:

- The expensive class, blobs, is deferred out of preparation entirely. Enumeration needs
  trees only (a `--name-status` compares tree entries by OID), so the index of a
  million-line PR arrives without a single file body crossing the network.
- Blob cost is then paid per patch actually generated, at most batch-sized, and after the
  alternates borrow only for content the local store cannot supply. The deferred class
  shrinks twice before anyone pays it.
- Trees are re-sent per fetch only when missing; the deepening rungs mostly add commits,
  since deep history shares most tree content with what earlier rungs delivered, and
  identical trees are identical objects.

The pack format itself, deltas, and why "cheap in bytes" does not mean "cheap to inflate"
are the subject of [packfiles and deltas](../git-internals/packfiles-and-deltas.md).

## The merge base through the compare API

The ladder works, but for deep divergence it spends serial network round trips discovering a
fact GitHub already knows. PR #47 added the short-circuit: ask first.

### Theory: why the server knows

GitHub's compare endpoint, `GET /repos/{owner}/{repo}/compare/{base}...{head}` (see the
[GitHub REST API](https://docs.github.com/en/rest)), computes exactly the three-dot semantics
described above on the server, over the full history it hosts, and reports the result in the
`merge_base_commit` field. The server has no shallow boundary and pays no per-round-trip
latency for the DAG walk; the operation that costs Quinjet a fetch ladder costs GitHub one
graph query. One metadata request therefore replaces up to ten fetches (two per rung), and
more importantly replaces them in the exact cases where the ladder is slowest.

### Practice: merge_base_from_api

The implementation (`src/git/github/mod.rs:1288-1325`), with its doc comment
(`src/git/github/mod.rs:1285-1287`) stating the design intent verbatim: "Ask the GitHub
compare API for the merge base of the two immutable PR commits. One metadata request replaces
the deepening fetch ladder, which cannot reach a merge base thousands of commits behind either
tip."

```rust
fn merge_base_from_api(&self, pull_request: &PullRequest) -> Option<String> {
    let base = pull_request.base_oid.trim();
    let head = pull_request.head_oid.trim();
    let repository = &pull_request.base_repository;
    if !is_commit_oid(base) || !is_commit_oid(head) || repository.name_with_owner.is_empty() {
        return None;
    }
    let key = format!(
        "pr-merge-base-v1\n{}\n{base}\n{head}",
        repository.url.trim_end_matches('/')
    );
    if let Some(cached) = cache_read(&key, CacheLife::Immutable) {
        let cached = String::from_utf8_lossy(trim_ascii(&cached)).into_owned();
        if is_commit_oid(&cached) {
            return Some(cached);
        }
    }
    let output = self
        .run_gh([
            OsString::from("api"),
            OsString::from(format!(
                "repos/{}/compare/{base}...{head}",
                repository.name_with_owner
            )),
            OsString::from("--jq"),
            OsString::from(".merge_base_commit.sha"),
        ])
        .ok()?;
    if !output.status.success() || output.stdout_truncated {
        return None;
    }
    let sha = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
    if !is_commit_oid(&sha) {
        return None;
    }
    cache_write(&key, sha.as_bytes());
    Some(sha)
}
```

The function is written as a hint provider, not an authority, and every line enforces that
posture:

- The return type is `Option<String>`; there is no error path. Any failure (network down,
  rate limit, truncation, a body that is not a commit OID) yields `None`, and the caller's
  contract is that `None` merely re-enables the ladder. The workspace never becomes less
  capable than it was before this function existed.
- Both inputs and the output pass `is_commit_oid`. The output check matters because a cached
  or transported value that is not exactly a commit OID would otherwise flow into a refspec.
- The cache key `pr-merge-base-v1\n{repo url}\n{base}\n{head}` embeds both commit OIDs, so
  the entry is `CacheLife::Immutable`: the merge base of two fixed commits is a mathematical
  fact and can never change. A repeat preparation of the same PR state costs zero requests
  here forever (until cache eviction). The base OID's presence in the key is deliberate; the
  sibling counts cache learned the same lesson the hard way (next section).

The trust boundary is worth restating because the review process sharpened it: the API's
answer is taken as a hint about which single commit to fetch, and it is believed only after
the local guard in `fetch_pull_request` verifies the fetched head still matches the snapshot.
The merge base actually used is thus always consistent with the head actually diffed, whether
it came from the API or from local `git merge-base` after deepening.

## Enumerating changed files

With `(merge_base, head)` resolved, preparation's last act is building the file index: the
bounded list that drives the Files tree, the collapsed-header document, and every later patch
request. This is `changed_files_in_repository` (`src/git/github/mod.rs:1981-2089`).

### The listing command

```text
git diff --name-status -z --find-renames <merge_base> <head> --
```

`--name-status` lists each changed path with a one-letter status and computes no patch text at
all, which is what makes indexing a million-line PR cheap: the command walks two trees,
prunes every identical subtree by OID comparison, and only names what differs. `-z` terminates
every record with NUL so arbitrary path bytes (spaces, newlines, non-UTF-8) parse exactly;
combined with `core.quotepath=false` there is no unquoting logic anywhere in the parser.
`--find-renames` turns a delete/add pair into a single rename record. The trailing `--` ends
revision parsing so nothing that follows could be misread as a path or flag. The output format
and its caps are shared with the local diff pipeline, documented in
[the diff pipeline](../diff/pipeline.md).

### Caching and the fabricated exit status

The raw listing bytes are cached under `pr-files-v1\n{merge_base}\n{head}` with
`CacheLife::Immutable` and an 8 MiB limit (`MAX_PR_PATH_BYTES`): two fixed OIDs can only ever
produce one listing. A cache hit is replayed through the same parsing path as live output by
fabricating a successful `BoundedOutput` (`src/git/github/mod.rs:1999-2006` with
`successful_status` at `src/git/github/mod.rs:2124-2135`), so there is exactly one parser and
cached bytes cannot drift from live behavior. A live run is cached only when it was not
truncated: a cut listing must be re-attempted next time, not enshrined.

### Truncation repair and the parse loop

The listing is read through the capped pipe (8 MiB; the child is killed on overflow). Two
repair rules make a truncated stream still parse as whole records
(`src/git/github/mod.rs:2019-2087`):

- If the stream was truncated and does not end with NUL, everything after the last NUL is
  discarded, so a record cut mid-path vanishes instead of corrupting the tail of the index.
- The loop stops and marks `truncated = true` when `files.len() >= MAX_PR_PATHS` (16,384) or
  when a record run ends mid-file (a status with no following path, a rename with only one
  path).

The status byte maps as:

| Byte | Status |
| --- | --- |
| `A` | Added |
| `M` | Modified |
| `D` | Deleted |
| `R` | Renamed |
| `C` | Copied |
| `T` | TypeChanged |
| `U` | Unmerged |
| other | Unknown |

Rename and copy records carry a similarity score in the raw status (`R100`), matched on the
first byte only, and consume two path records: the old path then the new one, in `-z` order.
The parsed `PullRequestFile` keeps `old_path: Option<PathBuf>` so rename headers can render
"(from old)" without re-deriving anything.

When the index is truncated, `total_files` becomes
`pull_request.changed_files.max(files.len())` (`src/git/github/mod.rs:806-810`): the header
count stays honest by preferring GitHub's own total over the cut local count, so the UI can
say "showing 16,384 of 20,000" instead of silently pretending completeness.

### Where the counts come from: two worlds

Each file carries `counts: Option<DiffLineCounts>`, the `+n -n` pair that lets a header render
its real totals before any patch exists (invariant 8a: "A file's totals never depend on
whether its patch has loaded"). The source differs by preparation arm, and the difference is
one of the largest optimizations in the module:

```rust
let counts = api_counts.unwrap_or_else(|| numstat_counts(repository, merge_base, head));
```

**On the Opened path**, `api_counts` is `None` and `numstat_counts`
(`src/git/github/mod.rs:2094-2120`) runs `git diff --numstat -z --find-renames` over the same
range. Blobs are local, so counting lines is pure CPU; the result is cached immutably under
`pr-numstat-v1\n{merge_base}\n{head}`. Its doc comment states the rendering rationale: "One
extra `--numstat` pass over the same range lets every file header render its real `+n -n`
immediately, so the list never fills in unevenly as patches load." Failure or truncation of
this pass degrades to an empty map: counts are a rendering enhancement, never a correctness
requirement.

**On the Temporary path**, a local `--numstat` would be a catastrophe, and was one: to count
lines Git must inflate both versions of every changed blob, and in a `blob:none` workspace
each missing blob is a lazy network fetch. Before PR #49 this single pass downloaded
essentially every changed blob of the PR while the UI sat at "Enumerating changed files"; the
session's failure-mode analysis ranked it the dominant cold-load cost. The fix reads the
counts GitHub already has, in `pull_request_file_counts_from_api`
(`src/git/github/mod.rs:1238-1283`), whose doc comment compresses the argument: "In the
blob-less disposable workspace a local `--numstat` would download every changed blob just to
count lines; GitHub already knows the totals."

### The files endpoint read

The endpoint is `repos/{owner}/{name}/pulls/{number}/files?per_page=100`, flattened by jq into
four-field TSV records:

```text
gh api -i "repos/{owner}/{repo}/pulls/{n}/files?per_page=100&page=N" \
  --jq '.[] | [.filename, (.additions|tostring), (.deletions|tostring), .status] | @tsv'
```

Pages are read through the shared `api_page` helper (`src/git/github/mod.rs:1202-1233`), which
uses `gh api -i` to expose response headers and decides continuation from the `Link` header's
`rel="next"`. The loop runs at most `MAX_FILE_COUNT_PAGES` (64) pages, which at 100 records a
page covers 6,400 files; a truncated page aborts with `None` (a gapped counts map would
mislabel files); an incomplete but untruncated accumulation is still returned (partial counts
are still useful) but only a complete one is cached. The cache entry is immutable under
`pr-file-counts-v3\n{repo url}\n{number}\n{base}\n{head}` with the 8 MiB limit.

Two details of this key and parser are review-driven corrections, worth recording as the kind
of subtle staleness that immutable caching demands you get exactly right:

**1. The key names both commits.** The first shipped version keyed counts by
`(repo url, number, head)` alone. The adversarial review caught the flaw: per-file additions
and deletions depend on the merge base too, so retargeting a PR to a different base branch (or
a base reset) changes every count while leaving the head OID untouched, and an immutable entry
under the old key would serve stale counts forever. The fix is the `-v3` key with both `base`
and `head`, matching the merge-base cache that had included base identity from the start.

**2. Zero-count records are dropped, except pure renames.** `parse_api_file_counts`
(`src/git/github/mod.rs:1918-1943`) skips records where `additions == 0 && deletions == 0`
unless `status == "renamed"`. The skip exists because GitHub reports 0/0 for some records
where the truth is "unknown or not applicable" (mode-only changes, and some very large
generated files), and labeling those `+0 -0` would be a false claim; Quinjet renders such
files with placeholder counts until their patch arrives and backfills the real numbers. The
rename exception exists because a pure rename genuinely has zero changed lines: its honest
`+0 -0` must be kept, or renamed files would show loading skeletons forever. This exception
was itself a fix in PR #55, after the zero-skip rule from the #49 review round over-applied to
renames. The test `api_file_counts_parse_and_skip_malformed_records`
(`src/git/github/mod.rs:3177-3203`) pins both halves.

One accepted cost: the API reports no binary flag, so `parse_api_file_counts` hardcodes
`binary: false` and the "· binary" suffix that local numstat detection produces (a `-` in
either column) is absent on the workspace path. The review recorded it as a known minor loss;
the file's patch, once loaded, still identifies binary content.

## Path-scoped diffs

The index exists so that patches never have to. Once the handle holds `(merge_base, head)` and
the file list, every patch the reader ever sees is generated on demand by a `git diff` scoped
to exactly the paths that are wanted, in the prepared repository, against the two pinned OIDs.

### Theory: why pathspec limiting is cheap

`git diff <a> <b> -- <paths>` does not compute the whole diff and filter it. Git walks the two
root trees in parallel, and at every level it can discard entire subtrees two ways: any
subtree whose OID is identical on both sides is skipped wholesale (content addressing again),
and any entry that cannot match the pathspec is never descended into. Blob inflation, the
expensive part, happens only for the surviving paths. In a partial clone this matters double:
un-inflated blobs are also un-downloaded blobs. A path-scoped diff of 3 files in a 2,188-file
PR touches 3 files' worth of blobs, not 2,188, on disk and on the wire alike. The manpage is
[git-diff](https://git-scm.com/docs/git-diff).

### The patch command

`diff_selected_paths` (`src/git/github/mod.rs:2141-2173`) is the one generator:

```rust
let mut args = vec![
    OsString::from("diff"),
    OsString::from("--no-color"),
    OsString::from("--no-ext-diff"),
    OsString::from("--find-renames"),
    OsString::from("--patch"),
    OsString::from("--unified=3"),
    OsString::from(merge_base),
    OsString::from(head),
    OsString::from("--"),
];
args.extend(paths.iter().map(|path| path.as_os_str().to_owned()));
```

`--no-ext-diff` guarantees the output shape (a user-configured external diff tool would
produce arbitrary text and spawn an arbitrary process); `--no-color` keeps the bytes parseable;
`--find-renames` matches the index's rename detection so a renamed file's patch pairs old and
new content. Stdout is capped at `MAX_DIFF_BYTES` (8 MiB, `src/git/mod.rs:25`) through the
kill-on-overflow pipe, and a truncated patch is popped back to the last `\n` so it always ends
on a whole line. The unified-diff format itself and the document model the patch parses into
are covered in [the diff pipeline](../diff/pipeline.md).

### Single files: diff_file

`PreparedPullRequest::diff_file` (`src/git/github/mod.rs:402-434`) answers one path:

1. The path must exist in the index; anything else errors with "{path} is not part of this
   pull request". The index is the membership authority, so a stale or mistyped path can never
   reach Git.
2. The per-file cache is consulted under `pr-patch-v1\n{merge_base}\n{head}\n{path}`
   (`patch_cache_key`, `src/git/github/mod.rs:2137-2139`), bounded by `MAX_CACHED_PATCH_BYTES`
   (1 MiB). A hit builds the document without spawning anything: for a warm PR, opening a file
   is a disk read.
3. On a miss, `diff_selected_paths` runs for that single path, and the patch is cached only if
   it was not truncated. The 1 MiB per-file ceiling has its own doc comment
   (`src/git/github/mod.rs:40-41`): "A single file's patch is cached only if it is small
   enough that one file cannot crowd out the rest of a pull request," a fairness rule for the
   shared 128 MiB cache budget.

### Batches: diff_files

Opening files one at a time would make process spawn overhead the dominant cost of a wide PR.
The batched read, `PreparedPullRequest::diff_files` (`src/git/github/mod.rs:440-517`), carries
the design in its doc comment: "Produce many file documents from a single `git diff`.
Spawning one Git process per file dominates the cost of a wide pull request, so batching is
what lets the whole diff arrive while the reader is still reading the first file."

The algorithm:

1. Resolve every requested path against the index; unknown paths are silently dropped, and an
   empty resolution returns `Ok(vec![])` (the caller may race a workspace change; answering
   nothing is correct).
2. Partition into cache hits and misses. Hits are served from the per-file cache without
   touching Git at all, so a batch that is mostly warm costs one small invocation for the
   remainder.
3. All misses go into one `diff_selected_paths` call, producing one combined patch under the
   single 8 MiB cap.
4. `split_patch_by_file` (`src/git/diff.rs:618-663`) cuts the combined patch back into
   per-file sections by scanning for line starts matching `diff --git `, `diff --cc `, or
   `diff --combined `, the only byte sequences that begin a new file's patch in Git's output.
   Each `PatchSection` keeps the old and new path parsed from that header line, and
   `PatchSection::matches(path)` (`src/git/diff.rs:673-675`) accepts a section when either
   side equals the requested path, which is what makes renamed files findable under their new
   name.
5. Each requested file finds its section, becomes a `DiffDocument` via
   `pull_request_file_document`, and, when complete, is written into the per-file patch cache
   as a side effect. This is the quiet payoff of batching: a batch warms the cache for every
   later single-file open, so `diff_file` after prefetch is disk-only.

### Truncation: only the last section can lie

A combined patch cut at 8 MiB has a precise failure geometry: every section except the last is
provably complete (its terminating `diff --git` boundary was seen), and only the final section
may be missing its tail. `diff_files` encodes exactly that
(`src/git/github/mod.rs:487-515`):

```rust
let section_truncated = truncated && index == sections.len().saturating_sub(1);
if section_truncated && requested.len() > 1 {
    if truncated_fallback.is_none() {
        truncated_fallback = Some((
            file.path.clone(),
            pull_request_file_document(section.body, &self.pull_request, file, true),
        ));
    }
    continue;
}
```

Three rules fall out:

- A complete section is emitted and cached normally, whether or not the batch as a whole was
  cut.
- The cut last section of a multi-file batch is neither cached nor emitted as complete;
  instead it is remembered once as a fallback. The caller's next batch will request that file
  again, alone, where the whole 8 MiB budget belongs to it.
- If the batch produced nothing else (the very first section swallowed the entire cap), the
  fallback is returned, marked truncated, so a single enormous file still renders its
  truncated head instead of nothing.

That last rule is a direct fix for a livelock the adversarial review demonstrated: in the
first version, a batch whose first section overran the cap returned `Ok(vec![])`, nothing was
cached, nothing was marked prefetched, and the scheduler immediately re-dispatched the
identical batch, re-running the identical 8 MiB `git diff` in a tight worker loop forever. The
review's concrete trigger was instructive: an added minified bundle written as one 10 MB line
has `additions = 1`, so its byte estimate is tiny (one line's worth) and the byte budget
cannot see it coming. Returning the truncated document breaks the loop: the file is answered,
marked done, and rendered honestly with its truncation notice.

The integration test `disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source`
also pins the happy path at batch scale: all 21 files of a 21-file PR requested at once come
back in request order, each as a single-file document.

### A worked example: splitting a batch

An illustrative three-file batch makes the boundary rules concrete. Suppose `diff_files` is
asked for `src/lib.rs`, `docs/guide.md`, and `assets/logo.png`, none cached. One Git
invocation returns one combined patch shaped like this (bodies elided):

```diff
diff --git a/src/lib.rs b/src/lib.rs
index 3f1b2c0..9ad41e7 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -10,6 +10,7 @@ pub fn run() {
 ...
diff --git a/docs/guide.md b/docs/guide.md
index 88f2a01..1c9e442 100644
--- a/docs/guide.md
+++ b/docs/guide.md
@@ -1,4 +1,4 @@
 ...
diff --git a/assets/logo.png b/assets/logo.png
index e69de29..4b825dc 100644
Binary files a/assets/logo.png and b/assets/logo.png differ
```

`split_patch_by_file` scans for line starts matching `diff --git ` (and the `--cc` and
`--combined` variants) and produces three sections, each keyed by the old and new paths
parsed from its header line. Then, per requested file:

- `src/lib.rs` matches section 0. The batch was not truncated, so the section is complete:
  cached under its `pr-patch-v1` key and emitted as a document.
- `docs/guide.md` matches section 1: same treatment.
- `assets/logo.png` matches section 2, the last section. If the 8 MiB cap had cut the read,
  this is the only section that could be incomplete; being last is what `section_truncated`
  tests. Complete, it is cached and emitted; cut, it becomes the lone fallback for a solo
  retry.

Had the request instead included a renamed file, its section's header would read
`diff --git a/old/name.rs b/new/name.rs`, and `matches` would find it whether the caller
asked by old or new path. The parsing rules for these header forms, extended headers, and
hunks are specified in [the diff pipeline](../diff/pipeline.md).

### Who calls it, and in what sizes

Two consumers drive `diff_files`, with different batch shapes:

- **The terminal's background prefetch** (`request_pull_request_prefetch`,
  `src/app.rs:5930-5977`) sends `WorkerCommand::LoadPullRequestFileBatch` with up to
  `PULL_REQUEST_PREFETCH_BATCH` (32) paths, sized so the sum of per-file byte estimates stays
  under `PULL_REQUEST_PREFETCH_BYTE_BUDGET` (6 MiB). The estimate
  (`estimated_patch_bytes`, `src/app.rs:7052-7060`) is
  `(additions + deletions) * 80 + 4,096` bytes per file, with a
  `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE` of 512 KiB for a file whose counts are unknown; the
  6 MiB estimate budget keeps a batch's real patch comfortably under the hard 8 MiB read cap,
  so batch truncation is the exception rather than the norm. This is the reason exact counts
  are fetched up front: the counts from the files endpoint are what the byte estimator runs
  on. A single file whose estimate alone exceeds the budget still travels, alone, because the
  budget check requires the batch to be non-empty first. Batches walk the index starting at
  the file the Files tree currently shows and wrap around
  (`prefetch_anchor_index`, `src/app.rs:5912-5925`), up to
  `MAX_PREFETCHED_PULL_REQUEST_FILES` (4,096) files in total; the scheduling policy itself is
  the subject of [prefetch](./prefetch.md).
- **The CLI's `pr diff`** (`pull_request_diff`, `src/cli/mod.rs:1935-1988`) loads every wanted
  path in `paths.chunks(16)`: sixteen paths per Git invocation, serially, since a one-shot
  command has no viewport to prioritize and simply wants the whole document.

The prefetch ordering has one documented evolution step. PR #50 ("perf: prefetch smallest
files first on huge pull requests") introduced size tiers: when a PR crossed 100,000 total
changed lines or 1,000 files, prefetch candidates were sorted by ascending byte estimate so
the budget filled with the smallest files first, maximizing files-per-batch on huge PRs. PR
PR 55 ("feat: progressive viewport-first loading for huge PR file views") replaced that ordering
entirely: it removed the two threshold constants and the sort, anchored each batch at the
first file visible in the Files tree with wrap-around over the rest, and raised the total
prefetch cap from 400 to 4,096 files. The current behavior is viewport-first; smallest-first
exists only in the history as the intermediate step that taught the batch sizing its
economics. What each file's arrival does to the visible document is covered in
[progressive loading](../rendering/progressive-loading.md).

### From patch bytes to document

Every emitted section passes through `pull_request_file_document`
(`src/git/github/mod.rs:1359-1409`), which counts the patch's raw `+`/`-` lines (excluding
the `+++ `/`--- ` header lines), parses the body with `parse_diff` under the title
`PR #{number}  ·  {path}`, ORs the truncation flags, and attaches the full
`PullRequestDetails` so the rendered header can show the PR's title, author, state, and
per-file counts without another lookup. Those raw line counts also feed the #55 count
backfill: when GitHub reported no counts for a file (the 0/0-unknown case above), the arrived
patch's own counts replace the placeholder in the index.

## Staying alive between selections

A subtle property carries the whole interactive experience: the workspace must persist across
requests, because every `diff_file` and `diff_files` call is meaningless without the
`(merge_base, head)` pair and the fetched objects behind them. Quinjet expresses this lifetime
with plain ownership rather than a registry or reference counting.

### The Session slot

`cli::Session` (`src/cli/command.rs:215-219`) owns at most one prepared PR workspace:

```rust
pub(crate) struct Session {
    repository: Repository,
    local_diff: Option<(u64, PreparedLocalDiff)>,
    pull_request_diff: Option<(u64, PreparedPullRequest)>,
}
```

`Command::PreparePullRequest { workspace, pull_request }` runs the preparation and stores
`(workspace, prepared)` into the slot; storing a new pair drops the previous one, and with it
the previous temporary directory. There is deliberately no map of workspaces: a session
serves one reader looking at one PR, and "the previous PR's workspace" has no further value
once a new one is prepared, while its immutable disk caches survive independently.

### The workspace tag guard

The `u64` beside the handle is the workspace tag, and it is the staleness defense.
`Command::PullRequestFile` and `Command::PullRequestFileBatch` each carry the tag their caller
prepared under, and the guard (`src/cli/command.rs:385-391`) refuses mismatches:

```rust
fn pull_request_workspace(&self, workspace: u64) -> Result<&PreparedPullRequest> {
    self.pull_request_diff
        .as_ref()
        .filter(|(prepared, _)| *prepared == workspace)
        .map(|(_, prepared)| prepared)
        .ok_or_else(|| anyhow::anyhow!("Pull-request diff workspace is no longer available"))
}
```

In the terminal the tag is the caller's generation number; in one-shot subcommands it is the
literal 0. The semantics: a file request answers only from the exact preparation its caller
asked under. If the reader switched PRs (new generation, new preparation) while an old file
request was still queued, the old request errors instead of being answered from the wrong
workspace, and the error is ignored upstream as stale. The test
`a_prepared_workspace_answers_only_the_generation_that_asked_for_it` (`src/cli/mod.rs`)
pins it: prepare under workspace 7, read under 7 succeeds, read under 8 fails.

Invariant 10a adds the complementary rule for background batches: they are "keyed to the
prepared workspace rather than to a preview generation, so they can never invalidate a
reader's own request." A prefetch batch belongs to the workspace it was scheduled against;
if the workspace changed, the batch's tag no longer matches and its answer is refused, but a
batch in flight for the current workspace is never cancelled by mere UI navigation, so its
results still land and warm the caches.

### Why not re-prepare per request

The alternative design, rebuilding the workspace inside every file request, would be
stateless and simpler, and it loses on every axis that matters here. Preparation costs a
metadata-dependent fetch sequence (seconds, cold); a path-scoped diff costs milliseconds.
Amortizing the former over dozens of the latter is the entire economic argument of the
handle. The one-shot CLI does pay preparation per invocation, by design, and the next section
shows how the immutable caches keep even that cheap on repetition.

## Session ownership

Invariant 14 is the sentence that makes one implementation serve both of Quinjet's faces, and
it is worth quoting whole: "A session owns the prepared workspaces, which is what makes one
layer serve both faces. The terminal keeps a session per worker lane and pays for a prepared
pull request once; a subcommand builds a session, prepares, reads, and drops it, so a repeated
invocation pays the fetch again and relies on the immutable per-file caches instead.
Mutations are serialized by app state inside the terminal only; two concurrent processes can
race on the index exactly as two `git` invocations would."

### The terminal face: a session per worker lane

The terminal never runs Git on the UI thread. `GitWorker::start` (`src/git/worker.rs`) spawns
six named threads, one per mailbox lane, and each thread builds its own `Session` around its
own clone of the repository handle. From `run_worker` (`src/git/worker.rs:532-533`):

```rust
fn run_worker(repository: &Repository, mailbox: &Arc<SharedMailbox>, events: &Sender<WorkerEvent>) {
    let mut session = Session::new(repository.clone_for_worker());
```

The six lanes and their threads:

| Thread name | Lane | PR-workspace relevance |
| --- | --- | --- |
| `quinjet-git` | Background | Status, history, mutations; never touches the PR workspace |
| `quinjet-github` | GitHubMetadata | PR lookup, checks, check logs |
| `quinjet-conversation` | Conversation | The paged conversation read |
| `quinjet-preview` | LocalPreview | The prepared local diff workspace |
| `quinjet-pr-preview` | PullRequestPreview | Prepare, per-file, and batched PR diffs |
| `quinjet-warm` | Warm | Background check-log warming |

Command routing is static: `worker_lane` (`src/git/worker.rs:302-318`) maps
`PreparePullRequest`, `LoadPullRequestFile`, and `LoadPullRequestFileBatch` to
`PullRequestPreview`, and nothing else there. Consequences:

- Exactly one session in the terminal ever holds a `PreparedPullRequest`: the
  `quinjet-pr-preview` lane's. The other five sessions have the slot but never fill it,
  because the commands that fill it are pinned to one lane. Preparation happens once per
  workspace generation, and every subsequent file selection and prefetch batch reuses it: the
  terminal "pays for a prepared pull request once".
- Serialization comes free. A lane is one thread draining one mailbox, so prepare and diff
  requests against the same workspace cannot interleave; there is no lock around the
  workspace because there is no concurrency over it.
- Isolation comes free too. A slow blob-lazy batch on the PR preview lane cannot delay a
  check-log read (GitHubMetadata lane) or a status refresh (Background lane); the mailbox
  slot design on top of this (a dedicated prefetch slot behind the preview slot, so a queued
  batch can never displace the preview a reader is waiting for) is invariant 3, covered in
  [concurrency](../rendering/concurrency.md).

The worker's own role is deliberately thin (invariant 1a): "The worker adds only a generation
tag and a lane; it constructs no argument list." A `WorkerCommand::PrepareLocalDiff`
becomes `Command::PrepareLocalDiff { workspace: generation, request }`, the generation
literally becoming the workspace tag; the PR commands are wired identically. Every argv is
constructed below the `Session`, in one place, for both faces.

### The one-shot face: build, prepare, read, drop

A subcommand lives one `dispatch` call (`src/cli/mod.rs:695-750`). After the metadata verbs
are handled, the remaining repository verbs run as: discover the repository, build exactly
one `Session`, route the verb, finish the spinner, return the exit code. The PR verbs then
follow a fixed shape; `pr files` is the minimal example:

1. `Command::PullRequestLookup` resolves metadata (progress `LoadingMetadata` relabels the
   stderr spinner).
2. `Command::PreparePullRequest { workspace: 0, pull_request }` builds the workspace; every
   preparation phase relabels the spinner in turn.
3. The returned `PullRequestDiffIndex` renders as text or one JSON document.
4. `run` returns, the closure's `Session` drops, the `PreparedPullRequest` drops, and the
   temporary directory is removed before the process exits.

`pr diff` inserts step 3.5: chunked `PullRequestFileBatch` reads (16 paths each) until every
wanted file has a document, then one combined render. The workspace tag is always the literal
0 because a process that owns its session for one linear execution has no staleness to guard
against; the guard machinery still runs, it just always matches.

### What a repeated invocation pays

Because the workspace dies with the process, running `quinjet pr diff 30412` twice prepares
twice. The second run is nonetheless cheap, and itemizing where each cost goes shows the
cache design carrying the one-shot face:

| Step | First run (cold) | Second run |
| --- | --- | --- |
| Metadata | one `gh pr view` | cache hit within the 5-minute TTL |
| Merge base | one compare-API request | immutable cache hit |
| Per-file counts | up to 64 paged requests | immutable cache hit |
| Workspace | `git init --bare` + fetches | `git init --bare` + fetches again |
| File listing | one `git diff --name-status` | immutable cache hit (raw bytes) |
| Numstat (Opened path) | one `git diff --numstat` | immutable cache hit |
| Per-file patches | batched `git diff` reads | immutable cache hits under 1 MiB each |

The one genuinely repeated cost is the fetch, and even that is smaller than it looks: the
alternates borrow and the immutable patch cache mean the second run's fetches carry history
metadata while most patch bytes come from disk. Everything keyed by the OID pair survives
because those keys name their content; the cache page ([caching](./caching.md)) develops the
full immutable-versus-TTL split.

### The race the design accepts

The last sentence of invariant 14 is an honesty clause. Two Quinjet processes (say, the
terminal plus a one-shot `pr diff` in another shell) can each prepare a workspace for the
same PR concurrently. Nothing coordinates them, and nothing needs to: the temporary
directories are distinct by PID, cache writes are atomic (temp file plus rename) so
concurrent writers of the same immutable key land one complete copy, and the opened
repository is only ever read. The processes race exactly as two plain `git` invocations
would, which is the contract Git users already accept.

## Lifecycle and cleanup

Collecting the whole lifetime in one place, from first request to empty disk.

### The timeline

```text
reader asks for PR #N
  │
  ├─ metadata lookup (gh pr view, cached 5 min)
  │
  ├─ prepare_pull_request_diff
  │    ├─ has_commit(base) && has_commit(head)?
  │    │     yes ──► Opened(root): local merge-base, no network
  │    │     no ───► merge_base_from_api (hint)
  │    │            pull_request_file_counts_from_api (counts)
  │    │            TemporaryBareRepository::new
  │    │              (reap stale pr-*.git ≥ 24 h, ≤ 256 entries scanned)
  │    │            borrow_local_objects (objects/info/alternates)
  │    │            fetch_pull_request (head, hint, ladder)
  │    │
  │    └─ changed_files_in_repository ──► PullRequestDiffIndex
  │
  ├─ Session stores (tag, PreparedPullRequest)
  │
  ├─ ... diff_file / diff_files, prefetch batches, reader browses ...
  │
  └─ new prepare, session end, or process exit
       └─ Drop(PreparedPullRequest)
            └─ Drop(TemporaryBareRepository) ──► fs::remove_dir_all(pr-<pid>-<id>.git)
```

### What is on disk, where

All of the workspace machinery's persistent footprint lives under one root, resolved by
`cache_root()` (`src/git/github/mod.rs:2482-2509`): `QUINJET_CACHE_DIR` if set, else the
platform cache directory (`XDG_CACHE_HOME/quinjet` or `~/.cache/quinjet` on Linux,
`~/Library/Caches/quinjet` on macOS, `LOCALAPPDATA/quinjet/cache` on Windows).

- `<cache_root>/github/` holds the response cache: entries named by a stable 128-bit hash of
  their key, `<hash>.cache`, each starting with the magic `quinjet-gh-cache-v1\n`, directory
  mode `0700` and files `0600`, pruned oldest-first past 128 MiB or 2,048 entries. Filenames
  are opaque hashes of keys like `pr-patch-v1\n<merge_base>\n<head>\n<path>`, so selective
  per-repository deletion by name is impossible; removing the whole root is always safe, and
  everything re-fetches.
- `<cache_root>/tmp/` holds the disposable `pr-*.git` workspaces: normally empty or nearly
  so, since workspaces are removed on drop and leaked ones are swept after 24 hours.

If no cache root resolves at all, every cache helper silently degrades to a miss and the
workspace parent falls back to the system temp dir; the feature set shrinks to "no reuse",
never to an error.

### Who drops when

The drop is triggered from three distinct owners, and enumerating them shows there is no
fourth path that could leak in normal operation:

**1. Replacement.** `Session::execute` on a new `PreparePullRequest` overwrites the slot.
This is the terminal reader switching PRs: the old workspace is deleted the moment the new
one is stored, so at most one temporary workspace per session exists at any instant.

**2. Session end.** The one-shot CLI drops its session when `dispatch`'s closure returns,
successful or not; an error return unwinds through the same ownership and still deletes the
workspace. In the terminal, worker threads drop their sessions at shutdown.

**3. Process death without destructors.** Kill signals, crashes, and power loss skip `Drop`
entirely. This is the reaper's jurisdiction: the next preparation in the same cache parent
sweeps any `pr-*.git` older than 24 hours. The window is deliberate; see
[the disposable bare workspace](#the-disposable-bare-workspace).

Note what is absent: no background janitor thread, no lock files, no pidfile protocol. The
sweep runs at the only moment it is needed (when a new workspace is about to be created) and
its cost is bounded by the 256-entry scan cap.

## Design evolution and alternatives

The workspace's current shape is the residue of decisions that can be documented with their
losing alternatives, because most of them were actually exercised during the optimization
sessions.

### Compare API versus the ladder alone

The pre-stack design had only the ladder, capped at 4,096 commits, and its failure was total:
past the cap the entire PR load errored after eight wasted fetches. The alternatives
considered by the shape of the fix:

- Raising the cap alone would trade a hard failure for an unbounded transfer on exactly the
  PRs least able to afford it.
- Fetching full history (`--depth` unlimited) makes the first PR view pay for a clone.
- Asking GitHub resolves the merge base in one request over full server-side history.

The shipped compromise keeps all three levers: the API hint first (one request, one depth-1
fetch), the ladder as fallback extended to 16,384, and the hard refusal past that with an
error message that names the bound. The review round added the head-match guard that keeps
the hint honest under force-pushes.

### API counts versus local numstat

The `--numstat` pass was correct and disastrous in a blob-less workspace. The alternative of
fetching blobs eagerly for the whole PR (dropping `--filter=blob:none` for the head fetch)
was proposed during the session as a possible follow-up for mid-size PRs but not built; it
would make every preparation pay for blobs the reader may never open. Reading the counts from
the files endpoint keeps blob transfer lazy while still giving every header its real totals
up front, at two accepted costs recorded above: occasional 0/0-unknown counts (rendered as
placeholders and backfilled from the arrived patch) and the missing binary flag. The Opened
path keeps local numstat, because with local blobs it is both exact and cheap.

### Alternates borrow versus fetching into the clone

The obvious fix for the squash-merged-PR slowness was a one-line fetch of the PR ref into the
user's clone, and Quinjet refuses it on principle: invariant 9's no-mutation guarantee is
worth more than the convenience, because a tool that writes refs into your repository is a
tool you must think about before running. The alternates borrow achieves the same read
performance with zero writes to the opened repository. The manual fetch remains available to
users who want the Opened fast path permanently for one PR, and the session actually ran it
once as a demonstration; a possible future affordance ("detect a missing-but-fetchable PR
head and offer the fetch hint") was noted and not built.

### Bare workspace versus the alternatives

- A linked worktree of the opened repository would mutate the repository's worktree list
  (administrative data under `.git/worktrees/`), violating the letter of invariant 9, and a
  checkout would write every file of one side to disk.
- A clone (even `--local` or `--shared`) copies or hard-links refs and configuration the
  workspace does not need, and `--shared` reproduces the alternates mechanism with more
  machinery around it.
- An in-process object database (libgit2, gitoxide) would avoid subprocess spawns but fork
  the project's Git semantics in two: Quinjet's contract is that `git` itself is the
  authority for every repository operation, which keeps behavior identical to what the user's
  own Git would do, hooks and config quirks included.

A bare `git init` plus two `remote add`s is the minimum viable object container, and every
piece of it is standard Git that can be inspected with standard tools while it exists.

### Superseded within the stack: smallest-first ordering

PR #50's size-tiered smallest-first prefetch was correct for its constraint set: with a
400-file prefetch cap, filling the byte budget with small files maximized how many files ever
got patches at all. PR #55 changed the constraint (cap raised to 4,096, effectively the whole
index for the benchmark PR) and the objective (the reader's viewport should fill first), at
which point the sort was pure overhead and was removed along with its two threshold
constants. The episode is a clean example of an optimization being superseded rather than
wrong: both orderings solved the problem their era had.

### Deferred deliberately

Two planned work packages did not ship and their absence is part of the current contract:
index chunking past the 16,384-file cap (bun#30412 has 2,188 files; the cap plus honest
truncation covers real PRs today), and a cooperative cancellation predicate for preparation
(the generation guard already discards stale answers; cancelling the in-flight fetch itself
was judged not worth the complexity yet). A `quinjet cache clear [--repo <url>]` verb and
per-repository cache subdirectories were likewise proposed and left unbuilt.

## Measured behavior on a million-line pull request

Every number in this section is a quoted measurement from the optimization session's working
notes, with its context; the full benchmark method and every other figure live in
[benchmarking](../benchmarking.md).

### The target and the rig

The benchmark PR was oven-sh/bun#30412 "Rewrite Bun in Rust": 2,188 changed files,
+1,009,257 added lines, referred to throughout the session as "the 1M-line PR". The driving
clone was a purpose-built worst-reasonable-case repository at `/tmp/bun-test`: 389 MB on
disk, shallow (`git rev-parse --is-shallow-repository` reports true), fetching only `main`
(`remote.origin.fetch=+refs/heads/main:refs/remotes/origin/main`), with
`remote.origin.promisor=true` and `remote.origin.partialclonefilter=blob:none`. Against that
clone the workspace path is fully exercised: the squash-merged PR's head commit is absent, so
every run takes the Temporary arm.

Cold-cache runs were isolated the same way any user can isolate them:

```bash
QUINJET_CACHE_DIR=$(mktemp -d) quinjet pr files 30412
```

which points the metadata cache, every immutable entry, and the disposable `pr-*.git`
workspaces at a throwaway root; the notes call this "exactly how I benchmarked the
before/after numbers". The blunt alternative, `rm -rf ~/.cache/quinjet`, is always safe:
everything re-fetches.

One recorded caveat keeps the comparison honest: the pre-stack baseline build also completed
on this PR from this clone (the digest notes bun#30412 is merged, "so its head is reachable
in main's shallow history"), "therefore correctness was not the differentiator on this exact
clone, timing was, and the baseline cold run was measured separately."

### First verification round

Measured at the top of the original five-PR stack, before the adversarial-review fixes:

- "Metadata in 1.7s" (`pr view` against bun#30412, cold).
- "The rewrite PR enumerates all 2,188 files with real counts in 18.5s cold." (`pr files`,
  cold cache, includes workspace prepare.)
- Warm re-run of the index: 0.04s.
- Single-file patches: 0.1s.
- The session's own summary: "the 1M-line 'Rewrite Bun in Rust' PR (#30412, 2,188 files)
  loads its full file index with real counts in 18.5s cold and 0.04s warm, single-file
  patches in 0.1s, and the 1,100-entry conversation in 21s".

The 0.04 s warm index is this page's cache design measured directly: a warm run reads the
`pr-files-v1` and counts entries from disk and spawns almost nothing. The 0.1 s single patch
is the path-scoped diff economics: three files or one, the diff pays only for what it names.

### Second verification round

Measured after all review fixes landed and the stack was rebased, on the final binary:

- "Final numbers on the bun PR: cold index 6.3s, warm 0.04s, conversation 26s with the
  honest truncation notice."
- Summary: "2,188-file/1M-line index in 6.3s cold, 0.04s warm, per-file patches instant,
  conversation newest-first in 26s."

The cold index improving from 18.5 s to 6.3 s came with the review-fix round, which among
other things rebased the chain and included the counts-cache key fix. The conversation
moving from 21 s to 26 s is a documented trade: the fixed code degrades honestly on capped
reads rather than caching a gapped first page as complete (that stream's machinery is the
subject of [conversation and checks](./conversation-and-checks.md)).

### After local install

One more figure from the session, after the top-of-stack build was installed as the user's
`q` shortcut: "Smoke-tested from the bun clone: `q pr files 30412` lists all 2,188 files of
the 1M-line rewrite PR in 1.4s." That run had warm metadata and the real (non-throwaway)
cache: the everyday steady state, where the one-shot face re-prepares its workspace but every
immutable read lands on disk.

## Failure modes and edge cases

The workspace touches the network, the filesystem, a foreign server's configuration, and a
racing world of force-pushes; its reliability comes from every failure being enumerated and
bounded. First the substrate that bounds them, then the catalog.

### The bounded runner underneath everything

Every subprocess in this page, `git init` included, runs through `run_bounded_command`
(`src/git/github/mod.rs:2222-2274`), and its mechanics are what turn "a huge diff" from a
memory incident into a flag:

- stdout is read on the calling thread in 64 KiB chunks into a vector whose capacity starts
  at `min(limit, 64 KiB)`; the moment a chunk would exceed the limit, only the remaining
  allowance is kept, `stdout_truncated` is set, and the child is killed immediately. A
  runaway stream costs at most the limit plus one buffer of transfer.
- stderr is drained on a spawned thread by `read_and_drain`
  (`src/git/github/mod.rs:2280-2294`), which reads to EOF but retains at most its own limit,
  discarding the excess, so a chatty child can never deadlock against a full stderr pipe
  while the parent reads stdout.
- The child is always reaped (`child.wait()`), and error text preference is trimmed stderr,
  then stdout, then the bare exit status.

The test `bounded_runner_kills_oversized_git_output` (`src/git/github/mod.rs:3090-3105`)
pins the exact contract: a 256 KiB blob read under a 1,024-byte cap comes back truncated with
exactly 1,024 bytes retained. Invariant 6 states the principle: "Crossing a cap kills the
child rather than first allocating all output and truncating afterward."

### Catalog

**1. The fork was deleted.** `head_repository` is `None` and the synthetic PR ref fetch
failed too. There is no object source left; preparation fails with the contextualized error
"the base repository no longer exposes the PR head and its fork was deleted". This is the
only unconditional dead end in the fetch choreography, and it is GitHub's, not Quinjet's.

**2. Metadata without refs.** Empty `base_ref` or `head_ref` bails immediately with "Pull
request metadata does not contain complete base/head refs" before any remote is configured;
a half-described PR never gets a half-built workspace.

**3. Workspace name exhaustion.** Sixteen colliding candidate paths bail with "unable to
allocate a unique disposable Git repository". Reaching it requires sixteen pre-existing
directories matching this process's PID and consecutive counter values, that is, a wrecked
tmp directory; refusing is safer than reusing a directory of unknown provenance.

**4. The server refuses partial clone.** `--filter=blob:none` requires server opt-in;
`fetch_ref` retries the identical fetch without the filter. The workspace silently becomes
"shallow with blobs", correct and merely heavier. Both attempts failing surfaces "unable to
fetch a pull-request ref" with the server's own stderr bounded to 256 KiB.

**5. The merge base is beyond 16,384 commits.** The ladder exhausts and bails with "Unable
to find the PR merge base within 16,384 commits; refusing an unbounded history fetch". This
is a policy refusal, not an inability: the bound is the module's promise that no preparation
transfers unbounded history. In practice the compare API answers first for any host that
supports it, so the ladder's ceiling is reached only when the API was also unavailable.

**6. Force-push between metadata and fetch.** Two defenses compose. The merge-base hint is
used only when the fetched head resolves to the snapshot's exact `head_oid`; otherwise the
ladder recomputes against reality. And `preferred_fetched_commit` pins each side to the
advertised OID when it is reachable, falling back to the fetched ref tip only when the
snapshot commit no longer exists to be shown.

**7. The listing is truncated.** Past 8 MiB or 16,384 files the index is cut at a record
boundary, flagged `truncated`, never cached, and `total_files` prefers GitHub's
`changedFiles` count so the UI states what it is not showing. Truncation is a rendering
state here, not an error: the files that were parsed are fully usable.

**8. One file's patch exceeds 8 MiB.** The pipe kills the child, the patch is trimmed to a
whole line, the document renders with a truncation notice, and the patch is not cached (a
capped read must be re-attempted, not enshrined). The 1 MiB per-file cache ceiling
separately means an under-cap-but-large patch is served fresh each time rather than crowding
out the rest of the PR's cache entries.

**9. A batch is truncated mid-file.** Only the last section can be incomplete; it is either
retried alone next batch or, if it was the only answer, returned marked truncated. The
pre-fix behavior (return nothing, re-dispatch the identical batch forever) is the recorded
livelock from the adversarial review, and its trigger, a one-line 10 MB minified bundle
invisible to the byte estimator, is the standing reminder of why estimates guard budgets and
caps guard truth.

**10. No cache root resolves.** Every cache helper is `Option`-shaped and best-effort:
caching silently disables, workspaces fall back to the system temp dir, and behavior remains
correct with every read paid at full price.

**11. The compare API answers garbage.** Any output that is not exactly a 40- or 64-hex
commit OID is discarded (`is_commit_oid` on the way in and on the way out of the cache), and
the ladder takes over. A poisoned cache entry is equally harmless: it fails the same check on
read.

**12. The process dies mid-preparation.** The half-fetched `pr-*.git` survives on disk until
any later preparation sweeps entries older than 24 hours. Nothing in the cache is affected:
cache writes are atomic (unique temp file, then rename), so a killed writer leaves either a
complete entry or none.

**13. Two processes prepare the same PR.** Distinct workspace directories by PID, atomic
cache writes, read-only use of the opened repository: the race is benign and explicitly
accepted by invariant 14.

**14. gh is missing or unauthenticated.** Every gh spawn carries the context "failed to
execute GitHub CLI (`gh`) in {root}; install it and run `gh auth login`". On the workspace
path, metadata-dependent steps fail fast; the Opened fast path, notably, still works without
gh for any PR whose commits are local, because it never spawns it after the metadata stage.

### Edge cases worth knowing by name

- **A PR from and to the same repository** (not cross-repository) still fetches through
  `refs/pull/N/head` first; the branch name is only a fallback. The synthetic ref is the more
  reliable name even for same-repo PRs, since branches can be renamed or deleted after merge.
- **Enterprise hosts** resolve fork URLs host-preservingly (`repository_url_for_name` reuses
  the base URL's scheme and host), and repository identity matching never crosses hosts, so
  `github.com/owner/name` and an enterprise `ghe.example.com/owner/name` are distinct
  workspaces, caches, and remotes throughout.
- **Renamed files in batches** are found by either side of their patch header
  (`PatchSection::matches` checks old and new path), so requesting the new path finds a
  section whose header names both.
- **Pure renames** carry an honest `+0 -0` from the API counts parser's `renamed` exception,
  the one zero-count record class that is kept.
- **Unknown-status records** (any status byte outside `AMDRCTU`) map to
  `PullRequestFileStatus::Unknown` and flow through rather than aborting the index; a future
  Git status letter degrades display, not function.

## Reference tables

A compact index of everything this page's machinery spawns, caches, and bounds, for readers
who arrive here from a stack trace or a `ps` listing rather than from the top.

### Git commands the workspace path issues

Every invocation runs with `-C <repo> -c core.quotepath=false` and the environment
`LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0` (the `git init` omits `-C`,
taking the path as its argument). Stdout and stderr caps are per invocation.

| Command | Issued by | stdout cap | stderr cap |
| --- | --- | --- | --- |
| `git cat-file -e <oid>^{commit}` | `has_commit` (fast-path probe) | none produced | default |
| `git merge-base <base> <head>` | `Repository::merge_base` (Opened path) | metadata cap | error cap |
| `git init --bare --quiet <path>` | `TemporaryBareRepository::new` | 64 KiB | 64 KiB |
| `git remote add origin\|head <url>` | `fetch_pull_request` | 2 MiB | 256 KiB |
| `git fetch --quiet --force --no-tags [--filter=blob:none] --depth=N <remote> <refspec>` | `fetch_ref` | 128 KiB | 256 KiB |
| `git rev-parse --verify <oid>^{commit}` | `preferred_fetched_commit` | 128 KiB | 128 KiB |
| `git merge-base <base> <head>` (workspace) | `try_merge_base` | 128 KiB | 128 KiB |
| `git diff --name-status -z --find-renames <mb> <head> --` | `changed_files_in_repository` | 8 MiB | 128 KiB |
| `git diff --numstat -z --find-renames <mb> <head> --` | `numstat_counts` (Opened path) | 8 MiB | 128 KiB |
| `git diff --no-color --no-ext-diff --find-renames --patch --unified=3 <mb> <head> -- <paths>` | `diff_selected_paths` | 8 MiB | 256 KiB |

### GitHub CLI invocations in preparation

Every `gh` spawn runs with cwd at the repository root and `GH_PROMPT_DISABLED=1`,
`GH_PAGER=cat`, `GH_NO_UPDATE_NOTIFIER=1`, `NO_COLOR=1` (invariant 13), so it can never
prompt, page, colorize, or phone home from a worker thread.

| Invocation | Purpose | Cache behavior |
| --- | --- | --- |
| `gh pr view <n> --repo <url> --json <18 fields> --jq <tsv>` | metadata snapshot | `pull-request-v3`, TTL 5 min, stale served on network failure |
| `gh api repos/{o}/{r}/compare/{base}...{head} --jq .merge_base_commit.sha` | merge-base hint | `pr-merge-base-v1`, immutable |
| `gh api -i "repos/{o}/{r}/pulls/{n}/files?per_page=100&page=N" --jq <tsv>` | per-file counts | `pr-file-counts-v3`, immutable, at most 64 pages |

The metadata read's stale-on-error path deserves its one sentence here: when gh fails and an
expired cached record exists, the record is served as `Stale` and the UI warns "GitHub is
unavailable; showing stale cached metadata for #N", so an offline reader keeps a working,
labeled view.

### Cache keys the workspace reads and writes

| Key template | Life | Size cap |
| --- | --- | --- |
| `pull-request-v3\n{repo url}\n{number}` | TTL 5 min | 2 MiB |
| `pr-merge-base-v1\n{repo url}\n{base}\n{head}` | immutable | 2 MiB |
| `pr-file-counts-v3\n{repo url}\n{number}\n{base}\n{head}` | immutable | 8 MiB |
| `pr-files-v1\n{merge_base}\n{head}` | immutable | 8 MiB |
| `pr-numstat-v1\n{merge_base}\n{head}` | immutable | 8 MiB |
| `pr-patch-v1\n{merge_base}\n{head}\n{path}` | immutable | 1 MiB |

Every immutable key embeds the OIDs whose content it names, which is the entire correctness
argument: the same key can never map to different bytes. The store's mechanics (hashing,
atomic writes, pruning, permissions) are in [caching](./caching.md).

### Constants

| Constant | Value | Defined at | Bounds |
| --- | --- | --- | --- |
| `MAX_DIFF_BYTES` | 8 MiB | `src/git/mod.rs:25` | any single patch read |
| `MAX_GH_METADATA_BYTES` | 2 MiB | `src/git/github/mod.rs:33` | default gh stdout and cache limit |
| `MAX_GH_ERROR_BYTES` | 256 KiB | `src/git/github/mod.rs:36` | gh and fetch stderr |
| `MAX_PR_PATH_BYTES` | 8 MiB | `src/git/github/mod.rs:37` | listing stdout and listing caches |
| `MAX_PR_PATHS` | 16,384 | `src/git/github/mod.rs:38` | files parsed into the index |
| `MAX_FILE_COUNT_PAGES` | 64 | `src/git/github/mod.rs:39` | pages read from the files endpoint |
| `MAX_CACHED_PATCH_BYTES` | 1 MiB | `src/git/github/mod.rs:42` | per-file patch cache entry |
| `MAX_CACHE_BYTES` | 128 MiB | `src/git/github/mod.rs:46` | whole cache, pruned oldest-first |
| `MAX_CACHE_ENTRIES` | 2,048 | `src/git/github/mod.rs:47` | whole cache entry count |
| `PULL_REQUEST_CACHE_TTL` | 5 min | `src/git/github/mod.rs:49` | metadata freshness |
| `TEMPORARY_REPOSITORY_MAX_AGE` | 24 h | `src/git/github/mod.rs:50` | leaked-workspace reaping |
| ladder depths | 64, 256, 1,024, 4,096, 16,384 | `src/git/github/mod.rs:1848` | deepening rungs |
| naming attempts | 16 | `src/git/github/mod.rs:1700` | unique workspace allocation |
| reap scan | 256 entries | `src/git/github/mod.rs:1758` | stale-sweep work per creation |
| `PULL_REQUEST_PREFETCH_BATCH` | 32 | `src/app.rs:33` | paths per background batch |
| `PULL_REQUEST_PREFETCH_BYTE_BUDGET` | 6 MiB | `src/app.rs:34` | estimated bytes per batch |
| `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE` | 512 KiB | `src/app.rs:35` | estimate for countless files |
| `PULL_REQUEST_PATCH_LINE_ESTIMATE` | 80 | `src/app.rs:36` | estimated bytes per changed line |
| `MAX_PREFETCHED_PULL_REQUEST_FILES` | 4,096 | `src/app.rs:37` | total background-filled files |
| `MAX_PULL_REQUEST_DOCUMENT_BYTES` | 32 MiB | `src/app.rs:38` | in-memory parsed documents |

## Related pages

- [GitHub group overview](./README.md): how this page fits the group.
- [Prefetch](./prefetch.md): the scheduling policy that feeds `diff_files`, its mailbox slot,
  and the viewport-anchored walk.
- [API strategy](./api-strategy.md): rate limits, pagination, conditional requests, and the
  full catalog of REST reads including the compare and files endpoints used here.
- [Conversation and checks](./conversation-and-checks.md): the other big PR streams and
  their newest-first paging.
- [Caching](./caching.md): the immutable-versus-TTL split, the on-disk store, and every
  cache key this page mentioned.
- [Object model](../git-internals/object-model.md): why OIDs are immutable and what that
  buys every cache in this page.
- [Packfiles and deltas](../git-internals/packfiles-and-deltas.md): what a blob-less pack
  actually contains and why fetches are cheap in bytes.
- [Shallow and partial clone](../git-internals/shallow-and-partial-clone.md): the protocol
  mechanics behind `--depth`, `--filter=blob:none`, and promisor remotes.
- [Merge bases and history](../git-internals/merge-bases-and-history.md): the DAG theory
  behind the ladder and the compare API.
- [Plumbing and porcelain](../git-internals/plumbing-and-porcelain.md): the byte-exact
  parsing discipline and the full invocation catalog.
- [The diff pipeline](../diff/pipeline.md): from patch bytes to the document model,
  including `split_patch_by_file`.
- [Progressive loading](../rendering/progressive-loading.md): what the reader sees while
  this page's machinery streams patches in.
- [Concurrency](../rendering/concurrency.md): lanes, mailboxes, and generations around the
  session that owns the workspace.
- [Benchmarking](../benchmarking.md): the full bun#30412 story and method.
- [Techniques](../techniques.md): the catalog entry view of every trick used here.
