# Optimization Techniques

The performance model begins with latency classes and explicit budgets, then assigns
work to the narrowest layer that can perform it correctly.

Use this catalog with the [benchmarking guide](./benchmarking.md) and the four
group hubs for [Git internals](./git-internals/README.md),
[diff engineering](./diff/README.md), [GitHub optimization](./github/README.md),
and [rendering](./rendering/README.md).

This chapter documents the current implementation, the Git or systems concept behind it,
the cost it avoids, the correctness condition that constrains it, and the evidence
expected before it changes. It is intentionally detailed enough to serve design review,
regression investigation, and onboarding rather than only feature discovery.

The implementation is the source of truth. Numeric bounds in this reference describe the
current tree and should change in the same pull request as the constants or algorithms
they explain. Measurements should distinguish cold setup, first useful output, steady
interaction, and eventual completion because those phases optimize different user
experiences.

## Source map

- [`ARCHITECTURE.md`](../../ARCHITECTURE.md)
- [`src/app.rs`](../../src/app.rs)
- [`src/main.rs`](../../src/main.rs)
- [`src/git/worker.rs`](../../src/git/worker.rs)

## Operational contract

1. Input handling has a frame-scale latency target and therefore mutates only memory.

2. Local Git reads are isolated from network-facing GitHub reads by worker lanes.

3. Preview debounce collapses selection churn before it becomes subprocess work.

4. Fixed mailbox slots make repeated key input consume constant queue space.

5. Viewport work scales with visible rows instead of total document rows.

6. Memory budgets cover raw output, parsed documents, cached patches, and lists
separately.

7. The first useful view is optimized independently from complete background fill.

8. Failure paths preserve previous snapshots instead of replacing them with emptiness.

## Git and systems foundations

### 1. Refs and object IDs

A ref such as a branch is a movable name. An object ID identifies immutable content.
Quinjet uses refs for user intent and resolved object IDs for workspaces and persistent
cache identity.

For performance model and budgets, this model matters because input handling has a
frame-scale latency target and therefore mutates only memory. The boundary is semantic
as well as computational: an optimization is invalid if it answers a cheaper but
different Git question.

### 2. The three trees

HEAD, the index, and the working tree represent committed, staged, and filesystem state.
Separate comparisons between these trees are what produce staged and unstaged views.

For performance model and budgets, this model matters because local git reads are
isolated from network-facing github reads by worker lanes. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

### 3. Merge-base semantics

A pull-request diff starts at the best common ancestor and ends at the head commit. This
isolates the contribution from unrelated changes later added to the base branch.

For performance model and budgets, this model matters because preview debounce collapses
selection churn before it becomes subprocess work. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 4. Path-limited diff

A pathspec narrows diff output after the comparison endpoints are fixed. It preserves
Git semantics while avoiding patch generation for files the current interaction does not
need.

For performance model and budgets, this model matters because fixed mailbox slots make
repeated key input consume constant queue space. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 5. Machine protocols

NUL-delimited status and diff-index records separate paths without relying on quoting or
locale. Explicit pretty-format delimiters do the same for commit history fields and
records.

For performance model and budgets, this model matters because viewport work scales with
visible rows instead of total document rows. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 6. Partial clone

Blob filtering permits commits and trees to arrive without every file body. This is
valuable only when later commands also avoid accidentally demanding all omitted blobs.

For performance model and budgets, this model matters because memory budgets cover raw
output, parsed documents, cached patches, and lists separately. The boundary is semantic
as well as computational: an optimization is invalid if it answers a cheaper but
different Git question.

### 7. Pack storage

Loose objects and packfiles are storage details behind the same object database.
Delegating to Git lets Quinjet benefit from delta compression and repository maintenance
without reimplementing them.

For performance model and budgets, this model matters because the first useful view is
optimized independently from complete background fill. The boundary is semantic as well
as computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 8. Diffcore

Git transforms raw tree differences through rename detection and other diffcore stages
before formatting a patch. Quinjet consumes the resulting machine and patch formats
instead of approximating those rules.

For performance model and budgets, this model matters because failure paths preserve
previous snapshots instead of replacing them with emptiness. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

## Representative Git command shapes

### Command 1: Bounded history page

```bash
git log --topo-order --decorate=short --no-color --skip=N --max-count=N --format=FORMAT REV --
```

This is a conceptual command shape rather than copyable internal tracing output. An
explicit revision and page bound avoid ambient HEAD races and repository-sized output.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 2: Changed-path index

```bash
git diff --name-status -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
path and status index is cheaper to acquire and parse than full patch bodies. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 3: Line-count index

```bash
git diff --numstat -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
same revision range supplies additions and deletions without syntax or hunk parsing.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 4: Selected-path patch

```bash
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 BASE HEAD -- PATH
```

This is a conceptual command shape rather than copyable internal tracing output. The
pathspec makes one interaction pay only for the file or batch it requested. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 5: Local merge base

```bash
git merge-base BASE_OID HEAD_OID
```

This is a conceptual command shape rather than copyable internal tracing output. The
common ancestor defines pull-request contribution semantics when both tips exist
locally. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

## Implementation walkthrough

### Mechanism 1: Input handling has a frame-scale latency target and therefore mutates only memory

Mechanics. Input handling has a frame-scale latency target and therefore mutates only
memory. The relevant flow begins in src/app.rs and crosses only the layers needed to
preserve the shared command and session boundary.

Cost model. The mechanism is reviewed against peak memory, subprocess count, and failure
degradation. A claim about speed is incomplete unless it identifies which cost moves,
which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
monorepo with many changed paths. Identity, ordering, truncation, and cache disposition
must remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `src/app.rs`, exercise WorkerLane and Mailbox routing tests,
and record steady-state frame cost. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 2: Local Git reads are isolated from network-facing GitHub reads by worker lanes

Mechanics. Local Git reads are isolated from network-facing GitHub reads by worker
lanes. The relevant flow begins in src/main.rs and crosses only the layers needed to
preserve the shared command and session boundary.

Cost model. The mechanism is reviewed against network transfer, cache correctness, and
user-visible continuity. A claim about speed is incomplete unless it identifies which
cost moves, which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
pull request with generated files. Identity, ordering, truncation, and cache disposition
must remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `src/main.rs`, exercise MAX_PULL_REQUEST_DOCUMENT_BYTES
eviction test, and record bytes accepted from child stdout. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 3: Preview debounce collapses selection churn before it becomes subprocess work

Mechanics. Preview debounce collapses selection churn before it becomes subprocess work.
The relevant flow begins in src/git/worker.rs and crosses only the layers needed to
preserve the shared command and session boundary.

Cost model. The mechanism is reviewed against subprocess count, concurrency ordering,
and latency. A claim about speed is incomplete unless it identifies which cost moves,
which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
deeply diverged branch. Identity, ordering, truncation, and cache disposition must
remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `src/git/worker.rs`, exercise viewport-scoped rendering tests,
and record number of Git and gh processes. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 4: Fixed mailbox slots make repeated key input consume constant queue space

Mechanics. Fixed mailbox slots make repeated key input consume constant queue space. The
relevant flow begins in ARCHITECTURE.md and crosses only the layers needed to preserve
the shared command and session boundary.

Cost model. The mechanism is reviewed against cache correctness, failure degradation,
and peak memory. A claim about speed is incomplete unless it identifies which cost
moves, which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
slow or unavailable network. Identity, ordering, truncation, and cache disposition must
remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `ARCHITECTURE.md`, exercise oversized subprocess output kill
test, and record maximum retained document bytes. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 5: Viewport work scales with visible rows instead of total document rows

Mechanics. Viewport work scales with visible rows instead of total document rows. The
relevant flow begins in src/app.rs and crosses only the layers needed to preserve the
shared command and session boundary.

Cost model. The mechanism is reviewed against concurrency ordering, user-visible
continuity, and network transfer. A claim about speed is incomplete unless it identifies
which cost moves, which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under
rapid keyboard navigation. Identity, ordering, truncation, and cache disposition must
remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `src/app.rs`, exercise PREVIEW_DEBOUNCE and polling constants,
and record cache hit identity and disposition. Compare the cold and warm paths because
cache and workspace reuse intentionally make them different.

### Mechanism 6: Memory budgets cover raw output, parsed documents, cached patches, and lists separately

Mechanics. Memory budgets cover raw output, parsed documents, cached patches, and lists
separately. The relevant flow begins in src/main.rs and crosses only the layers needed
to preserve the shared command and session boundary.

Cost model. The mechanism is reviewed against failure degradation, latency, and
subprocess count. A claim about speed is incomplete unless it identifies which cost
moves, which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
linked Git worktree. Identity, ordering, truncation, and cache disposition must remain
visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `src/main.rs`, exercise WorkerLane and Mailbox routing tests,
and record stale reply rejection count. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 7: The first useful view is optimized independently from complete background fill

Mechanics. The first useful view is optimized independently from complete background
fill. The relevant flow begins in src/git/worker.rs and crosses only the layers needed
to preserve the shared command and session boundary.

Cost model. The mechanism is reviewed against user-visible continuity, peak memory, and
cache correctness. A claim about speed is incomplete unless it identifies which cost
moves, which phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
cold cache followed by a warm cache. Identity, ordering, truncation, and cache
disposition must remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `src/git/worker.rs`, exercise MAX_PULL_REQUEST_DOCUMENT_BYTES
eviction test, and record visible continuity after failure. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 8: Failure paths preserve previous snapshots instead of replacing them with emptiness

Mechanics. Failure paths preserve previous snapshots instead of replacing them with
emptiness. The relevant flow begins in ARCHITECTURE.md and crosses only the layers
needed to preserve the shared command and session boundary.

Cost model. The mechanism is reviewed against latency, network transfer, and concurrency
ordering. A claim about speed is incomplete unless it identifies which cost moves, which
phase improves, and whether another resource grows.

Correctness. The output must remain the answer to the same repository question under a
small local repository. Identity, ordering, truncation, and cache disposition must
remain visible wherever they affect what the reader can infer.

Failure behavior. If the optimized path cannot complete, the code should preserve the
last authoritative snapshot, return a bounded partial document with an explicit marker,
or report a scoped error. It must not silently present missing work as an empty
repository result.

Review evidence. Inspect `ARCHITECTURE.md`, exercise viewport-scoped rendering tests,
and record time to first useful rows. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Input handling
has a frame-scale latency target and therefore mutates only memory. Capture steady-state
frame cost before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 2: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Input
handling has a frame-scale latency target and therefore mutates only memory. Capture
bytes accepted from child stdout before changing the implementation, then repeat with
the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 3: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Input
handling has a frame-scale latency target and therefore mutates only memory. Capture
number of Git and gh processes before changing the implementation, then repeat with the
same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 4: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Input handling
has a frame-scale latency target and therefore mutates only memory. Capture maximum
retained document bytes before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 5: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Input
handling has a frame-scale latency target and therefore mutates only memory. Capture
cache hit identity and disposition before changing the implementation, then repeat with
the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 6: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Input handling
has a frame-scale latency target and therefore mutates only memory. Capture stale reply
rejection count before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 7: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Input handling has
a frame-scale latency target and therefore mutates only memory. Capture visible
continuity after failure before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 8: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Input handling has a frame-scale latency target and therefore mutates only memory.
Capture time to first useful rows before changing the implementation, then repeat with
the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 9: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Local Git reads
are isolated from network-facing GitHub reads by worker lanes. Capture steady-state
frame cost before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 10: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Local
Git reads are isolated from network-facing GitHub reads by worker lanes. Capture bytes
accepted from child stdout before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 11: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Local
Git reads are isolated from network-facing GitHub reads by worker lanes. Capture number
of Git and gh processes before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 12: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Local Git reads
are isolated from network-facing GitHub reads by worker lanes. Capture maximum retained
document bytes before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 13: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Local Git
reads are isolated from network-facing GitHub reads by worker lanes. Capture cache hit
identity and disposition before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 14: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Local Git reads
are isolated from network-facing GitHub reads by worker lanes. Capture stale reply
rejection count before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 15: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Local Git reads are
isolated from network-facing GitHub reads by worker lanes. Capture visible continuity
after failure before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 16: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Local Git reads are isolated from network-facing GitHub reads by worker lanes. Capture
time to first useful rows before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 17: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Preview debounce
collapses selection churn before it becomes subprocess work. Capture steady-state frame
cost before changing the implementation, then repeat with the same repository identity
and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 18: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is:
Preview debounce collapses selection churn before it becomes subprocess work. Capture
bytes accepted from child stdout before changing the implementation, then repeat with
the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 19: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is:
Preview debounce collapses selection churn before it becomes subprocess work. Capture
number of Git and gh processes before changing the implementation, then repeat with the
same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 20: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Preview debounce
collapses selection churn before it becomes subprocess work. Capture maximum retained
document bytes before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 21: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Preview
debounce collapses selection churn before it becomes subprocess work. Capture cache hit
identity and disposition before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 22: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Preview
debounce collapses selection churn before it becomes subprocess work. Capture stale
reply rejection count before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 23: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Preview debounce
collapses selection churn before it becomes subprocess work. Capture visible continuity
after failure before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 24: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Preview debounce collapses selection churn before it becomes subprocess work. Capture
time to first useful rows before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 25: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Fixed mailbox
slots make repeated key input consume constant queue space. Capture steady-state frame
cost before changing the implementation, then repeat with the same repository identity
and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 26: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Fixed
mailbox slots make repeated key input consume constant queue space. Capture bytes
accepted from child stdout before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 27: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Fixed
mailbox slots make repeated key input consume constant queue space. Capture number of
Git and gh processes before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 28: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Fixed mailbox
slots make repeated key input consume constant queue space. Capture maximum retained
document bytes before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 29: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Fixed
mailbox slots make repeated key input consume constant queue space. Capture cache hit
identity and disposition before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 30: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Fixed mailbox
slots make repeated key input consume constant queue space. Capture stale reply
rejection count before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 31: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Fixed mailbox slots
make repeated key input consume constant queue space. Capture visible continuity after
failure before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 32: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Fixed mailbox slots make repeated key input consume constant queue space. Capture time
to first useful rows before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

## Failure modes and review responses

### Risk 1

Optimizing throughput can make the first visible result slower.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a pull request with generated
files. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 2

A byte cap after read completion does not cap peak allocation.

Review response. Locate the acquisition boundary in `src/main.rs`, identify the complete
cache or generation key, and prove the outcome under a deeply diverged branch. Prefer a
test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 3

One global queue permits head-of-line blocking across unrelated tasks.

Review response. Locate the acquisition boundary in `src/git/worker.rs`, identify the
complete cache or generation key, and prove the outcome under a slow or unavailable
network. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 4

A frame cache with an incomplete key displays stale geometry.

Review response. Locate the acquisition boundary in `ARCHITECTURE.md`, identify the
complete cache or generation key, and prove the outcome under rapid keyboard navigation.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 5

A retry loop without a strict attempt limit converts failure into load.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a linked Git worktree. Prefer a
test that asserts state and bounds over one that depends on wall-clock timing.

## Measurement playbook

1. Cold start measures repository discovery, process startup, parsing, and first paint
with no reusable cache entry.

2. Warm start measures immutable cache and prepared-workspace reuse without hiding
correctness checks.

3. First useful output ends when the requested file names, counts, or visible patch rows
can be read.

4. Complete fill ends when bounded background work has populated every eligible item.

5. Interaction latency measures input-to-frame delay while background work is active.

6. Peak retained memory includes raw bytes, parsed rows, layout rows, path indexes, and
cache staging buffers.

7. Process count separates setup commands, path-scoped reads, API pages, polls, and
retries.

8. Network bytes distinguish metadata, commit and tree transfer, and on-demand blob
transfer.

## Verification evidence

Evidence 1. PREVIEW_DEBOUNCE and polling constants. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 2. WorkerLane and Mailbox routing tests. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 3. MAX_PULL_REQUEST_DOCUMENT_BYTES eviction test. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 4. viewport-scoped rendering tests. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 5. oversized subprocess output kill test. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

## Official background reading

- [Git repository layout](https://git-scm.com/docs/gitrepository-layout)
- [Git object glossary](https://git-scm.com/docs/gitglossary)
- [Git diff formats](https://git-scm.com/docs/diff-format)
- [Git diffcore](https://git-scm.com/docs/gitdiffcore)
- [Git status porcelain](https://git-scm.com/docs/git-status#_porcelain_format_version_2)
- [Git merge-base](https://git-scm.com/docs/git-merge-base)
- [Git partial clone](https://git-scm.com/docs/partial-clone)
- [Git revisions](https://git-scm.com/docs/gitrevisions)

## Optimization audit matrix

The matrix is deliberately exhaustive. Each row combines a concrete mechanism, operating
context, review lens, and observable signal. It is a checklist for design reviews and
regression work, not a claim that every combination needs a standalone benchmark.

| ID | Mechanism | Review condition | Evidence to capture |
| ---: | --- | --- | --- |
| 1 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record time to first useful rows |
| 2 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Input handling has a frame-scale latency target and therefore mutates only memory | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Input handling has a frame-scale latency target and therefore mutates only memory | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Input handling has a frame-scale latency target and therefore mutates only memory | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Input handling has a frame-scale latency target and therefore mutates only memory | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Input handling has a frame-scale latency target and therefore mutates only memory | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Input handling has a frame-scale latency target and therefore mutates only memory | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Input handling has a frame-scale latency target and therefore mutates only memory | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Input handling has a frame-scale latency target and therefore mutates only memory | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record time to first useful rows |
| 514 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record steady-state frame cost |
| 515 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record stale reply rejection count |
| 520 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a small local repository | Record visible continuity after failure |
| 521 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 1015 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 1016 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 1017 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 1018 | Local Git reads are isolated from network-facing GitHub reads by worker lanes | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
