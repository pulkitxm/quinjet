# Concurrency, Generations, Mailboxes, and Worker Lanes

Quinjet preserves input responsiveness and reply correctness with generation-tagged
commands, fixed coalescing slots, priority rules, and separate latency lanes.

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

- [`src/git/worker.rs`](../../../src/git/worker.rs)
- [`src/app.rs`](../../../src/app.rs)
- [`src/cli/command.rs`](../../../src/cli/command.rs)
- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)

## Operational contract

1. Each request carries the generation under which its user intent was created.

2. App state increments generations before replacement work can reply.

3. Worker events return the same tag and stale events are ignored.

4. Mailbox slots replace obsolete refreshes and previews instead of extending a queue.

5. Ordered user mutations remain serialized and outrank speculative reads.

6. Local preview, GitHub metadata, PR preview, and warming work occupy separate lanes.

7. Prefetch has a distinct slot behind selected-file preview within its lane.

8. A coalesced in-flight stream remains due so completion triggers the missed refresh.

## Git and systems foundations

### 1. The three trees

HEAD, the index, and the working tree represent committed, staged, and filesystem state.
Separate comparisons between these trees are what produce staged and unstaged views.

For generations, mailboxes, and worker lanes, this model matters because each request
carries the generation under which its user intent was created. The boundary is semantic
as well as computational: an optimization is invalid if it answers a cheaper but
different Git question.

### 2. Merge-base semantics

A pull-request diff starts at the best common ancestor and ends at the head commit. This
isolates the contribution from unrelated changes later added to the base branch.

For generations, mailboxes, and worker lanes, this model matters because app state
increments generations before replacement work can reply. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

### 3. Path-limited diff

A pathspec narrows diff output after the comparison endpoints are fixed. It preserves
Git semantics while avoiding patch generation for files the current interaction does not
need.

For generations, mailboxes, and worker lanes, this model matters because worker events
return the same tag and stale events are ignored. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 4. Machine protocols

NUL-delimited status and diff-index records separate paths without relying on quoting or
locale. Explicit pretty-format delimiters do the same for commit history fields and
records.

For generations, mailboxes, and worker lanes, this model matters because mailbox slots
replace obsolete refreshes and previews instead of extending a queue. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

### 5. Partial clone

Blob filtering permits commits and trees to arrive without every file body. This is
valuable only when later commands also avoid accidentally demanding all omitted blobs.

For generations, mailboxes, and worker lanes, this model matters because ordered user
mutations remain serialized and outrank speculative reads. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

### 6. Pack storage

Loose objects and packfiles are storage details behind the same object database.
Delegating to Git lets Quinjet benefit from delta compression and repository maintenance
without reimplementing them.

For generations, mailboxes, and worker lanes, this model matters because local preview,
github metadata, pr preview, and warming work occupy separate lanes. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

### 7. Diffcore

Git transforms raw tree differences through rename detection and other diffcore stages
before formatting a patch. Quinjet consumes the resulting machine and patch formats
instead of approximating those rules.

For generations, mailboxes, and worker lanes, this model matters because prefetch has a
distinct slot behind selected-file preview within its lane. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

### 8. Index locking

Many mutations lock and rewrite the index. Read-only commands set GIT_OPTIONAL_LOCKS to
zero so background inspection avoids optional lock traffic and interference.

For generations, mailboxes, and worker lanes, this model matters because a coalesced
in-flight stream remains due so completion triggers the missed refresh. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

## Representative Git command shapes

### Command 1: Blob-filtered fetch

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=N REMOTE REFSPEC
```

This is a conceptual command shape rather than copyable internal tracing output. Commit
and tree history can arrive without every changed blob body. Quinjet constructs the real
argv directly and applies operation-specific output caps and repository context in the
implementation.

### Command 2: Revision validation

```bash
git rev-parse --verify --quiet REVISION^{commit}
```

This is a conceptual command shape rather than copyable internal tracing output. Git
validates object type and resolves revision syntax without a checkout. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 3: Status snapshot

```bash
git status --porcelain=v2 --branch -z --untracked-files=all --ignore-submodules=none
```

This is a conceptual command shape rather than copyable internal tracing output.
Porcelain version 2 and NUL records provide a stable byte protocol for branch and path
state. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

### Command 4: Bounded history page

```bash
git log --topo-order --decorate=short --no-color --skip=N --max-count=N --format=FORMAT REV --
```

This is a conceptual command shape rather than copyable internal tracing output. An
explicit revision and page bound avoid ambient HEAD races and repository-sized output.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 5: Changed-path index

```bash
git diff --name-status -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
path and status index is cheaper to acquire and parse than full patch bodies. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

## Implementation walkthrough

### Mechanism 1: Each request carries the generation under which its user intent was created

Mechanics. Each request carries the generation under which its user intent was created.
The relevant flow begins in src/app.rs and crosses only the layers needed to preserve
the shared command and session boundary.

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

Review evidence. Inspect `src/app.rs`, exercise user operation priority test, and record
steady-state frame cost. Compare the cold and warm paths because cache and workspace
reuse intentionally make them different.

### Mechanism 2: App state increments generations before replacement work can reply

Mechanics. App state increments generations before replacement work can reply. The
relevant flow begins in src/cli/command.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/cli/command.rs`, exercise lane isolation tests, and record
bytes accepted from child stdout. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 3: Worker events return the same tag and stale events are ignored

Mechanics. Worker events return the same tag and stale events are ignored. The relevant
flow begins in ARCHITECTURE.md and crosses only the layers needed to preserve the shared
command and session boundary.

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

Review evidence. Inspect `ARCHITECTURE.md`, exercise stale preview and metadata
rejection tests, and record number of Git and gh processes. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 4: Mailbox slots replace obsolete refreshes and previews instead of extending a queue

Mechanics. Mailbox slots replace obsolete refreshes and previews instead of extending a
queue. The relevant flow begins in src/git/worker.rs and crosses only the layers needed
to preserve the shared command and session boundary.

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

Review evidence. Inspect `src/git/worker.rs`, exercise refresh replay after in-flight
completion, and record maximum retained document bytes. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 5: Ordered user mutations remain serialized and outrank speculative reads

Mechanics. Ordered user mutations remain serialized and outrank speculative reads. The
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

Review evidence. Inspect `src/app.rs`, exercise mailbox coalescing test, and record
cache hit identity and disposition. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 6: Local preview, GitHub metadata, PR preview, and warming work occupy separate lanes

Mechanics. Local preview, GitHub metadata, PR preview, and warming work occupy separate
lanes. The relevant flow begins in src/cli/command.rs and crosses only the layers needed
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

Review evidence. Inspect `src/cli/command.rs`, exercise user operation priority test,
and record stale reply rejection count. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 7: Prefetch has a distinct slot behind selected-file preview within its lane

Mechanics. Prefetch has a distinct slot behind selected-file preview within its lane.
The relevant flow begins in ARCHITECTURE.md and crosses only the layers needed to
preserve the shared command and session boundary.

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

Review evidence. Inspect `ARCHITECTURE.md`, exercise lane isolation tests, and record
visible continuity after failure. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 8: A coalesced in-flight stream remains due so completion triggers the missed refresh

Mechanics. A coalesced in-flight stream remains due so completion triggers the missed
refresh. The relevant flow begins in src/git/worker.rs and crosses only the layers
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

Review evidence. Inspect `src/git/worker.rs`, exercise stale preview and metadata
rejection tests, and record time to first useful rows. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Each request
carries the generation under which its user intent was created. Capture steady-state
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

Start with a monorepo with many changed paths. The mechanism under inspection is: Each
request carries the generation under which its user intent was created. Capture bytes
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

### Scenario 3: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Each
request carries the generation under which its user intent was created. Capture number
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

### Scenario 4: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Each request
carries the generation under which its user intent was created. Capture maximum retained
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

### Scenario 5: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Each
request carries the generation under which its user intent was created. Capture cache
hit identity and disposition before changing the implementation, then repeat with the
same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 6: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Each request
carries the generation under which its user intent was created. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Each request
carries the generation under which its user intent was created. Capture visible
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
Each request carries the generation under which its user intent was created. Capture
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

### Scenario 9: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: App state
increments generations before replacement work can reply. Capture steady-state frame
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

### Scenario 10: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: App
state increments generations before replacement work can reply. Capture bytes accepted
from child stdout before changing the implementation, then repeat with the same
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

Start with a pull request with generated files. The mechanism under inspection is: App
state increments generations before replacement work can reply. Capture number of Git
and gh processes before changing the implementation, then repeat with the same
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

Start with a deeply diverged branch. The mechanism under inspection is: App state
increments generations before replacement work can reply. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: App state
increments generations before replacement work can reply. Capture cache hit identity and
disposition before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 14: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: App state
increments generations before replacement work can reply. Capture stale reply rejection
count before changing the implementation, then repeat with the same repository identity
and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 15: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: App state
increments generations before replacement work can reply. Capture visible continuity
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

Start with a cold cache followed by a warm cache. The mechanism under inspection is: App
state increments generations before replacement work can reply. Capture time to first
useful rows before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 17: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Worker events
return the same tag and stale events are ignored. Capture steady-state frame cost before
changing the implementation, then repeat with the same repository identity and selection
path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 18: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Worker
events return the same tag and stale events are ignored. Capture bytes accepted from
child stdout before changing the implementation, then repeat with the same repository
identity and selection path after the change.

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
Worker events return the same tag and stale events are ignored. Capture number of Git
and gh processes before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 20: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Worker events
return the same tag and stale events are ignored. Capture maximum retained document
bytes before changing the implementation, then repeat with the same repository identity
and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 21: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Worker
events return the same tag and stale events are ignored. Capture cache hit identity and
disposition before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 22: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Worker events
return the same tag and stale events are ignored. Capture stale reply rejection count
before changing the implementation, then repeat with the same repository identity and
selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 23: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Worker events
return the same tag and stale events are ignored. Capture visible continuity after
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

### Scenario 24: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Worker events return the same tag and stale events are ignored. Capture time to first
useful rows before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 25: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Mailbox slots
replace obsolete refreshes and previews instead of extending a queue. Capture
steady-state frame cost before changing the implementation, then repeat with the same
repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 26: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is:
Mailbox slots replace obsolete refreshes and previews instead of extending a queue.
Capture bytes accepted from child stdout before changing the implementation, then repeat
with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 27: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is:
Mailbox slots replace obsolete refreshes and previews instead of extending a queue.
Capture number of Git and gh processes before changing the implementation, then repeat
with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 28: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Mailbox slots
replace obsolete refreshes and previews instead of extending a queue. Capture maximum
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

### Scenario 29: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Mailbox
slots replace obsolete refreshes and previews instead of extending a queue. Capture
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

### Scenario 30: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Mailbox slots
replace obsolete refreshes and previews instead of extending a queue. Capture stale
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

### Scenario 31: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Mailbox slots
replace obsolete refreshes and previews instead of extending a queue. Capture visible
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

### Scenario 32: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Mailbox slots replace obsolete refreshes and previews instead of extending a queue.
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

## Failure modes and review responses

### Risk 1

FIFO queues grow under key repeat even when only the final selection matters.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a cold cache followed by a warm
cache. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 2

Coalescing mutations loses operations whose order changes repository state.

Review response. Locate the acquisition boundary in `src/cli/command.rs`, identify the
complete cache or generation key, and prove the outcome under a small local repository.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 3

A generation checked only before execution can still apply stale output later.

Review response. Locate the acquisition boundary in `ARCHITECTURE.md`, identify the
complete cache or generation key, and prove the outcome under a monorepo with many
changed paths. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

### Risk 4

One network request can head-of-line block local status and preview work.

Review response. Locate the acquisition boundary in `src/git/worker.rs`, identify the
complete cache or generation key, and prove the outcome under a pull request with
generated files. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

### Risk 5

A prefetch slot that replaces preview reverses interactive priority.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a deeply diverged branch. Prefer a
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

Evidence 1. mailbox coalescing test. The check should state the repository question, the
optimized boundary, the expected bounded behavior, and the state that must remain
unchanged. When the behavior is asynchronous, include both the accepted reply and a
stale or replayed reply.

Evidence 2. user operation priority test. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 3. lane isolation tests. The check should state the repository question, the
optimized boundary, the expected bounded behavior, and the state that must remain
unchanged. When the behavior is asynchronous, include both the accepted reply and a
stale or replayed reply.

Evidence 4. stale preview and metadata rejection tests. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 5. refresh replay after in-flight completion. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

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
| 1 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record time to first useful rows |
| 2 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Each request carries the generation under which its user intent was created | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Each request carries the generation under which its user intent was created | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Each request carries the generation under which its user intent was created | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Each request carries the generation under which its user intent was created | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Each request carries the generation under which its user intent was created | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Each request carries the generation under which its user intent was created | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Each request carries the generation under which its user intent was created | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Each request carries the generation under which its user intent was created | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Each request carries the generation under which its user intent was created | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Each request carries the generation under which its user intent was created | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Each request carries the generation under which its user intent was created | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Each request carries the generation under which its user intent was created | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Each request carries the generation under which its user intent was created | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Each request carries the generation under which its user intent was created | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Each request carries the generation under which its user intent was created | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Each request carries the generation under which its user intent was created | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Each request carries the generation under which its user intent was created | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Each request carries the generation under which its user intent was created | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Each request carries the generation under which its user intent was created | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Each request carries the generation under which its user intent was created | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Each request carries the generation under which its user intent was created | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Each request carries the generation under which its user intent was created | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Each request carries the generation under which its user intent was created | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Each request carries the generation under which its user intent was created | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Each request carries the generation under which its user intent was created | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Each request carries the generation under which its user intent was created | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Each request carries the generation under which its user intent was created | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Each request carries the generation under which its user intent was created | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Each request carries the generation under which its user intent was created | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Each request carries the generation under which its user intent was created | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Each request carries the generation under which its user intent was created | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Each request carries the generation under which its user intent was created | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Each request carries the generation under which its user intent was created | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Each request carries the generation under which its user intent was created | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Each request carries the generation under which its user intent was created | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Each request carries the generation under which its user intent was created | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Each request carries the generation under which its user intent was created | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Each request carries the generation under which its user intent was created | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Each request carries the generation under which its user intent was created | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Each request carries the generation under which its user intent was created | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Each request carries the generation under which its user intent was created | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Each request carries the generation under which its user intent was created | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Each request carries the generation under which its user intent was created | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Each request carries the generation under which its user intent was created | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Each request carries the generation under which its user intent was created | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Each request carries the generation under which its user intent was created | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Each request carries the generation under which its user intent was created | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Each request carries the generation under which its user intent was created | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Each request carries the generation under which its user intent was created | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Each request carries the generation under which its user intent was created | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Each request carries the generation under which its user intent was created | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Each request carries the generation under which its user intent was created | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record time to first useful rows |
| 514 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record steady-state frame cost |
| 515 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record stale reply rejection count |
| 520 | App state increments generations before replacement work can reply | Check latency in a small local repository | Record visible continuity after failure |
| 521 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | App state increments generations before replacement work can reply | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | App state increments generations before replacement work can reply | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | App state increments generations before replacement work can reply | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | App state increments generations before replacement work can reply | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | App state increments generations before replacement work can reply | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | App state increments generations before replacement work can reply | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | App state increments generations before replacement work can reply | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | App state increments generations before replacement work can reply | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | App state increments generations before replacement work can reply | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | App state increments generations before replacement work can reply | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | App state increments generations before replacement work can reply | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | App state increments generations before replacement work can reply | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | App state increments generations before replacement work can reply | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | App state increments generations before replacement work can reply | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | App state increments generations before replacement work can reply | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | App state increments generations before replacement work can reply | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | App state increments generations before replacement work can reply | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | App state increments generations before replacement work can reply | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | App state increments generations before replacement work can reply | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | App state increments generations before replacement work can reply | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | App state increments generations before replacement work can reply | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | App state increments generations before replacement work can reply | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | App state increments generations before replacement work can reply | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | App state increments generations before replacement work can reply | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | App state increments generations before replacement work can reply | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | App state increments generations before replacement work can reply | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | App state increments generations before replacement work can reply | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | App state increments generations before replacement work can reply | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | App state increments generations before replacement work can reply | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | App state increments generations before replacement work can reply | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | App state increments generations before replacement work can reply | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | App state increments generations before replacement work can reply | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | App state increments generations before replacement work can reply | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | App state increments generations before replacement work can reply | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | App state increments generations before replacement work can reply | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | App state increments generations before replacement work can reply | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | App state increments generations before replacement work can reply | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | App state increments generations before replacement work can reply | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | App state increments generations before replacement work can reply | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | App state increments generations before replacement work can reply | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | App state increments generations before replacement work can reply | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | App state increments generations before replacement work can reply | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | App state increments generations before replacement work can reply | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | App state increments generations before replacement work can reply | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | App state increments generations before replacement work can reply | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | App state increments generations before replacement work can reply | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | App state increments generations before replacement work can reply | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | App state increments generations before replacement work can reply | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | App state increments generations before replacement work can reply | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | App state increments generations before replacement work can reply | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | App state increments generations before replacement work can reply | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | App state increments generations before replacement work can reply | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | App state increments generations before replacement work can reply | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | App state increments generations before replacement work can reply | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | App state increments generations before replacement work can reply | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | App state increments generations before replacement work can reply | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | App state increments generations before replacement work can reply | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | App state increments generations before replacement work can reply | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | App state increments generations before replacement work can reply | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | App state increments generations before replacement work can reply | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | App state increments generations before replacement work can reply | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 1015 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 1016 | App state increments generations before replacement work can reply | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 1017 | App state increments generations before replacement work can reply | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 1018 | App state increments generations before replacement work can reply | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 1019 | App state increments generations before replacement work can reply | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
