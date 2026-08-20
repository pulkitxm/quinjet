# Background prefetch: filling a pull request while the reader reads

Quinjet renders a pull request's file index the moment it exists and fills in the patches behind
the reader's back. This page documents that background fill end to end: the dedicated mailbox slot
that keeps it behind every interactive read, the byte-budgeted batch construction that keeps every
Git invocation under the subprocess read cap, the three eras of ordering policy that ended in
viewport-anchored wrap-around fill, the count backfill that repairs file headers from arrived
patches, the workspace-keyed replies that can never invalidate what a reader asked for, the
livelock a review found in the batching path and the fix that killed it, and the atomic
generation abort that governs the neighboring warm lane. Throughout, the general scheduling
theory comes first and the exact merged code follows it.

## Contents

- [The problem a prefetcher has to solve](#the-problem-a-prefetcher-has-to-solve)
- [Prefetch theory: demand paging, latency hiding, and working sets](#prefetch-theory-demand-paging-latency-hiding-and-working-sets)
- [The lane and the slot: where prefetch is allowed to run](#the-lane-and-the-slot-where-prefetch-is-allowed-to-run)
- [Sizing a batch: 32 files under a 6 MiB estimate](#sizing-a-batch-32-files-under-a-6-mib-estimate)
- [The 8 MiB ceiling: capped pipes under the batch](#the-8-mib-ceiling-capped-pipes-under-the-batch)
- [One process, many patches: diff_files and the section split](#one-process-many-patches-diff_files-and-the-section-split)
- [The prefetch livelock and its fix](#the-prefetch-livelock-and-its-fix)
- [Ordering, first era: index order and the 400-file stop](#ordering-first-era-index-order-and-the-400-file-stop)
- [Ordering, second era: smallest-first size tiers](#ordering-second-era-smallest-first-size-tiers)
- [Ordering today: viewport-anchored wrap-around](#ordering-today-viewport-anchored-wrap-around)
- [Count backfill: headers that repair themselves](#count-backfill-headers-that-repair-themselves)
- [Two kinds of stale: workspace-keyed batch replies](#two-kinds-of-stale-workspace-keyed-batch-replies)
- [Memory accounting: the 32 MiB parsed-document budget](#memory-accounting-the-32-mib-parsed-document-budget)
- [The warm lane and the atomic generation abort](#the-warm-lane-and-the-atomic-generation-abort)
- [Prefetch and the disk cache](#prefetch-and-the-disk-cache)
- [The life of one batch, end to end](#the-life-of-one-batch-end-to-end)
- [What a batch costs in a partial workspace](#what-a-batch-costs-in-a-partial-workspace)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Measured behavior on the benchmark](#measured-behavior-on-the-benchmark)
- [Alternatives considered and rejected](#alternatives-considered-and-rejected)
- [The invariants in force](#the-invariants-in-force)
- [Related pages](#related-pages)

## The problem a prefetcher has to solve

A pull request view has two very different kinds of content. The file index, a list of paths with
statuses and line counts, is small and cheap: one `git diff --name-status -z` read plus one
`--numstat` pass or one API listing, bounded at 8 MiB and 16,384 entries. The patches, the actual
diff bodies behind those headers, are collectively enormous: on a large pull request they can be
hundreds of megabytes of unified diff text once every file is expanded. Quinjet's answer, stated
as invariant 8 in `ARCHITECTURE.md`, is collapsed headers first: the index renders immediately
and every patch is fetched lazily. Prefetch is the machinery that turns "lazily" from "when the
reader clicks" into "before the reader clicks".

### The benchmark pull request

The whole prefetch design was driven by one concrete target. The optimization session that
produced the merged stack (PRs #46 through #50, then #52, #54, #55, all squash-merged on
2026-08-20) benchmarked against oven-sh/bun pull request 30412, "Rewrite Bun in Rust": 2,188
changed files and +1,009,257 added lines. The session digest calls it "the 1M-line PR". Before
the stack, that pull request did not merely load slowly; the session brief records that it was
"even breaking right now". The reproduction setup, a shallow `blob:none` clone driven by the
`quinjet pr` verbs with a throwaway `QUINJET_CACHE_DIR`, is documented in
[the benchmarking page](../benchmarking.md).

### What the baseline did

The pre-stack prefetcher was structurally the same shape as today's but numerically naive. It
issued fixed batches of 12 paths each (the CLI `pr diff` path used 16, an inconsistency the stack
later unified at 32), walked the index in storage order, and stopped after 400 files total. Three
consequences followed on a huge pull request, all confirmed during the analysis phase of the
session:

**1. Coverage was roughly 2 percent.** With a 400-file stop against a 20,000-file-scale index,
the overwhelming majority of files had no background fill at all. Every file past the stop was a
blocking, on-demand `git diff` issued only when selected.

**2. A fixed file count ignores file size.** Twelve paths might estimate at ten kilobytes or at
eighty megabytes; the batch had no idea. An oversized batch ran into the 8 MiB subprocess read
cap, was truncated, and silently dropped every file section after the cap with no error and no
retry of the lost tail.

**3. The on-demand path and the batch path shared a coalescing slot's neighborhood.** A reader
selecting a file while a batch was in flight had to wait for the batch's Git invocation to
finish, which on an oversized batch could be tens of seconds of pipe transfer before the kill.

The merged stack attacks each of these: byte-budgeted batches (PR #47), ordering policy
(PR #50, superseded by PR #55), a raised and viewport-anchored walk (PR #55), and strict lane
and slot discipline in the worker (predating the stack, hardened by it).

### The three scheduling questions

Every prefetcher, from a CPU cache-line prefetch unit to a video player's read-ahead buffer,
answers the same three questions:

1. *What next?* An ordering policy over the not-yet-fetched items.
2. *How much at once?* A batch size that amortizes fixed costs without hogging the channel.
3. *When to stop?* A budget that bounds total speculative work.

Quinjet's current answers are: start at the file the Files tree is showing and wrap around the
rest of the index in order; take up to 32 files whose estimated patch bytes fit in 6 MiB; stop
after 4,096 files have ever been requested for the workspace. The rest of this page is the
derivation and the exact code for each answer, plus the correctness scaffolding (generations,
lanes, caps, backfill) that lets a speculative background process coexist with an interactive
reader without ever getting in the way.

## Prefetch theory: demand paging, latency hiding, and working sets

### Demand loading and its stall cost

The simplest lazy-loading policy is pure demand loading: fetch an item the first time it is
referenced, block until it arrives. Operating systems call this demand paging; the cost model is
identical for a TUI fetching diffs. Every first touch pays the full miss latency, and the miss
latency here is not a disk seek but a subprocess spawn plus a Git object walk plus, in a
`blob:none` partial workspace, potentially a network round trip to a promisor remote for every
missing blob (see [shallow and partial clone](../git-internals/shallow-and-partial-clone.md)).
The user-visible symptom is a "Loading diff…" pause on every file selection, which is exactly
what the session's user reported against a full local clone whose one missing commit forced the
partial-workspace path: "Everything is local. Why is it taking so much time to load this for
each of the files here?"

Demand loading is unbeatable on total work: it fetches exactly what is used, nothing more. It is
worst-possible on perceived latency, because every miss happens at the worst time, the moment of
use.

### Prefetching as speculation with a budget

Prefetch inverts the tradeoff: spend work speculatively during idle time so that future
references hit. The classic risks are the classic virtual-memory ones:

- *Wasted bandwidth*: fetching items never used. Quinjet bounds this with the 4,096-file cap and
  by fetching nothing until a prepared workspace exists, which itself only exists once a reader
  has opened the pull request.
- *Cache pollution*: speculative data evicting useful data. Quinjet bounds this with the 32 MiB
  parsed-document budget pruned oldest-first, and with the rule that the on-demand single-file
  document is owned separately from the prefetch cache.
- *Channel contention*: speculation delaying demand fetches. Quinjet solves this structurally,
  with a mailbox slot that is only popped when no interactive request is waiting, rather than
  with priorities inside one queue.

The working-set framing makes the ordering question precise. At any instant the reader's working
set is the set of files they will look at soon. A prefetcher cannot know it, but it can estimate
it, and the quality of the estimate is the whole difference between the three ordering eras
documented below. Index order estimates nothing. Smallest-first estimates that the reader wants
*many* files ready, whichever they are. Viewport-anchored order estimates that the reader wants
*the files on their screen*, which is the best available proxy for the true working set because
the screen is the one thing the reader is provably looking at.

### Why a terminal client is not a browser

Web clients prefetch with cheap, highly parallel HTTP requests against a server built for
fan-out. Quinjet's fetch primitive is different in three ways that shape everything:

**1. The unit of work is a subprocess, not a socket.** Every patch read is a `git diff`
invocation. Process spawn, repository open, and object graph walk are fixed costs per
invocation, which is why batching many paths into one invocation is the single largest
constant-factor win (see the `diff_files` section below). It is also why parallel fan-out is
unattractive: dozens of concurrent `git` processes against one object store would contend on
disk and, in a partial workspace, would each race to lazily fetch blobs.

**2. Every read is capped, and the cap kills.** Subprocess stdout is read through an 8 MiB
capped pipe that kills the child on overflow (invariant 6). A batch must therefore be sized so
that its combined patch fits under the cap, because truncation is not graceful degradation, it
is losing the tail of the batch.

**3. The consumer is a 60-ish FPS render loop.** Results land on the UI thread as parsed
documents and are composed into the visible view. A prefetcher that delivered faster than the
UI could integrate would just grow queues. One batch in flight at a time, with the next batch
requested only when the previous reply has been applied, is a deliberately self-pacing design:
the prefetcher's throughput is naturally throttled by the consumer's integration speed.

## The lane and the slot: where prefetch is allowed to run

### Coalescing mailboxes instead of queues

Quinjet's worker layer does not use work queues for reads. A queue is the wrong data structure
for a UI, because a UI generates redundant requests: scrolling through ten files enqueues ten
preview requests of which only the last matters. Instead, each worker lane owns a `Mailbox`, a
set of fixed slots where a newer request of a kind overwrites the older one, plus one true queue
reserved for user mutations, which must never be dropped or reordered. The full structure, from
`src/git/worker.rs`:

```rust
#[derive(Default)]
struct Mailbox {
    operations: VecDeque<WorkerCommand>,
    branches: Option<WorkerCommand>,
    projects: Option<WorkerCommand>,
    refresh: Option<WorkerCommand>,
    preview: Option<WorkerCommand>,
    history: Option<WorkerCommand>,
    pull_request: Option<WorkerCommand>,
    repositories: Option<WorkerCommand>,
    prefetch: Option<WorkerCommand>,
    checks: Option<WorkerCommand>,
    conversation: Option<WorkerCommand>,
    check_log: Option<WorkerCommand>,
    warm: Option<WorkerCommand>,
    shutdown: bool,
}
```

Two of those slots matter for this page: `preview` and `prefetch`. The routing in
`Mailbox::push` (`src/git/worker.rs:228`) sends every interactive diff request to the `preview`
slot and every background batch to the `prefetch` slot:

```rust
command @ (WorkerCommand::PrepareLocalDiff { .. }
| WorkerCommand::LoadLocalDiffFile { .. }
| WorkerCommand::PreparePullRequest { .. }
| WorkerCommand::LoadPullRequestFile { .. }) => {
    self.preview = Some(command);
}
command @ WorkerCommand::LoadPullRequestFileBatch { .. } => {
    self.prefetch = Some(command);
}
```

Overwrite semantics are the point. If two batches are somehow pending, only the newest survives,
and since a batch is recomputed from current state every time, dropping a stale one loses
nothing. The mailbox is a lossy channel by design, in the same spirit as the filesystem
watcher's capacity-1 signal channel (invariant 4): redundant requests coalesce instead of
queuing.

### The push routing and the pop order

`Mailbox::pop` (`src/git/worker.rs:269`) encodes the priority policy as a fixed take order:

```rust
fn pop(&mut self) -> Option<WorkerCommand> {
    self.operations
        .pop_front()
        .or_else(|| self.branches.take())
        .or_else(|| self.projects.take())
        .or_else(|| self.preview.take())
        .or_else(|| self.repositories.take())
        .or_else(|| self.pull_request.take())
        .or_else(|| self.refresh.take())
        .or_else(|| self.check_log.take())
        .or_else(|| self.checks.take())
        .or_else(|| self.conversation.take())
        .or_else(|| self.history.take())
        .or_else(|| self.prefetch.take())
        .or_else(|| self.warm.take())
}
```

User mutations come first, unconditionally. The `preview` slot is taken fourth; the `prefetch`
slot is taken twelfth, behind every interactive read the process knows about, with only the
`warm` slot behind it. This is what "the dedicated mailbox slot behind the preview slot" means
concretely: the two are separate `Option` fields in the same mailbox, and the pop order
guarantees that whenever both hold a command, the preview runs first.

### What the slot behind the preview prevents

Consider the alternative: one slot for all pull-request diff work. A background batch lands in
the slot; the reader selects a file; the preview request overwrites the batch. That direction is
survivable (the batch is recomputed later). But the reverse ordering is not: the reader's
preview sits in the slot, a scheduled batch overwrites it, and the reader waits forever for a
document that was silently discarded. Splitting the slots removes both hazards at once: a batch
can never displace a preview, and a preview never destroys the background walk's progress
because the walk re-derives its next batch from app state after every reply.

`ARCHITECTURE.md` states this as part of invariant 3:

> Background diff prefetch occupies its own mailbox slot behind the preview slot, so a queued
> batch can never displace the preview a reader is waiting for.

The property is pinned by a worker test, `src/git/worker.rs:1041`, which pushes a batch, then a
preview, then a second batch, and asserts the pop order:

```rust
assert!(matches!(
    mailbox.pop(),
    Some(WorkerCommand::LoadPullRequestFile { generation: 5, .. })
));
assert!(
    matches!(
        mailbox.pop(),
        Some(WorkerCommand::LoadPullRequestFileBatch { paths, .. })
            if paths == vec![PathBuf::from("c.rs")]
    ),
    "only the newest background batch survives, and it runs after the preview"
);
```

The test name is the specification: `background_prefetch_never_displaces_the_preview_a_reader_is_waiting_for`.

### Lane isolation: one thread, one session

Slots order work within a lane; lanes isolate work between subsystems. The worker runs six OS
threads, each with its own cloned repository session and its own `SharedMailbox`, all replying
into one unbounded crossbeam event channel. The lane assignment is a `const fn` over the command
type (`src/git/worker.rs:302`):

```rust
const fn worker_lane(command: &WorkerCommand) -> WorkerLane {
    match command {
        WorkerCommand::PrepareLocalDiff { .. } | WorkerCommand::LoadLocalDiffFile { .. } => {
            WorkerLane::LocalPreview
        }
        WorkerCommand::LoadGitHubRepositories { .. }
        | WorkerCommand::LookupPullRequest { .. }
        | WorkerCommand::LoadPullRequestChecks { .. }
        | WorkerCommand::LoadCheckRunLog { .. } => WorkerLane::GitHubMetadata,
        WorkerCommand::LoadPullRequestConversation { .. } => WorkerLane::Conversation,
        WorkerCommand::PrefetchCheckRunLogs { .. } => WorkerLane::Warm,
        WorkerCommand::PreparePullRequest { .. }
        | WorkerCommand::LoadPullRequestFile { .. }
        | WorkerCommand::LoadPullRequestFileBatch { .. } => WorkerLane::PullRequestPreview,
        _ => WorkerLane::Background,
    }
}
```

Prefetch batches ride `WorkerLane::PullRequestPreview`, the `quinjet-pr-preview` thread,
alongside workspace preparation and single-file loads. That colocation is deliberate: all three
touch the same prepared workspace, and a single thread serializes them so the workspace never
sees concurrent Git invocations. Meanwhile the placement keeps prefetch away from everything
else: a slow batch cannot delay a status refresh (Background lane), a metadata poll
(GitHubMetadata lane), a conversation page (Conversation lane), or a local diff preview
(LocalPreview lane). The full lane map and what each isolation prevents is on
[the concurrency page](../rendering/concurrency.md).

### Backpressure: one batch in flight

The last structural rule lives on the app side: at most one batch is ever in flight. The
scheduler, `App::request_pull_request_prefetch` in `src/app.rs`, begins:

```rust
fn request_pull_request_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
    if self.pull_request_prefetching {
        return;
    }
```

The flag is set when a batch effect is emitted and cleared when its reply arrives, and the reply
handler's last act on success is to call `request_pull_request_prefetch` again. The result is a
strict request-reply-request cycle: the prefetcher can never build a backlog inside the worker,
its pace is bounded by how fast replies are parsed and applied, and every batch is computed from
the app state that exists at the moment it is needed, not queued from a stale plan. This is
backpressure implemented with one boolean instead of a bounded channel, which is enough
precisely because the mailbox slot already guarantees at most one queued batch.

## Sizing a batch: 32 files under a 6 MiB estimate

### The constants

The batch geometry is five constants at the top of `src/app.rs`:

```rust
const PULL_REQUEST_PREFETCH_BATCH: usize = 32;
const PULL_REQUEST_PREFETCH_BYTE_BUDGET: usize = 6 * 1024 * 1024;
const PULL_REQUEST_PATCH_FALLBACK_ESTIMATE: usize = 512 * 1024;
const PULL_REQUEST_PATCH_LINE_ESTIMATE: usize = 80;
const MAX_PREFETCHED_PULL_REQUEST_FILES: usize = 4_096;
```

In words: a batch holds at most 32 files, whose summed estimated patch sizes must stay within
6 MiB; a file with unknown counts is assumed to be 512 KiB; a known file is estimated at 80
bytes per changed line plus a fixed overhead; and background fill stops after 4,096 files have
ever been requested for the current workspace. The batch size of 32 arrived in PR #47, which
raised it from the baseline's fixed 12 while simultaneously making the byte budget the real
limiter; the 4,096 total arrived in PR #55, which raised it from 400.

### The estimate formula

The sizing function is `estimated_patch_bytes` (`src/app.rs:7052`):

```rust
fn estimated_patch_bytes(counts: Option<DiffLineCounts>) -> usize {
    counts.map_or(PULL_REQUEST_PATCH_FALLBACK_ESTIMATE, |counts| {
        counts
            .additions
            .saturating_add(counts.deletions)
            .saturating_mul(PULL_REQUEST_PATCH_LINE_ESTIMATE)
            .saturating_add(4_096)
    })
}
```

Each term has a physical meaning grounded in the unified diff format (documented byte by byte on
[the diff pipeline page](../diff/pipeline.md)):

| Term | Value | What it models |
|---|---|---|
| `additions + deletions` | per file | Every changed line becomes one `+` or `-` output line |
| `* 80` | bytes per line | Line text plus its one-byte prefix, at a generous average width |
| `+ 4_096` | bytes per file | The `diff --git` header block, `index` line, mode lines, hunk headers, and context lines |
| fallback `512 * 1024` | when counts are `None` | A file GitHub could not count is treated as potentially large |

The estimate is deliberately biased high. Context lines (three per hunk side under `--unified=3`)
and hunk headers are folded into the per-file overhead rather than modeled per hunk, and 80
bytes per line comfortably covers typical source code. Overestimating shrinks batches slightly;
underestimating risks hitting the 8 MiB pipe cap and losing the batch tail. Between those two
failure modes, small batches are merely slower and truncation is incorrect, so the bias goes one
way only.

The counts that feed the formula come from the changed-file index. On the disposable workspace
path they originate from the GitHub pulls files endpoint (PR #49; see
[the API strategy page](./api-strategy.md)), and on the local path from a `git diff --numstat`
pass over the same commit range. Either way they exist before any patch has been read, which is
what makes size-aware scheduling possible at all: the prefetcher knows approximately how big
every file's patch will be while holding none of them.

### A worked sizing example

Take a batch walk that encounters the following files, in walk order, with per-file counts
already attached to the index:

| File | additions | deletions | Estimate (bytes) | Running total |
|---|---|---|---|---|
| `src/lexer.rs` | 4,000 | 1,000 | 5,000 x 80 + 4,096 = 404,096 | 404,096 |
| `src/parser.rs` | 20,000 | 5,000 | 25,000 x 80 + 4,096 = 2,004,096 | 2,408,192 |
| `assets/schema.json` | unknown | unknown | 524,288 (fallback) | 2,932,480 |
| `src/codegen.rs` | 30,000 | 10,000 | 40,000 x 80 + 4,096 = 3,204,096 | 6,136,576 |
| `README.md` | 12 | 4 | 16 x 80 + 4,096 = 5,376 | not admitted |

The 6 MiB budget is 6,291,456 bytes. The first three files fit (2,932,480). Adding
`src/codegen.rs` would raise the total to 6,136,576, which still fits, so it is admitted. Adding
`README.md` would raise it to 6,141,952, which also fits, so in this example the walk would
actually continue; the batch closes either at the first file whose estimate no longer fits or at
the 32-file count limit, whichever comes first. The arithmetic is worth doing once by hand to
see the shape of the policy: a handful of large files fill the budget in three or four entries,
while a run of small files exhausts the 32-file count limit long before the byte budget, since
32 files at a few hundred changed lines each estimate well under 1 MiB combined.

### The admission loop

The batch is built by a single forward pass in `request_pull_request_prefetch`
(`src/app.rs:5930`), quoted here in full because every line is policy:

```rust
let remaining = MAX_PREFETCHED_PULL_REQUEST_FILES
    .saturating_sub(self.pull_request_prefetched_paths.len());
let limit = PULL_REQUEST_PREFETCH_BATCH.min(remaining);
let anchor = self
    .prefetch_anchor_index()
    .min(self.pull_request_files.len());
let (before, from_anchor) = self.pull_request_files.split_at(anchor);
let mut batch_bytes = 0_usize;
let mut paths: Vec<PathBuf> = Vec::new();
for file in from_anchor.iter().chain(before.iter()) {
    if paths.len() >= limit {
        break;
    }
    if !self.pull_request_file_needs_patch(&file.path)
        || self.pull_request_prefetched_paths.contains(&file.path)
    {
        continue;
    }
    let estimate = estimated_patch_bytes(file.counts);
    if !paths.is_empty()
        && batch_bytes.saturating_add(estimate) > PULL_REQUEST_PREFETCH_BYTE_BUDGET
    {
        break;
    }
    batch_bytes = batch_bytes.saturating_add(estimate);
    paths.push(file.path.clone());
}
```

Points worth naming:

**1. The walk never reorders.** This is first-fit admission along a fixed traversal, not bin
packing. A file that does not fit ends the batch (`break`, not `continue`), even if a smaller
file further along would have fit. That choice keeps the walk strictly sequential in traversal
order, which matters for the viewport-anchored policy: files arrive in exactly the order the
tree displays them, so the fill front moves visibly down the screen. A best-fit packer would
deliver better byte utilization per batch and a scrambled arrival order.

**2. Two skip conditions, two meanings.** `pull_request_file_needs_patch` skips files whose
document is already cached, currently loading on the preview path, or currently occupying the
single-file view; `pull_request_prefetched_paths` skips files this workspace has already spent
prefetch budget on. The distinction matters at eviction time, covered in the memory section.

**3. The count limit binds before the walk begins.** `limit` is the batch constant clamped by
the remaining global allowance, so the final batch before the 4,096 cap can be smaller than 32
without any special casing.

### Why the budget is 6 MiB when the pipe cap is 8 MiB

The batch's combined patch is read through one capped pipe whose limit is `MAX_DIFF_BYTES`,
defined in `src/git/mod.rs` as 8 MiB. If the real patch crosses that cap, the Git child is
killed and the output is truncated, losing every section after the cut. The 6 MiB budget is the
safety margin between the estimate and the cap: the estimate can be wrong by a third in
aggregate before a batch is actually truncated. The margin absorbs the cases the linear model
ignores: files with very long lines (an 80-byte average is not a guarantee), unusually hunk-dense
patches whose context lines outrun the 4,096-byte overhead, and rename or mode metadata.

The margin is a heuristic, not a proof, and the one place where it fails catastrophically rather
than gracefully is a file whose line count wildly under-represents its byte count. That case is
the subject of the livelock section below.

### The oversized file travels alone

One admission rule remains: the budget check reads `!paths.is_empty() && ...`, so the first file
of a batch is admitted unconditionally. A file whose single-file estimate exceeds the entire
6 MiB budget would otherwise be unschedulable forever: no batch could ever admit it, and the
walk would break on it every time without progress. Letting it travel alone bounds the damage
(one Git invocation reads one oversized patch, possibly truncated at 8 MiB) while guaranteeing
the walk always advances.

The app test `pull_request_prefetch_batches_by_estimated_patch_size` (`src/app.rs:8912`) pins
both halves of the behavior. Given files with 200,000, 10, and 5 added lines, the first batch is
the huge file alone and the second batch carries the two small files together:

```rust
assert!(
    matches!(
        effects.as_slice(),
        [AppEffect::Git(command)] if matches!(
            command.as_ref(),
            WorkerCommand::LoadPullRequestFileBatch { workspace_generation: 10, paths }
                if paths == &[PathBuf::from("src/huge.rs")]
        )
    ),
    "a file estimated past the byte budget travels alone"
);
```

The 200,000-line file estimates at 200,000 x 80 + 4,096 = 16,004,096 bytes, over both the 6 MiB
budget and the 8 MiB cap, so when its solitary batch runs, its patch will be truncated at 8 MiB
and rendered with a truncation marker. That is the accepted cost: one file's tail is cut rather
than a batch of thirty files losing twenty-nine of them.

## The 8 MiB ceiling: capped pipes under the batch

### run_bounded_command and the kill-on-cap contract

Everything the prefetcher does bottoms out in `run_bounded_command`
(`src/git/github/mod.rs:2222`), the one subprocess runner in the GitHub module. Its contract,
stated as invariant 6 in `ARCHITECTURE.md`, is that potentially large subprocess output is read
through capped pipes and that crossing a cap kills the child rather than first allocating all
output and truncating afterward. Mechanically:

- stdout is read on the calling thread in 64 KiB chunks into a Vec whose initial capacity is
  the smaller of the limit and 64 KiB, so a small read never preallocates its cap.
- stderr is drained concurrently on a spawned thread that retains at most its own limit and
  reads the rest to the sink, so a chatty child never deadlocks on a full stderr pipe.
- when a chunk would push stdout past the limit, only the remaining allowance is kept,
  `stdout_truncated` is set, and the child is killed immediately. A runaway `git` costs at most
  the limit plus one in-flight buffer of transfer, never unbounded memory or unbounded time.

The unit test `bounded_runner_kills_oversized_git_output` (`src/git/github/mod.rs:3090`) pins
the arithmetic: a 256 KiB blob read under a 1,024-byte cap yields `stdout_truncated == true` and
exactly 1,024 retained bytes.

The kill matters as much as the cap. Without it, an oversized `git diff` would keep writing
until Git finished walking the whole range, and the reader would pay the full generation cost of
output that was going to be discarded. With it, the cost of a sizing mistake is bounded by the
cap itself. The same runner and the same discipline back every other GitHub-layer read; the
per-stream limits (2 MiB for metadata, 8 MiB for listings and patches) are cataloged on
[the API strategy page](./api-strategy.md).

### diff_selected_paths: one invocation, one bounded read

The batch's Git invocation is `diff_selected_paths` (`src/git/github/mod.rs:2141`):

```rust
fn diff_selected_paths(
    repository: &Path,
    merge_base: &str,
    head: &str,
    paths: &[PathBuf],
) -> Result<(Vec<u8>, bool)> {
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
    let output = run_repository_git(repository, &args, MAX_DIFF_BYTES, MAX_GH_ERROR_BYTES)?;
    if !output.status.success() && !output.stdout_truncated {
        bail!(
            "{}",
            bounded_command_error("unable to generate the local pull-request diff", &output)
        );
    }
    let mut patch = output.stdout;
    if output.stdout_truncated {
        while patch.last().is_some_and(|byte| *byte != b'\n') {
            let _ = patch.pop();
        }
    }
    Ok((patch, output.stdout_truncated))
}
```

Details worth reading twice:

- The paths are passed as argv `OsString`s after a literal `--`, never through a shell
  (invariant 7). A path that looks like a flag, contains spaces, or holds arbitrary non-UTF-8
  bytes is still just one argv entry. The pathspec limits which files Git diffs, so a 32-path
  batch makes Git walk exactly those 32 blobs and no others.
- `--no-ext-diff` refuses external diff drivers, so no repository configuration can substitute
  an arbitrary program for the diff engine. `--no-color` and `--unified=3` fix the byte format
  the parser expects.
- A killed child exits nonzero, so the error check deliberately exempts the truncated case:
  truncation is a valid, flagged outcome, not a failure. Only a genuinely failed and
  non-truncated invocation bails.
- On truncation, trailing bytes after the last newline are popped so the patch ends on a whole
  line. The kill can land mid-line; the parser downstream is never handed a torn line.

- `run_repository_git` itself runs `git -C <repo> -c core.quotepath=false <args>` with
  `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, and `GIT_TERMINAL_PROMPT=0`: byte-stable output, no
  optional index locks taken against the workspace, and no possibility of an interactive
  credential prompt hanging the lane (invariant 13).

### When the estimate is wrong

The estimate is a linear model over line counts, and line counts are the wrong measure for
exactly one class of file: machine-generated content with very long lines. A minified JavaScript
bundle added as a single 10 MB line has `additions == 1`. Its estimate is 1 x 80 + 4,096 = 4,176
bytes, four orders of magnitude under its true size. The batch admits it happily alongside 31
other files, the combined patch blows through 8 MiB inside that file's section, the child is
killed, and every file after it in the batch has no section at all.

The system's defenses are layered rather than predictive, because no line-count model can see
this case coming:

1. The pipe cap bounds the damage to 8 MiB of transfer, by construction.
2. The section splitter attributes truncation only to the final section, so the completed files
   before the cut are still cached and delivered (next section).
3. The one pathological interaction that survived those two layers, a truncation inside the
   *first* emitted section, was a genuine livelock, found in review and fixed; it gets its own
   section below.
4. The files lost after the cut remain unprefetched and are simply picked up by later batches,
   because nothing marked them done.

## One process, many patches: diff_files and the section split

### Why batching wins: the cost of a process

A patch read has a fixed cost and a marginal cost. The fixed cost is everything that happens
before the first byte of useful output: `fork`/`exec` of the `git` binary, dynamic linking,
reading the repository configuration, opening the object database, and resolving the two commit
OIDs to trees. The marginal cost is walking and diffing the blobs for one more pathspec entry.
For small files the fixed cost dominates by a wide margin, and a wide pull request is mostly
small files. Fetching 2,188 patches as 2,188 processes pays the fixed cost 2,188 times; fetching
them as 69 batches of up to 32 pays it 69 times.

The doc comment on `PreparedPullRequest::diff_files` (`src/git/github/mod.rs:436`) states the
design intent exactly:

```rust
/// Produce many file documents from a single `git diff`. Spawning one Git
/// process per file dominates the cost of a wide pull request, so batching
/// is what lets the whole diff arrive while the reader is still reading the
/// first file.
```

There is a second, subtler win in a partial workspace. A `blob:none` fetch defers every blob, so
the first diff over a range triggers lazy blob downloads from the promisor remote (see
[packfiles and deltas](../git-internals/packfiles-and-deltas.md) for why blob inflation, not
transfer, is the expensive half). Git batches those lazy fetches per invocation, so one
invocation covering 32 files negotiates one download for their blobs instead of 32 separate
round trips.

### The algorithm step by step

`diff_files` receives the batch's paths and returns `(PathBuf, DiffDocument)` pairs. The merged
implementation works in six steps:

1. *Resolve.* Each requested path is looked up in the workspace's index; unknown paths are
   silently dropped. An empty resolution returns an empty Ok, not an error, because the caller
   may race a workspace replacement.
2. *Partition by cache.* Every file's per-path disk cache entry is probed first, under its
   immutable key `pr-patch-v1\n{merge_base}\n{head}\n{path}` bounded at `MAX_CACHED_PATCH_BYTES`
   (1 MiB). Hits are set aside as raw patch bytes; misses accumulate into `requested`.
3. *One invocation.* If any misses remain, a single `diff_selected_paths` call produces the
   combined patch for all of them, with the truncation flag from the 8 MiB pipe.
4. *Split.* `split_patch_by_file` (`src/git/diff.rs:618`) scans the combined patch for line
   starts matching `diff --git `, `diff --cc `, or `diff --combined `, slicing the buffer into
   `PatchSection { old_path, new_path, body }` records. The boundaries are Git's own file
   headers, so the split needs no length prefixes or sentinels; the format itself is the frame.
5. *Attribute truncation.* For each requested file, its section is found by path match against
   either side of the header (a rename matches on old or new path). A section is considered
   truncated only when the whole read was truncated *and* the section is the last one in the
   buffer, because a mid-buffer section is by construction complete: the next `diff --git `
   header proves the previous section ended.
6. *Cache and emit.* Non-truncated sections are written to the per-file disk cache (if they fit
   the 1 MiB ceiling) and emitted as parsed documents. The one truncated section is handled by
   the fallback rule described in the livelock section.

The core of the partition-and-split, from `src/git/github/mod.rs`:

```rust
let mut cached: HashMap<PathBuf, Vec<u8>> = HashMap::new();
let mut requested: Vec<PathBuf> = Vec::new();
for file in &files {
    let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
    match cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES) {
        Some(patch) => {
            drop(cached.insert(file.path.clone(), patch));
        }
        None => requested.push(file.path.clone()),
    }
}
let (patch, truncated) = if requested.is_empty() {
    (Vec::new(), false)
} else {
    diff_selected_paths(
        self.repository.path(),
        &self.merge_base,
        &self.head,
        &requested,
    )?
};
let sections = split_patch_by_file(&patch);
```

Note what the cache partition buys: a batch whose files were all fetched in a previous session
spawns no Git process at all. On a warm disk cache the entire background fill degenerates into
disk reads and parsing, which is a large part of why the warm-cache numbers in the benchmark
section are two orders of magnitude below the cold ones.

### Only the last section can be cut

The truncation attribution deserves its own statement because the livelock fix depends on it:

```rust
let section_truncated = truncated && index == sections.len().saturating_sub(1);
```

The capped pipe cuts the byte stream at exactly one point. Every section that ends before that
point is whole; the section the cut landed inside is the last one the splitter sees; and every
requested file after it simply has no section. Those three populations get three different
treatments:

| Population | Treatment |
|---|---|
| Whole sections | Parsed, disk-cached, emitted |
| The cut section | Never disk-cached; emitted only under the fallback rule, flagged truncated |
| Files after the cut | No section found, skipped without an entry, left unmarked for a later batch |

The third row is the quiet self-healing property of the whole design: a file lost to a
truncated batch is not recorded anywhere as attempted, so the very next
`request_pull_request_prefetch` walk finds it still needing a patch and schedules it again,
this time in a batch that no longer contains the oversized file (which by then is cached or
flagged). Loss is converted into retry by *not* doing bookkeeping, which is less code than any
explicit retry list and cannot leak.

This is the invariant 10a sentence in `ARCHITECTURE.md`:

> One Git invocation answers for a batch of paths and the combined patch is split back apart at
> its `diff --git` boundaries.

The exact byte-level grammar of those boundaries, extended headers, and hunk framing lives on
[the diff pipeline page](../diff/pipeline.md).

## The prefetch livelock and its fix

### The finding

The stack was adversarially reviewed before merge, and the single most serious prefetch finding
was a livelock in the batching path, rated major. The reviewer's scenario, reconstructed from
the session record:

> A batch whose diff truncates inside the first emitted section made `diff_files` return
> `Ok(vec![])` (partial section dropped because `requested.len() > 1`, all other files find no
> section), nothing was cached, and the Ok handler immediately re-dispatched the identical
> batch: "the app re-runs the identical 8 MB git diff in a tight worker loop forever."

The trigger case is exactly the estimate blind spot described earlier: an added minified bundle
written as one enormous line has `additions == 1`, so its estimate is 80 + 4,096 bytes and the
byte budget cannot see it. The reviewer's example was a 10 MB single-line file: the batch admits
it first, its section alone overflows the 8 MiB pipe, and the cut lands inside section zero.

### The failure trace

Walking the pre-fix code through that input makes the loop mechanical:

1. The batch `[bundle.min.js, a.rs, b.rs, ...]` runs; the pipe truncates inside
   `bundle.min.js`'s section. `sections` holds exactly one, truncated, section.
2. The pre-fix rule dropped a truncated section whenever the batch had requested more than one
   path, reasoning that a partial patch should be retried alone later. So `bundle.min.js`
   produced no document.
3. Every other file in the batch finds no section (they were after the cut). They are skipped.
4. `diff_files` returns `Ok` with zero documents. Crucially this is the *success* arm: nothing
   failed, so the app's retry-once error handling never engages.
5. The app handler applies zero documents, marks nothing prefetched, and calls
   `request_pull_request_prefetch` again. The walk recomputes and finds the exact same files
   still needing patches, in the same order. It emits the identical batch.
6. Steps 1 through 5 repeat forever, each iteration transferring 8 MiB through a pipe and
   killing a Git child.

This is a livelock in the textbook sense: no thread is blocked, every component is doing work,
observable state never advances. It is also invisible: the UI stays responsive (the loop runs on
the prefetch lane), the reader just sees a pull request that never finishes filling in while a
CPU core and the disk stay busy. Livelocks of this shape, a retry loop whose retry condition is
recomputed from state the failed attempt was supposed to change, are the characteristic bug of
"stateless" schedulers that re-derive their plan instead of remembering it. Re-derivation is
what makes the scheduler robust to invalidation everywhere else on this page; here it is
exactly the trap.

### The fix: a truncated fallback document

The fix, landed on the #47 branch during the review round, makes `diff_files` guarantee
progress: if the batch would otherwise produce nothing and a truncated first section exists,
that section is emitted anyway, flagged truncated. The merged code
(`src/git/github/mod.rs:471`):

```rust
let mut truncated_fallback = None;
for file in files {
    // ... cached files emitted, sections matched ...
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
    if !section_truncated {
        let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
        cache_write_bounded(&key, section.body, MAX_CACHED_PATCH_BYTES);
    }
    documents.push((
        file.path.clone(),
        pull_request_file_document(
            section.body,
            &self.pull_request,
            file,
            section_truncated,
        ),
    ));
}
if documents.is_empty()
    && let Some(fallback) = truncated_fallback
{
    documents.push(fallback);
}
```

The rule reads as three cases:

- *Truncated section, batch of one.* The file was already traveling alone (the oversized-file
  rule sent it that way, or a retry did). Its truncated document is emitted directly through the
  normal path, flagged, and never disk-cached.
- *Truncated section, batch of many, other documents produced.* The truncated file is held back
  entirely so the caller can retry it alone in a later batch, where the batch-of-one case gives
  it an honest truncated rendering. The whole files still make progress.
- *Truncated section, batch of many, nothing else produced.* The old livelock input. The
  fallback fires: the one truncated document is returned, the app caches it in memory and marks
  the path prefetched, and the next walk excludes it. Progress is guaranteed.

The guarantee can be stated as an induction: every batch reply now either delivers at least one
document (advancing `pull_request_prefetched_paths`) or is an error (consuming the retry-once
budget). Since the path population is finite and monotonically shrinking, the walk terminates.
Before the fix, the first arm could deliver zero documents while reporting success, and the
induction broke.

Note also what is *not* cached: the truncated body never reaches the disk cache, in either arm.
An immutable cache entry keyed by `(merge_base, head, path)` must be the true patch for those
commits forever; a cut body under that key would be indistinguishable from the real thing on
every future read. Truncation is a session-local rendering state, never a cached fact. The same
principle, "a partial answer must never be validated as whole", governs the ETag store on
[the caching page](./caching.md).

### The retry-once policy above it

The app-side reply handler wraps a second, coarser safety layer around genuine failures
(`src/app.rs:3336`):

```rust
Err(_) if !self.pull_request_prefetch_retrying => {
    self.pull_request_prefetch_retrying = true;
    self.request_pull_request_prefetch(&mut effects);
}
Err(_) => {
    self.pull_request_prefetch_retrying = false;
}
```

A failed batch (the worker returns `Err` when the Git invocation itself fails, for example when
the workspace directory disappeared or the object store refused a read) is retried exactly once,
immediately. A second consecutive failure stops the background walk silently: no toast, no error
state, because prefetch is speculative work whose absence merely returns files to the on-demand
path. Any successful reply resets the flag, so the budget is per-incident, not per-session. The
test `pull_request_prefetch_retries_once_after_a_failure` (`src/app.rs:9015`) pins the sequence.

The two layers answer different questions. The fallback document answers "what if Git succeeds
but the output cannot cover the batch?", which must make progress because it will reproduce
deterministically. The retry-once answers "what if the invocation fails outright?", which is
worth one immediate retry (transient contention) but not a loop (persistent breakage). Keeping
them separate keeps each trivially auditable.

### Livelock, deadlock, starvation

The three classic liveness failures all show up somewhere in this subsystem's design space, and
the merged code has a distinct defense for each:

- *Deadlock* (everyone blocked, no one can proceed): structurally impossible in the lane model,
  because a lane thread never waits on another lane; the only blocking wait is the mailbox
  condvar, and shutdown is a slot every push can set. The stderr drain thread in
  `run_bounded_command` exists precisely to prevent the classic parent-child pipe deadlock,
  where a child blocks writing a full stderr pipe while the parent blocks reading stdout.
- *Livelock* (everyone busy, no state advances): the truncated-first-section loop above, fixed
  by the progress guarantee.
- *Starvation* (one party never scheduled): the pop order intentionally starves prefetch in
  favor of interactive work, but only while interactive work exists; because the UI issues a
  bounded number of interactive requests per user action and the prefetcher re-requests after
  every reply, the prefetch slot is always reached again once the reader pauses. Priority
  without preemption plus a finite high-priority workload cannot starve forever.

## Ordering, first era: index order and the 400-file stop

The original ordering policy was no policy: the walk took `pull_request_files` from element
zero, in the order `git diff --name-status -z` emitted them, which is Git's tree-traversal
order, effectively lexicographic by path. It stopped after 400 files
(`MAX_PREFETCHED_PULL_REQUEST_FILES` before PR #55), in fixed batches of 12 paths
(`PULL_REQUEST_PREFETCH_BATCH` before PR #47).

Index order is a defensible default for the pull requests the app was originally built around.
A typical pull request changes a few dozen files; 400 covers all of them, the tree shows them
in the same order the fill proceeds, and the whole walk completes in a handful of batches
before the reader has finished the first file. The policy only becomes visible when the index
outgrows the stop, and then it fails on two axes at once:

**1. The stop is a cliff.** File 399 gets a background patch; file 401 gets a blocking
"Loading diff…" on selection, forever. On the 2,188-file benchmark PR, 82 percent of the index
was past the cliff. The analysis phase of the session put it more bluntly for a 20,000-file
projection: coverage of roughly 2 percent, with everything past coverage "a blocking on-demand
git diff through the coalescing preview slot, which an in-flight batch could block for tens of
seconds".

**2. Alphabetical position is not importance.** The files that happen to sort first eat the
budget. On the benchmark PR, index order spent its 400-file allowance wherever the path names
landed it, with no relationship to what the reader was likely to open, and a single
alphabetically early directory of large generated files could consume most of the byte budget
of every early batch.

The era's numbers survive in `ARCHITECTURE.md` history: invariant 5 read "Background prefetch
stops after 400 files" until PR #55 rewrote the sentence. The 12-path fixed batch had already
been replaced in PR #47 by the 32-file, 6 MiB byte-budgeted batch documented above, so for the
short period between #47 and #50 the policy was "index order, byte-budgeted, 400-file stop".

## Ordering, second era: smallest-first size tiers

### The huge predicate

PR #50, merged as commit `133e28a` with the subject "perf: prefetch smallest files first on
huge pull requests", introduced the first real ordering policy. Two thresholds classified a
pull request as huge:

```rust
const HUGE_PULL_REQUEST_LINES: usize = 100_000;
const HUGE_PULL_REQUEST_FILES: usize = 1_000;
```

In `request_pull_request_prefetch`, as of that commit:

```rust
let huge = self.pull_request.as_ref().is_some_and(|pull_request| {
    pull_request.additions.saturating_add(pull_request.deletions) >= HUGE_PULL_REQUEST_LINES
}) || self.pull_request_files.len() >= HUGE_PULL_REQUEST_FILES;
let mut candidates: Vec<&PullRequestFile> = self.pull_request_files.iter().collect();
if huge {
    candidates.sort_by_key(|file| estimated_patch_bytes(file.counts));
}
```

A pull request past 100,000 total changed lines or 1,000 changed files had its candidate list
sorted ascending by the same `estimated_patch_bytes` the byte budget uses; anything smaller
kept index order. The sort used `sort_by_key`, which is stable, so equal-size files preserved
their index order and the policy degraded gracefully toward era one as sizes converged. Note
which inputs the predicate reads: the PR-level `additions`/`deletions` come from the metadata
record, and the file count from the index, both available before a single patch has loaded, so
the tier decision cost nothing and never flapped mid-walk.

The PR body states the goal in one sentence: "Spend the prefetch budget on the smallest files
first once a pull request crosses 100k changed lines or 1,000 files, so most of the tree opens
instantly." The test that pinned it, `huge_pull_requests_prefetch_their_smallest_files_first`,
built a PR with `additions = 1_000_000` and files of 50,000, 5, and 500 added lines, and
asserted the batch order small, medium, big.

### Shortest-job-first and what it optimizes

Smallest-first is shortest-job-first scheduling, and SJF has a precise optimality property: for
a fixed set of jobs on one server, running them in nondecreasing size order minimizes the mean
completion time. Applied here, the "completion" of a file is the moment its patch is cached and
its row in the tree becomes instantly openable. Under a fixed budget of bytes per unit time,
smallest-first maximizes, at every instant, the *number* of files already complete.

That is the right objective under one assumption: that the reader is equally likely to open any
file, so maximizing the count of ready files maximizes the probability that the next click
hits. And the aggregate effect on a huge PR is dramatic. Consider the benchmark PR's shape: the
overwhelming majority of its 2,188 files are small, with a long tail of giant generated and
vendored files. Under index order, the giants land wherever the alphabet puts them and each one
they appear in stalls a batch's budget; under smallest-first, the first batches carry dozens of
tiny files each, and with a 400-file stop the entire allowance goes to the 400 smallest files,
the maximum possible coverage per byte. Hence the PR body's claim that "most of the tree opens
instantly": the tree *rows* most likely to be complete are most of the rows.

### Why it was superseded

The assumption is also the flaw. Readers are not equally likely to open any file. They open the
files in front of them, and smallest-first actively fights that: the file under the cursor, if
it is mid-sized, sits behind hundreds of smaller files it will never lose a comparison to. The
policy optimizes a global statistic while the reader experiences a local one, the readiness of
the specific rows on their screen. During the follow-up sessions the user reported exactly this
gap ("still the same", with a screenshot of visible files still loading), which is what
motivated PR #55's replacement.

There is also a structural cost worth recording: the sort ran over the whole candidate list on
every batch construction, and the tier predicate introduced a behavioral discontinuity at the
thresholds (a 999-file PR and a 1,000-file PR filled in visibly different orders). Both were
acceptable at 400 files and neither would have aged well at 4,096.

PR #55 deleted both `HUGE_` constants and the sort outright. Smallest-first ordering exists
only in the history between commits `133e28a` and `1261472`, and this section documents it as
an evolution step: the right optimization for the wrong objective function, replaced when a
better estimate of the reader's working set became available. The general technique, spending a
bounded budget on the cheapest items first, remains valid and is cataloged with the others in
[the technique catalog](../techniques.md).

## Ordering today: viewport-anchored wrap-around

### The anchor

PR #55 ("feat: progressive viewport-first loading for huge PR file views", commit `1261472`,
the current tip behavior) replaced the sort with an anchor. The question changed from "which
files are cheapest?" to "which files is the reader looking at?", and the answer is computed
from render state the app already tracks, in `App::prefetch_anchor_index` (`src/app.rs:5912`):

```rust
/// Where background fill should start: the first file visible in the
/// Files tree, so patches land where the reader is looking and then wrap
/// around the rest of the index in order.
fn prefetch_anchor_index(&self) -> usize {
    if self.view != View::PullRequests || self.pull_request_section != PullRequestSection::Files
    {
        return 0;
    }
    self.pull_request_tree
        .iter()
        .skip(self.sidebar_offset)
        .find_map(|entry| match entry {
            PullRequestTreeEntry::File { index, .. } => Some(*index),
            PullRequestTreeEntry::Directory { .. } => None,
        })
        .unwrap_or(0)
}
```

Reading it closely:

- Outside the Files section (the reader is on the Overview, or in another view entirely), the
  anchor is 0 and the walk degenerates to plain index order. There is no screen to anchor to,
  so the policy costs nothing.
- Inside the Files section, `sidebar_offset` is the number of tree rows scrolled above the
  viewport, so `.skip(self.sidebar_offset)` lands on the first visible row. The scan then walks
  forward to the first row that is a file, skipping directory rows, and returns that file's
  index into `pull_request_files`.
- The tree the scan walks is the flattened, fold-aware entry list: collapsed directories'
  children do not appear in it at all (invariant 10), so the anchor is always a file the reader
  can actually see, never one hidden inside a collapsed subtree.

The anchor is recomputed for every batch, because `request_pull_request_prefetch` calls it
fresh each time. Scrolling between batches retargets the fill to the new viewport with zero
bookkeeping: there is no persistent plan to invalidate, only the next batch's starting point.

### The rotation

The walk itself turns the anchor into a rotation of the index rather than a truncation of it:

```rust
let (before, from_anchor) = self.pull_request_files.split_at(anchor);
for file in from_anchor.iter().chain(before.iter()) {
```

`split_at` divides the file list into the files before the anchor and the files from the anchor
onward; chaining `from_anchor` then `before` visits every file exactly once, starting at the
anchor and wrapping past the end back to the top. The iteration is over borrowed slices: no
allocation, no copy, no sort, O(1) setup regardless of index size. Compare the superseded
policy's full-list `collect` and `sort_by_key` per batch.

Wrap-around matters as much as the anchor. A policy that filled only forward from the anchor
would strand the files above the viewport: a reader who scrolled to the bottom of the tree
would leave the entire top of the index unfetched until they scrolled back. The rotation
guarantees the whole index is covered from any anchor, with the region behind the reader simply
scheduled last, on the reasonable estimate that "behind the reader" is where they least
urgently need patches.

The test `prefetch_starts_at_the_files_viewport_and_wraps_around` (`src/app.rs:8972`) pins the
composed behavior: four files `a.rs` through `d.rs`, the Files section showing, the tree built,
and `sidebar_offset = 2`; the emitted batch's paths are asserted to be exactly
`[c.rs, d.rs, a.rs, b.rs]`, with the message "fill starts at the visible file and wraps around
the index".

### Composing with free scroll

The anchor reads `sidebar_offset`, and PR #54 (merged one position below #55 in the stack) made
`sidebar_offset` independently steerable: wheel scrolling over the sidebar pans the viewport
two rows per tick without moving the selection, setting a `sidebar_free_scroll` flag that
detaches the window from the cursor until the selection next moves. The composition is a
feature, not an accident: wheel-panning the Files tree *is* retargeting the prefetcher.

The interaction deserves spelling out because three subsystems meet in one integer:

1. The renderer clamps and maintains `sidebar_offset` each frame through
   `App::sidebar_viewport`, following the cursor when attached and leaving the offset where
   the wheel put it when detached.
2. The wheel handler mutates `sidebar_offset` directly and requests nothing: no preview, no
   effects, no selection change (the point of #54; see
   [the viewport page](../rendering/viewport.md)).
3. The next batch construction, whenever the current batch's reply arrives, reads the offset
   through `prefetch_anchor_index` and aims the fill at whatever the reader panned to.

So a reader who wheels down to a distant directory starts receiving that directory's patches
one batch boundary later, before they have clicked anything, while their selection, and
therefore the preview pane, stays put. Demand never even happens; the speculation followed the
reader's gaze. There is no event wiring between #54 and #55 making this work, just both
features reading and writing the same piece of viewport state.

### The cap raised to 4,096

PR #55 also raised `MAX_PREFETCHED_PULL_REQUEST_FILES` from 400 to 4,096, an order-of-magnitude
change in what the walk attempts. The two changes only make sense together. Anchored ordering
with a 400-file stop would fill one screen's neighborhood and then hit the cliff; the raised
cap lets the wrap-around actually cover a benchmark-scale index (2,188 files fit inside 4,096
with room to spare), so "the whole tree eventually fills in" became literally true for the
target workload.

The cap still exists, and still matters, because the index itself is bounded at
`MAX_PR_PATHS = 16_384` entries (`src/git/github/mod.rs:38`). In the gap between 4,096 and
16,384, prefetch does not attempt total coverage; files past the allowance stay on the
on-demand path with their patches served through the preview slot and the per-file disk cache.
The two limits fail independently by design: the index cap bounds one subprocess read and one
in-memory listing, the prefetch cap bounds total speculative Git work per workspace. A
50,000-file pull request truncates its index at 16,384 (with `truncated: true` surfacing in the
UI) and background-fills the first 4,096 of what remains reachable from the anchor.

`ARCHITECTURE.md` invariant 5 now carries the merged wording of this whole section:

> Background prefetch walks the whole index up to 4,096 files, starting at the file the Files
> tree is showing and wrapping around the rest in order, sizes each batch by per-file count
> estimates to stay under the 8 MiB patch read, and backfills a header's counts from its
> arrived patch when GitHub could not report them.

### Rotation versus sorting

The three eras make a tidy case study in ordering-policy design, summarized:

| Era | Policy | Optimizes | Cost per batch | Fails when |
|---|---|---|---|---|
| Baseline | Index order, 400 stop | Nothing | O(1) | Index outgrows the stop |
| PR #50 | Smallest-first past 100k lines or 1,000 files | Count of ready files | O(n log n) sort | Reader's viewport is mid-sized files |
| PR #55 | Viewport anchor, wrap-around, 4,096 stop | Readiness of visible files | O(1) rotation | Essentially only when estimates mislead batch sizing |

The deeper lesson is about information. Smallest-first used only static information (file
sizes), which was available on day one and optimized a proxy objective. Viewport anchoring uses
dynamic information (where the reader is right now), which required nothing new to collect,
`sidebar_offset` already existed for rendering, and optimizes the real objective directly. When
a scheduler can observe its consumer, following the consumer beats any static heuristic; the
static heuristic remains the right fallback exactly where the consumer is unobservable, which
is why the anchor returns 0 outside the Files section rather than trying to be clever.

## Count backfill: headers that repair themselves

### Why counts go missing

A file header in the tree and in the all-files document shows `+n -n` counts before its patch
exists. Those counts have two possible sources, and both can decline to answer:

**1. The API source declines per file.** On the disposable-workspace path, counts come from the
GitHub pulls files endpoint (PR #49), which reports `additions` and `deletions` per file. For
some files, GitHub itself reports `0/0` because it could not compute counts, typically very
large or generated files. During the session this was observed directly on the benchmark PR:
"GitHub's API itself reports additions: 0, deletions: 0 for some huge generated/added files
(the h2_client files were the example)". Storing that `0/0` as a real count would render a
false `+0 -0`, so the parser drops such records and the file's `counts` stays `None`.

There is exactly one legitimate `0/0`: a pure rename changes zero lines. PR #55 fixed an
over-application of the drop rule by carrying the API `status` field through the parser and
keeping `0/0` records whose status is `renamed`; the cache key was bumped from
`pr-file-counts-v2` to `pr-file-counts-v3` to invalidate entries written under the stricter
rule. A rename thus shows an honest `+0 -0` immediately instead of a skeleton forever. The full
counts pipeline, endpoint, jq program, pagination, and cache key, is documented on
[the API strategy page](./api-strategy.md).

**2. The local source declines wholesale.** On the local path, counts come from a
`git diff --numstat -z` pass; if that read fails or truncates at its 8 MiB cap, the whole count
map comes back empty and every file's `counts` is `None`.

A `None` count is not just a cosmetic gap. It feeds the scheduler: `estimated_patch_bytes`
charges an unknown file the 512 KiB fallback, so a batch admits at most twelve such files
before the 6 MiB budget closes (twelve at 524,288 bytes each is 6,291,456, exactly the budget,
so the twelfth is the last admitted and a thirteenth cannot fit). Unknown counts therefore make
batches conservatively small, which is the correct direction: the files GitHub could not count
are disproportionately the enormous ones.

### The skeleton placeholder

While `counts` is `None`, the header renders a loading skeleton rather than a number. PR #55
changed the placeholder glyphs in `DiffFileIndexEntry::count_spans` (`src/git/diff.rs`) from
`("+?", "-?")` to `("+··", "-··")`, two middle-dot characters that read as "pending" rather
than as an error or a shrug. The distinction matters because the placeholder is now usually
temporary: the backfill below replaces it with real numbers as soon as the file's patch
arrives, so the UI's promise is "counting", not "unknowable".

### The backfill routine

A parsed patch is a complete description of its own line counts: every `Added` line is one
addition, every `Removed` line one deletion. So the moment a document arrives, the counts
GitHub could not provide are computable locally for free, from data already in memory.
`App::backfill_pull_request_counts` (`src/app.rs:5881`) does exactly that:

```rust
/// A finished patch knows its real totals, so a file whose counts GitHub
/// could not report fills its header in as soon as its document arrives.
fn backfill_pull_request_counts(&mut self, path: &Path, document: &DiffDocument) -> bool {
    if document.truncated {
        return false;
    }
    let Some(file) = self
        .pull_request_files
        .iter_mut()
        .find(|file| file.path == path && file.counts.is_none())
    else {
        return false;
    };
    let mut additions = 0_usize;
    let mut deletions = 0_usize;
    for line in &document.lines {
        match line.kind {
            DiffLineKind::Added => additions = additions.saturating_add(1),
            DiffLineKind::Removed => deletions = deletions.saturating_add(1),
            _ => {}
        }
    }
    file.counts = Some(DiffLineCounts {
        additions,
        deletions,
        binary: false,
    });
    true
}
```

Three guards define its correctness:

- *Truncated documents never backfill.* A cut patch undercounts; writing its partial totals
  into the header would present a wrong number as fact. The header keeps its skeleton and the
  count waits for an untruncated read.
- *Existing counts are never overwritten.* The find requires `counts.is_none()`, so an
  API-reported or numstat-reported figure always wins over a recount. This makes backfill
  strictly additive: it can only replace "unknown" with "measured", never fight another source.
- *The return value reports whether anything changed*, which the batch handler uses to decide
  whether a redraw of the composed document is warranted at all.

One subtlety about timing: backfill cannot improve the *scheduler's* estimate for the file it
fills, because a file only gets a document after its patch was fetched, at which point it is
out of the walk forever. The backfilled count serves the reader (a real header) and any later
consumer of the index, not the batch sizing of the file itself. The 512 KiB fallback remains
the scheduling cost of every uncounted file up to the moment its patch lands.

### The batch handler and the counts_changed bit

Backfill runs at both arrival sites. A single-file document arrival (the on-demand preview
path) backfills its one path. The batch arrival handler threads the result through a dirty bit
(`src/app.rs:3316`):

```rust
Ok(documents) => {
    self.pull_request_prefetch_retrying = false;
    let mut arrived_visible = false;
    let mut counts_changed = false;
    for (path, document) in documents {
        if !self.pull_request_documents.contains_key(&path) {
            arrived_visible = arrived_visible
                || !self.preview_file_collapsed(&path.to_string_lossy());
            counts_changed |=
                self.backfill_pull_request_counts(&path, &document);
            self.cache_pull_request_document(path, document);
        }
    }
    if (arrived_visible || counts_changed)
        && self.pull_request_file_view == PullRequestFileView::AllFiles
    {
        self.rebuild_pull_request_all_files_document();
    }
    self.request_pull_request_prefetch(&mut effects);
}
```

The rebuild condition is the point of the two booleans. Composing the all-files document is
real work proportional to the loaded set, so it runs only when the batch changed something the
reader can see: a document for a file that is not collapsed (`arrived_visible`), or a header
count that went from skeleton to number (`counts_changed`). A batch that delivered only
collapsed files with known counts applies silently, costing nothing on the UI thread beyond
cache insertion. And the handler's final line is the self-pacing loop closing: apply, maybe
redraw, immediately schedule the next batch from post-application state.

## Two kinds of stale: workspace-keyed batch replies

### Generations as epochs

Every asynchronous reply in Quinjet carries the u64 generation of the request that caused it,
and every reply handler's first line compares that number against the current counter, dropping
mismatches (invariant 2). This is epoch-based invalidation: instead of finding and canceling
in-flight work when the world changes, the world's version number changes, and stale answers
identify themselves on arrival. Cancellation by comparison is free, has no race window (the
counter and the state it guards are both owned by the single UI thread), and needs no
cross-thread signaling.

The interesting design decision is not the mechanism but the granularity: *which* counter a
given stream answers to. Too coarse, and unrelated changes discard useful work; too fine, and
a real invalidation leaks stale data through. The pull-request diff subsystem uses two
counters with deliberately different scopes.

### The preview generation and the workspace generation

`diff_generation` is the fine-grained counter. It bumps on every preview-affecting action:
selecting a file, switching views, scheduling a new preview. A single-file load answers to it
because its reply will *replace the document the reader is looking at*, and the reader's intent
may have moved on since the request. `LoadPullRequestFile` therefore carries both numbers:

```rust
LoadPullRequestFile {
    generation: u64,
    workspace_generation: u64,
    path: PathBuf,
},
```

and its reply is checked against the current `diff_generation`, the current workspace, the
currently loading path, and continued index membership before it may touch `App::document`.

`pull_request_workspace_generation` is the coarse counter. It names the prepared workspace,
the `(merge base, head)` pair and its backing repository, that the current index belongs to. It
changes only when the workspace itself is replaced: a different pull request opened, or the
same pull request's head force-pushed (a lookup reply with a changed `head_oid` calls
`reset_pull_request_diff_runtime`, which clears the generation along with every document,
byte counter, and prefetch bookkeeping field).

A batch answers *only* to the workspace. The command carries no preview generation at all, and
the doc comment on the variant (`src/git/worker.rs:63`) is the design note:

```rust
/// Background fill for the rest of a prepared pull request. It carries no
/// preview generation because it never replaces what the reader is looking
/// at; the workspace it was prepared against is the only thing that can
/// make its results stale.
LoadPullRequestFileBatch {
    workspace_generation: u64,
    paths: Vec<PathBuf>,
},
```

### The reply gate

The handler's gate is correspondingly minimal (`src/app.rs:3307`):

```rust
WorkerEvent::PullRequestDiffBatch {
    workspace_generation,
    result,
} => {
    if Some(workspace_generation) != self.pull_request_workspace_generation {
        return effects;
    }
    self.pull_request_prefetching = false;
```

Work through the two directions of the staleness question:

*Can a batch reply be wrongly applied?* Only if its contents no longer describe the current
state. Batch documents are patches for `(merge_base, head, path)` triples; those bytes are
functions of immutable commit OIDs (see [the object model](../git-internals/object-model.md)),
so the only way they can be wrong is if the app's current workspace names different OIDs, which
is exactly what the workspace generation detects. A reader's navigation, by contrast, cannot
make a batch wrong: the batch populates a side cache keyed by path, and whichever file the
reader is now looking at either is or is not in that cache. Cache inserts are idempotent with
respect to reader intent.

*Can a batch reply wrongly displace a reader's request?* No, structurally. The batch handler
never writes `App::document` directly; it inserts into `pull_request_documents` and, at most,
triggers a rebuild of the composed all-files document, which incorporates the reader's own
fold and selection state at rebuild time. The single-file document the reader explicitly
requested is owned by the preview path and its stricter four-way gate. The two reply paths
touch disjoint state, which is the deepest reason the batch does not need the fine counter:
gating it on `diff_generation` would not protect anything, and it *would* discard perfectly
valid cache fills every time the reader moved the cursor, which on an active reader is
constantly.

`ARCHITECTURE.md` invariant 10a compresses all of this into one sentence:

> Remaining patches arrive through batched background reads keyed to the prepared workspace
> rather than to a preview generation, so they can never invalidate a reader's own request.

### The force-push walkthrough

The one scenario that exercises every piece at once: the reader has PR #30412 open, 900 files
prefetched, a batch in flight, and the author force-pushes.

1. The 20-second detail poll's silent lookup returns metadata with a new `head_oid`. The
   lookup handler classifies it as `same && head_moved` and calls
   `reset_pull_request_diff_runtime`: workspace generation to `None`, documents, order deque,
   byte count, prefetched set, loading path, single-file marker, and both prefetch flags all
   cleared. The reader's section, cursors, checks, and conversation are untouched (the doc
   comment: "Drop only the prepared diff. The section, cursors, checks and conversation stay
   exactly where the reader left them").
2. The in-flight batch, built against the old workspace, completes in the worker and its reply
   arrives carrying the old generation. The gate compares it against `None`, drops it, and the
   900 old-head patches it might have extended are already gone.
3. A new `PreparePullRequest` rides the preview slot; its index reply installs a fresh
   workspace generation and starts the first batch of the new walk, anchored at whatever the
   Files tree currently shows.
4. Every per-file disk cache entry from the old head remains on disk under its old
   `pr-patch-v1\n{old_base}\n{old_head}\n{path}` key, harmless because nothing will ever ask
   that question again, until the cache's oldest-first pruning reclaims it (see
   [the caching page](./caching.md)).

No cancellation message crossed a thread, no lock was held, and no stale byte reached the
screen. The session's test for the app-level half is named for the experience:
"a force push replaces the diff, not the reader's place in the view".

## Memory accounting: the 32 MiB parsed-document budget

### Measuring a document

Prefetched patches live in memory as parsed `DiffDocument`s, not raw bytes: lines split, spans
built, syntax colors resolved. Parsed form is what the renderer consumes, but it is also
several times larger than the patch text, so the cache is accounted in real in-memory bytes
rather than file counts. `diff_document_size` (`src/app.rs:7062`) walks the structure and sums
`size_of_val` for the document, each line, and each span, plus the heap capacity of the title
and of every span's text. Capacity, not length: a `String` owns its allocation regardless of
how much of it is used, and the budget exists to bound allocations.

The budget itself is `MAX_PULL_REQUEST_DOCUMENT_BYTES = 32 * 1024 * 1024` (`src/app.rs:38`),
32 MiB of parsed documents per open pull request, alongside the byte-level bounds that
invariant 6 places on the parsing itself (512 KiB of grammar parsing per patch, 32 KiB per
row; see [intraline and highlighting](../diff/intraline-and-highlighting.md)).

### Insertion and pruning

Every arriving document, from a batch or from the on-demand path being stashed back, goes
through one bookkeeping choke point, `cache_pull_request_document` (`src/app.rs:5742`):

```rust
fn cache_pull_request_document(&mut self, path: PathBuf, document: DiffDocument) {
    if let Some(previous) = self.pull_request_documents.remove(&path) {
        self.pull_request_document_bytes = self
            .pull_request_document_bytes
            .saturating_sub(diff_document_size(&previous));
        self.pull_request_document_order
            .retain(|candidate| candidate != &path);
    }
    self.pull_request_document_bytes = self
        .pull_request_document_bytes
        .saturating_add(diff_document_size(&document));
    let _ = self.pull_request_prefetched_paths.insert(path.clone());
    self.pull_request_document_order.push_back(path.clone());
    drop(self.pull_request_documents.insert(path, document));
    self.prune_pull_request_documents(MAX_PULL_REQUEST_DOCUMENT_BYTES);
}
```

The eviction policy is insertion-order FIFO over `pull_request_document_order`, a `VecDeque`
of paths: pruning pops the oldest inserted path while the byte total exceeds the budget,
always keeping at least one document (`self.pull_request_documents.len() > 1` guards the
loop), so a single document larger than the entire budget is held rather than thrashed.
Insertion order approximates least-recently-*loaded* rather than least-recently-*used*; the
one strong recency signal the app has, "the reader opened this file", is handled by moving
that document out of the cache entirely into the single-file slot via
`take_pull_request_document`, which subtracts its bytes and removes its order entry. The test
pinning the policy is named `parsed pull request documents evict oldest entries by size`.

On the benchmark PR the budget is expected to be hit: 2,188 files of parsed documents will not
fit in 32 MiB, so by the time the wrap-around walk finishes, the earliest-filled region has
been evicted in favor of the latest. The system's bet is that the disk cache makes eviction
cheap to reverse, which is the next point.

### The prefetched-path set survives eviction

Note the asymmetry in the code above: insertion adds the path to
`pull_request_prefetched_paths`, but `prune_pull_request_documents` removes only from the
document map and the order deque. An evicted file keeps its membership in the prefetched set,
so the background walk never fetches it a second time. Its next appearance is on demand: when
the reader selects it, `request_pull_request_diff_file` misses the in-memory map and issues a
`LoadPullRequestFile`, which lands in `PreparedPullRequest::diff_file` and, for any patch that
fit the 1 MiB per-file ceiling, is served from the immutable disk cache without spawning Git.

The design consequence is worth stating plainly: prefetch budget is spent at most once per
path per workspace. Background fill is a one-pass warmer, not a resident cache manager; after
the pass, the disk cache absorbs re-reads and the in-memory budget holds the most recently
loaded 32 MiB. For patches too large for the disk-cache ceiling, an evicted file does pay a
fresh path-scoped `git diff` on selection, the price of keeping one file from crowding out a
pull request on disk (invariant 12a's 1 MiB ceiling, whose rationale is the doc comment in
`src/git/github/mod.rs`: "A single file's patch is cached only if it is small enough that one
file cannot crowd out the rest of a pull request").

All of the bookkeeping resets wholesale with the workspace: `reset_pull_request_diff_runtime`
zeroes the map, the deque, the byte counter, and the prefetched set together, so a new head
starts a genuinely new one-pass walk.

## The warm lane and the atomic generation abort

### What warming is for

The prefetch family has a second member. Opening a pull request also warms the logs of its
settled check runs: `request_check_log_prefetch` (`src/app.rs:6217`) collects checks that are
not running and have a job id, skips identities already warmed this session, caps the
collection at 32 per pull request, and sends one `PrefetchCheckRunLogs` command. The doc
comment gives the objective:

```rust
/// Warm every finished run's log once per pull request. Selecting a check
/// then costs a disk read rather than a round trip, which is the difference
/// between the list being browsable and being a series of waits.
```

A finished run's log is immutable, so warming writes it into the disk cache under its
job-keyed immutable entry and never needs to be repeated (invariant 12; the log pipeline
itself is on [the conversation and checks page](./conversation-and-checks.md)). Like diff
prefetch, log warming is pure speculation: nothing the reader asked for, everything the reader
is about to ask for.

### A lane of its own, behind everything

Warming is the lowest-priority work in the entire process, and its placement says so twice.
Its mailbox slot, `warm`, is the last field `Mailbox::pop` checks, behind even `prefetch`. And
it runs on a dedicated lane, `WorkerLane::Warm`, the `quinjet-warm` thread, so that its
network reads cannot occupy the GitHub metadata lane while a reader is waiting for a check
list or an interactive log. The worker test pinning the separation is named
`warming_logs_never_shares_a_lane_with_the_reads_a_reader_waits_on`.

Diff prefetch shares its lane with the interactive preview and relies on the pop order for
priority; log warming gets a whole thread and still sits last in its own mailbox. The
difference is workload shape: a diff batch is one bounded local Git invocation, while warming
up to 32 logs is a long train of network reads worth isolating completely.

### The generation stamp at send time

Warming needs a cancellation story that diff prefetch does not. A diff batch is a single
command that either runs or is overwritten in its slot before running. A warm command is one
command that *expands into up to 32 network reads inside the worker*, and a reader who closes
one pull request and opens another should not fund the old one's remaining reads. Slot
overwrite only helps before the command is popped; once running, it needs mid-flight abort.

The mechanism is one atomic counter, `warm_generation: Arc<AtomicU64>`, shared between the
send side and the warm thread. `GitWorker::send` stamps every warm command as the newest
generation at the moment of sending (`src/git/worker.rs:440`):

```rust
pub(crate) fn send(&self, mut command: WorkerCommand) -> bool {
    if let WorkerCommand::PrefetchCheckRunLogs { generation, .. } = &mut command {
        *generation = self.warm_generation.fetch_add(1, Ordering::SeqCst) + 1;
    }
```

`fetch_add` returns the previous value, so the stamp is `previous + 1`, exactly the value the
counter now holds. Two properties follow: every warm command carries a unique, strictly
increasing generation, and at any instant the counter equals the generation of the newest
command ever stamped.

### The cancellation closure

The warm thread checks that equality between steps (`src/git/worker.rs:499`):

```rust
/// The warm-up lane runs one job at a time and answers to nothing but its own
/// generation, so a pull request the reader has left stops costing requests as
/// soon as another one asks to be warmed.
fn run_warm_worker(
    repository: &Repository,
    mailbox: &Arc<SharedMailbox>,
    _events: &Sender<WorkerEvent>,
    generation: &Arc<AtomicU64>,
) {
    let mut session = Session::new(repository.clone_for_worker());
    while let Some(command) = next_command(mailbox) {
        match command {
            WorkerCommand::PrefetchCheckRunLogs {
                generation: mine,
                pull_request,
                checks,
            } => {
                drop(session.execute_with(
                    Command::WarmCheckRunLogs {
                        pull_request,
                        checks,
                    },
                    &mut |_| {},
                    &|| generation.load(Ordering::SeqCst) == mine,
                ));
            }
            WorkerCommand::Shutdown => break,
            _ => {}
        }
    }
}
```

The third argument to `execute_with` is a keep-going predicate the command implementation
polls between logs: `|| generation.load(Ordering::SeqCst) == mine`. While this job is the
newest ever stamped, the load returns `mine` and the predicate holds. The instant any newer
warm command is stamped, on the send side, possibly while this job is mid-download, the
counter moves past `mine`, the predicate fails at the next poll, and the job abandons its
remaining reads. The already-cached logs stay cached; the abandonment costs nothing and
returns nothing, which is why the events sender is deliberately unused (`_events`): warming
has no reply, its only output is disk-cache entries that future interactive reads will hit.

This is cooperative cancellation with the cheapest possible signal: no channel, no flag per
job, no join. One monotonically increasing integer encodes "everything older than the newest
request is abandoned", which is precisely the semantics warming wants, since only the current
pull request's warmth matters.

### Cancel at the source versus discard at the sink

Set side by side, the diff-prefetch and warm designs are the two canonical answers to stale
background work, chosen by where the waste would occur:

| | Diff prefetch batch | Log warm job |
|---|---|---|
| Unit of staleness | The prepared workspace | The newest warm request |
| Detection point | Reply arrival on the UI thread | Between steps inside the worker |
| Mechanism | Compare `workspace_generation` in the reply gate | Poll an `AtomicU64` in a keep-going predicate |
| Stale work wasted | At most one already-running Git invocation | At most one already-running log read |
| Why this choice | A batch is one short invocation; aborting it mid-run saves little and complicates the runner | A warm job is up to 32 network reads; running them all for an abandoned PR wastes real requests |

Discarding at the sink is simpler and suffices when the unit of work is small and bounded.
Canceling at the source pays for itself when one command fans out into many expensive steps.
Quinjet uses each exactly where its economics apply, and nothing anywhere blocks: both
mechanisms are wait-free reads of a number.

## Prefetch and the disk cache

Background fill and the on-disk cache are designed as one system, and several behaviors on
this page only make sense in light of the cache's key discipline, documented in full on
[the caching page](./caching.md). The short version and the prefetch-relevant consequences:

Every artifact prefetch produces or consumes is keyed by immutable identity:

| Entry | Key shape | Bound |
|---|---|---|
| Changed-file index | `pr-files-v1\n{merge_base}\n{head}` | 8 MiB |
| Numstat counts | `pr-numstat-v1\n{merge_base}\n{head}` | 8 MiB |
| API file counts | `pr-file-counts-v3\n{url}\n{number}\n{base}\n{head}` | 8 MiB |
| One file's patch | `pr-patch-v1\n{merge_base}\n{head}\n{path}` | 1 MiB |

Because merge base and head are commit OIDs, each key names content that can never change,
only become irrelevant; every entry is `CacheLife::Immutable` and lives until the
oldest-first pruning (128 MiB / 2,048 entries) reclaims it. Three consequences for prefetch:

**1. Prefetch is idempotent across sessions.** Reopening a pull request whose patches were
background-filled yesterday replays the same walk, but `diff_files` partitions almost every
file into the cached set and spawns few or no Git processes. The walk becomes a disk-to-memory
promotion pass. This is also what makes the CLI face cheap to repeat: a subcommand session
drops its prepared workspace on exit (invariant 14) and "relies on the immutable per-file
caches instead".

**2. Only whole truths are cached.** Truncated patches, truncated indexes, and partial API
count listings are all excluded from their caches at write time. The cache can therefore be
trusted blindly at read time, with no verification pass; the verification happened once, at
the only moment the data's completeness was knowable.

**3. The 1 MiB per-file ceiling shapes eviction economics.** The overwhelming majority of
patches fit and become permanent (until pruned) disk hits, so in-memory eviction is cheap to
reverse. The rare giant patch is exactly the one worth not letting monopolize a 128 MiB
budget shared by every repository the user touches.

The one prefetch artifact that is deliberately *not* durable is the prepared workspace itself:
the bare repository under the cache root's `tmp/` directory is removed on drop and swept after
24 hours, so a new session pays the fetch again (or takes the network-free local path). Why
the workspace is disposable while its outputs are durable is the subject of
[the PR workspace page](./pr-workspace.md).

## The life of one batch, end to end

The pieces above each own one decision. This section threads them into a single trace: one
batch, from the moment a fresh index arrives to the moment its patches are on screen, with
every thread boundary named. Three threads participate: the UI thread (event loop plus `App`),
the `quinjet-pr-preview` lane thread, and the unbounded crossbeam event channel between them.

### Stage 1: the index arrives and seeds the walk

On the UI thread, the `PullRequestIndex` reply passes its generation gate, and the handler
records `pull_request_workspace_generation = Some(generation)`, applies the index
(`apply_pull_request_index` resets the diff-side caches, syncs cursors, resets folds, and
rebuilds the composed all-files document from collapsed headers), and makes the first call to
`request_pull_request_prefetch`. At this instant the reader is already looking at a complete,
scrollable file tree with real counts; not one patch exists yet. This ordering, headers
render before any patch is even requested, is invariant 8's "collapsed headers first"
extended to pull requests.

### Stage 2: the batch is built from live state

`request_pull_request_prefetch` runs entirely on the UI thread against current state: the
in-flight flag is clear, the workspace exists, the prefetched set is empty, so the walk
computes its anchor from the Files tree and `sidebar_offset`, rotates the file list, and
admits files under the count and byte limits. The output is a single
`AppEffect::Git(Box::new(WorkerCommand::LoadPullRequestFileBatch { workspace_generation,
paths }))` and the flag flips to in-flight. Nothing has blocked; building a batch is a few
hundred iterations of integer arithmetic over data already in memory.

### Stage 3: dispatch, routing, and the slot

The event loop in `src/main.rs` dispatches the effect through `GitWorker::send`, which maps
the command through `worker_lane` to the `PullRequestPreview` lane, locks that lane's
mailbox, pushes (the command lands in the `prefetch` slot, overwriting any batch already
there), and notifies the lane's condvar. If the lane thread is idle it wakes immediately; if
it is busy running a preview, the batch waits in its slot and the pop order guarantees any
newly arrived preview will still go first.

### Stage 4: the lane translates and the session resolves

The lane thread's `run_worker` loop pops the command and performs a pure translation, adding
nothing but the reply envelope (`src/git/worker.rs:676`):

```rust
WorkerCommand::LoadPullRequestFileBatch {
    workspace_generation,
    paths,
} => WorkerEvent::PullRequestDiffBatch {
    workspace_generation,
    result: answer(
        session
            .execute(Command::PullRequestFileBatch {
                workspace: workspace_generation,
                paths,
            })
            .and_then(Outcome::pull_request_diff_batch),
    ),
},
```

The worker constructs no argv and holds no policy (invariant 1a); the real work happens in
`cli::Session`, the same command vocabulary the CLI subcommands execute. The session owns the
prepared workspace and resolves it by generation (`src/cli/command.rs:385`):

```rust
fn pull_request_workspace(&self, workspace: u64) -> Result<&PreparedPullRequest> {
    self.pull_request_diff
        .as_ref()
        .filter(|(prepared, _)| *prepared == workspace)
        .map(|(_, prepared)| prepared)
        .ok_or_else(|| anyhow::anyhow!("Pull-request diff workspace is no longer available"))
}
```

This is the workspace generation's second checkpoint, on the producer side: a batch that
names a workspace the session has already replaced fails cleanly here, before any Git
process spawns, with an error the app's retry-once logic will absorb. The reply-side gate
described earlier is thus a second line of defense, not the only one.

### Stage 5: Git runs, the patch splits, documents parse

`PreparedPullRequest::diff_files` partitions the paths against the disk cache, runs the one
`diff_selected_paths` invocation for the misses under the 8 MiB capped pipe, splits the
combined patch at its `diff --git` boundaries, attributes any truncation to the final
section, disk-caches the whole sections, and parses each into a `DiffDocument` with its full
pull-request details attached. All of this happens on the lane thread; the UI thread has
been rendering frames the whole time.

### Stage 6: the reply lands and the loop closes

The lane sends the `PullRequestDiffBatch` event into the channel. The event loop drains the
channel with `try_recv` each iteration, so the reply is picked up within one loop pass and
handed to `handle_worker_event` on the UI thread: workspace gate, in-flight flag cleared,
new documents backfill counts and enter the memory-accounted cache, the composed document
rebuilds if anything visible changed, and the handler's final act calls
`request_pull_request_prefetch` again, returning to stage 2 with the anchor recomputed from
wherever the reader has scrolled in the meantime.

The cycle repeats until a walk finds nothing to schedule. On the benchmark PR's 2,188 files
that is at most 69 batch cycles at 32 files each, fewer in practice as on-demand loads and
cache hits thin the remaining set, and each cycle's UI-thread cost is bounded by the batch
it applies, never by the size of the pull request.

## What a batch costs in a partial workspace

### Lazy blobs and the promisor round trip

When the pull request's commits are not present locally, the workspace behind
`diff_selected_paths` is a disposable bare repository whose history arrived through
`git fetch --filter=blob:none`: commits and trees, no file contents. Git records the remote
as a promisor, and any operation that needs a missing blob fetches it on demand (the full
mechanism is on [the shallow and partial clone page](../git-internals/shallow-and-partial-clone.md)).
A `git diff` over 32 paths therefore has a hidden line item: the first time each changed
file's old and new blobs are needed, they come over the network.

The saving grace is that Git batches those lazy fetches per invocation: one `git diff` with
32 pathspecs negotiates its missing blobs together rather than one round trip per file. The
prefetch batch is thus a network-batching unit as much as a process-batching unit, and the
byte budget indirectly bounds the network transfer a single batch can trigger, since the
blobs behind a patch are of the same order as the patch itself.

### How the cost moved across the stack

The stack relocated this blob cost twice, and the movement explains why prefetch got faster
without its own code changing much:

**1. Before PR #49, enumeration paid it all at once.** The index step ran a local
`git diff --numstat` to get per-file counts, and numstat needs file contents. In a
`blob:none` workspace that forced "one lazy blob download per changed file" for the entire
pull request, serially, inside one uninterruptible invocation, while the UI showed
"Enumerating changed files". The session's analysis ranked this the dominant load-time
cost.

**2. After PR #49, the counts come from metadata and the blobs wait for the batches.** The
pulls files endpoint answers the counts question with no blobs at all, so enumeration became
pure metadata and the first blob downloads moved into the prefetch batches, spread across
the walk, behind the reader, in viewport-anchored order. The same total transfer, paid where
nobody waits on it, and skipped entirely for files never fetched.

**3. After PR #55, locally present blobs cost nothing.** The workspace borrows the opened
repository's object store through Git's alternates mechanism, wired in
`prepare_pull_request_diff` immediately after the temporary repository is created. From
`src/git/github/mod.rs:1731`:

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

One line in `objects/info/alternates` makes every object lookup in the workspace fall
through to the opened repository's store before consulting the promisor remote. The opened
repository is never written (invariant 9 forbids any mutation of it); the borrow is
read-only by construction, and best-effort, since every failure path silently returns and
the workspace merely stays network-backed.

### The squash-merge case that motivated the borrow

The borrow exists because of a diagnosis during the session. The user ran the TUI against a
full local bun clone and still saw per-file loading crawls: "Everything is local. Why is it
taking so much time to load this for each of the files here?" The answer: bun squash-merged
the rewrite PR, so the PR's own head commit exists only on GitHub's `refs/pull/30412/head`
and never in the clone's `main`. Quinjet takes its network-free path only when *both* PR
commits are locally present, and it never fetches into the user's clone to make that true
(the invariant again), so it fell back to the disposable `blob:none` workspace and paid
network for blobs that were sitting on local disk under other refs, byte-identical because
a squash merge reproduces the same file contents in the merged tree.

The alternates borrow closes exactly that gap: the workspace's lazy blob reads now resolve
from the local store whenever the content exists anywhere in the opened repository, and only
genuinely novel blobs travel. The test pinning it,
`locally_available_pr_objects_avoid_disposable_fetches` (`src/git/github/mod.rs:2946`),
points the workspace at a deliberately unreachable base URL and asserts the whole prepare
and diff sequence completes anyway, proving no network dependency remains when objects are
local. For clones where even the head commit is fetchable, the session also recorded the
manual escape hatch: a one-time
`git fetch origin +refs/pull/30412/head:refs/remotes/origin/pr-30412` in the user's clone
makes both commits locally present and flips every later open onto the fully local,
workspace-free path.

## Failure modes and edge cases

The scheduler's simple shape, one boolean, one set, one walk, is only tenable because every
edge case has a specific, examined answer. This section is the catalog.

### The workspace disappears mid-batch

The prepared workspace is a disposable bare repository removed on drop. If the app replaces it
while a batch is running, the worker-side batch may fail (directory gone) or succeed against
the old workspace. Both outcomes are handled by the same two lines of the reply path: a
failure consumes the retry-once budget against a workspace that no longer matches and is then
gated out; a success arrives carrying the old workspace generation and is dropped at the gate.
Neither can touch current state.

### An empty batch and the walk's natural end

When the walk finds no schedulable file, every path cached, in flight, already prefetched, or
past the cap, `paths` is empty and `request_pull_request_prefetch` returns without emitting
anything or setting the in-flight flag. There is no explicit "done" state; completion is the
stable condition of every future walk finding nothing to do. New work reactivates it
naturally: a head change resets the bookkeeping, and an eviction does not (by design, as
covered in the memory section).

### Unknown paths in a batch

`diff_files` resolves each requested path against the workspace index and silently drops
misses. This tolerance exists for the same reason the generation gates do: the app and the
worker are asynchronous, and a batch built one reply before an index replacement may name
files the new index lacks. Dropping them is correct because the app-side walk will simply
never schedule them again if they are truly gone.

### A single file over every budget

A file whose estimate exceeds 6 MiB travels alone; if its true patch also exceeds 8 MiB, its
solitary batch returns one truncated document (the batch-of-one arm of the truncation rule).
The document renders with its truncation marked, is held in memory but never disk-cached, and
the path is marked prefetched so the walk moves on. Selecting the file later re-reads it
through the on-demand path, which produces the same honestly truncated document. There is no
configuration under which the file's full patch can be shown, and that is the system telling
the truth: an 8 MiB cap on a single read is the bound that keeps every other file's latency
predictable.

### The gap between 4,096 and 16,384

A pull request with more files than `MAX_PREFETCHED_PULL_REQUEST_FILES` but fewer than
`MAX_PR_PATHS` has a complete index and an incomplete background fill. Files past the
allowance behave exactly like the pre-prefetch world: collapsed headers with real counts,
patches on demand through the preview slot, disk-cached individually after first read. The
allowance is spent along the wrap-around order from the anchor, so which 4,096 files get the
warm treatment follows the reader's viewport, the best possible spend of a bounded budget.

### Esc, view switches, and who pays for abandonment

Closing the pull request (Esc in the PullRequests view) tears down the runtime, and with it
the workspace handle; the workspace's `Drop` removes the bare repository from disk. An
in-flight batch at that moment either fails against the missing directory or completes and is
gated out by generation. Switching *views* without closing the PR is gentler: the walk simply
continues, because a prepared workspace remains valid and patches keep landing in the side
cache. The anchor function returns 0 while the Files tree is not showing, so background fill
of an unwatched pull request proceeds in plain index order, still bounded by the same caps.

### Truncated index, honest totals

When the changed-file listing itself truncates (8 MiB of NUL-separated records or 16,384
entries), the index carries `truncated: true` and the total file count falls back to the
metadata's `changedFiles` figure, so the UI shows the honest total while listing what it has.
Prefetch operates on the listed subset only; the unlisted tail cannot be fetched because it
cannot be named. The repair for the listing's own torn last record, discarding bytes after
the final NUL so only whole records parse, mirrors the whole-line and whole-record trims on
every other capped read in the module.

### Two sessions, one cache

Two Quinjet processes (a TUI and a CLI invocation, say) can prefetch the same pull request
concurrently. The disk cache is safe under that race by construction: entries are written to
unique temp names and renamed into place atomically, and both writers produce identical bytes
for identical immutable keys, so the loser of the rename race overwrites the winner with the
same content. The in-memory caches are per-process and never shared. The one shared mutable
resource, the repository index, is not touched by any read path (`GIT_OPTIONAL_LOCKS=0`,
invariant 13).

### What prefetch never does

A closing inventory of deliberate absences, each a bug class removed by construction:

- It never runs before a workspace exists, so "no GitHub command is queued at startup or
  merely by opening the PR tab" (invariant 3) extends to Git work as well.
- It never touches `App::document`, so it cannot repaint the pane the reader is reading.
- It never writes the opened repository: all Git work happens in the disposable workspace or
  read-only against local objects through the alternates link (invariant 9).
- It never reports errors to the reader: speculative work fails into silence and leaves the
  on-demand path intact.
- It never re-fetches a path within one workspace, and never remembers anything across
  workspaces except through the immutable disk cache.

## Measured behavior on the benchmark

Every number below is quoted from the session record for the merged stack, measured against
oven-sh/bun pull request 30412 (2,188 files, +1,009,257 added lines) using the cold-cache
isolation setup (`QUINJET_CACHE_DIR` pointed at a throwaway directory) described on
[the benchmarking page](../benchmarking.md). The numbers measure the whole loading pipeline,
of which prefetch is one stage; they are reproduced here because they bracket what the
batching and ordering work actually bought.

First verification round, at the top of the original five-PR stack:

- Metadata: "Metadata in 1.7s" (`pr view` against the benchmark PR, cold).
- Full index with counts: "The rewrite PR enumerates all 2,188 files with real counts in
  18.5s cold." (includes workspace preparation and fetch)
- Warm re-run of the index: 0.04s.
- Single-file patches: 0.1s.

Second verification round, after the adversarial-review fixes (among them the livelock fix
and the counts cache-key fix) and the restack:

- "Final numbers on the bun PR: cold index 6.3s, warm 0.04s, conversation 26s with the honest
  truncation notice."

After a local install of the final build, with a warm real cache:

- "Smoke-tested from the bun clone: `q pr files 30412` lists all 2,188 files of the 1M-line
  rewrite PR in 1.4s."

Reading them against this page's mechanisms: the cold-to-warm collapse from seconds to 0.04s
is the immutable disk cache answering the index and counts questions without any subprocess
or network work, the same property that makes a second session's prefetch walk nearly free.
The 0.1s single-file patch is the on-demand path against a workspace whose blobs the earlier
reads already materialized. The 18.5s-to-6.3s cold improvement landed with the review-fix
round. And the progressive behavior that these end-state numbers do not capture, what the
reader sees during those cold seconds, headers first, visible files next, the rest streaming
in, is the subject of [the progressive loading page](../rendering/progressive-loading.md).

## Alternatives considered and rejected

The merged design is one point in a space the session explicitly or implicitly explored.
The instructive rejections:

**Pure on-demand loading.** The zero-speculation baseline: every file loads when selected.
Rejected by the founding complaint itself, the per-file "Loading diff…" crawl; a reader
paging through a review would pay a subprocess (or a network round trip) per keystroke.

**Parallel per-file fetches.** Spawn many concurrent `git diff` processes and fill the cache
by fan-out. Rejected on three grounds: fixed process cost paid per file instead of per 32
files; contention on one object store and, in a partial workspace, competing lazy blob
downloads; and the loss of the single-lane serialization that keeps the workspace free of
concurrent Git invocations. Batching gets the throughput without any of it.

**One giant diff for everything.** The opposite extreme: `git diff` over the whole range with
no pathspec, split the result once. Rejected by arithmetic: the benchmark PR's combined patch
is far past the 8 MiB read cap, so the single invocation would truncate almost everything,
and no cap raise survives contact with a million-line PR. Bounded batches are the only shape
under which the kill-on-cap contract and full coverage coexist.

**Exact sizes instead of estimates.** Sizing batches by true patch bytes would need those
bytes, which is the work being scheduled; any cheaper proxy (object sizes via `cat-file`)
would cost another subprocess pass per file and still miss diff-specific overheads. Line
counts arrive free with the index (API counts on the workspace path, one numstat pass
locally), and the 2 MiB safety margin absorbs the model error everywhere except the
single-line-giant case, which no line-count model survives and the truncation fallback
handles.

**A persistent priority queue of pending files.** An explicit scheduler object holding a
sorted plan, updated as the viewport moves. Rejected by the invalidation burden: every scroll,
fold, selection, eviction, and head change would need to patch the plan, and every bug in
that patching would strand or duplicate files. Recomputing the batch from live state at each
boundary makes the plan trivially consistent with reality at the only moments it is read.
The livelock fix hardened the one weakness recomputation has (progress must be provable),
after which statelessness is strictly the simpler correct design.

**Keeping smallest-first alongside the anchor.** A hybrid, anchor first then
smallest-of-the-rest, was implicitly on the table when #55 removed the tiers. Rejected for
predictability: after the anchored region, wrap-around index order fills the tree in the
order the reader sees it, top to bottom, while a size-sorted remainder would fill in visually
random order for a marginal average-latency gain that the raised 4,096 cap already dwarfs.

**Fetching blobs eagerly for mid-size PRs.** Fetch the PR head *with* blobs so later diffs
never lazy-load. Recorded in the session as proposed but not built; the partial-clone ladder
with API counts (PR #49) removed the dominant blob storm, and the alternates borrow (PR #55)
made locally present objects free, leaving eager blob fetch as complexity without a
demonstrated win.

## The invariants in force

The prefetch subsystem is governed by five `ARCHITECTURE.md` invariants; each has appeared
above in context, and they are collected here as the contract a future change must keep.

- *Invariant 3* (placement): "Background diff prefetch occupies its own mailbox slot behind
  the preview slot, so a queued batch can never displace the preview a reader is waiting
  for."
- *Invariant 5* (policy and bounds): "Background prefetch walks the whole index up to 4,096
  files, starting at the file the Files tree is showing and wrapping around the rest in
  order, sizes each batch by per-file count estimates to stay under the 8 MiB patch read,
  and backfills a header's counts from its arrived patch when GitHub could not report them."
- *Invariant 6* (the caps beneath): capped pipes that kill the child on overflow, so a sizing
  mistake costs bounded transfer, never unbounded memory.
- *Invariant 10a* (staleness): batched background reads are "keyed to the prepared workspace
  rather than to a preview generation, so they can never invalidate a reader's own request",
  with one Git invocation per batch split at its `diff --git` boundaries.
- *Invariant 12b* (the warm sibling): settled run logs warm "in the background, on a
  dedicated lane behind every interactive read, capped at 32 stable GitHub job identities."

Together they compose the page's one-sentence summary: speculative work runs strictly behind
interactive work, in bounded bites, ordered by where the reader is looking, keyed by
immutable identity, and abandoned by comparison rather than cancellation.

## Related pages

- [The PR workspace](./pr-workspace.md): the prepared bare repository every batch diffs
  against, its fetch ladder, and the alternates borrow.
- [API strategy](./api-strategy.md): the pulls files endpoint that supplies the counts the
  estimates run on, and every gh-side cap.
- [Caching](./caching.md): the immutable-key discipline that makes prefetched patches durable
  and re-reads free.
- [Conversation and checks](./conversation-and-checks.md): the check logs the warm lane
  fills, and their own caps.
- [The diff pipeline](../diff/pipeline.md): the unified diff format the section splitter
  parses and the document model batches produce.
- [Progressive loading](../rendering/progressive-loading.md): what the reader sees while the
  walk runs, stage by stage, on the benchmark PR.
- [Concurrency](../rendering/concurrency.md): the full lane and mailbox model this page's
  slot lives inside.
- [The viewport](../rendering/viewport.md): the sidebar offset and free-scroll state the
  anchor reads.
- [Benchmarking](../benchmarking.md): the bun#30412 setup behind every quoted number.
- [The technique catalog](../techniques.md): byte-budgeted batching, smallest-first
  scheduling, generation tagging, and background warming as reusable patterns.
