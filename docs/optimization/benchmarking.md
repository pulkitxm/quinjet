# Verification, Benchmarking, and Regression Control

Optimization is complete only when correctness, bounds, invalidation, degradation, and
user-visible latency are expressed as repeatable evidence.

This page complements the concrete measurements embedded throughout the
[pull-request workspace](./github/pr-workspace.md),
[prefetch](./github/prefetch.md), and
[progressive loading](./rendering/progressive-loading.md) chapters.

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

- [`Makefile`](../../Makefile)
- [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)
- [`.github/workflows/hygiene.yml`](../../.github/workflows/hygiene.yml)
- [`src/app.rs`](../../src/app.rs)

- [`ARCHITECTURE.md`](../../ARCHITECTURE.md)

## Operational contract

1. Unit tests exercise parsers, scheduling, cache identity, and app state transitions.

2. Process tests validate argv, exit codes, JSON shape, and destructive previews.

3. Large synthetic documents expose accidental full-document work in rendering.

4. Bound tests prove the child is killed rather than merely truncating after allocation.

5. Regression tests encode the race that each generation or replay flag prevents.

6. Documentation links and generated wiki pages are checked before publication.

7. Clippy restriction lints keep error paths total and collection access defensive.

8. Measurements separate cold setup, first useful paint, steady frame, and complete
fill.

## Git and systems foundations

### 1. Partial clone

Blob filtering permits commits and trees to arrive without every file body. This is
valuable only when later commands also avoid accidentally demanding all omitted blobs.

For verification, benchmarking, and regression control, this model matters because unit
tests exercise parsers, scheduling, cache identity, and app state transitions. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 2. Pack storage

Loose objects and packfiles are storage details behind the same object database.
Delegating to Git lets Quinjet benefit from delta compression and repository maintenance
without reimplementing them.

For verification, benchmarking, and regression control, this model matters because
process tests validate argv, exit codes, json shape, and destructive previews. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 3. Diffcore

Git transforms raw tree differences through rename detection and other diffcore stages
before formatting a patch. Quinjet consumes the resulting machine and patch formats
instead of approximating those rules.

For verification, benchmarking, and regression control, this model matters because large
synthetic documents expose accidental full-document work in rendering. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

### 4. Index locking

Many mutations lock and rewrite the index. Read-only commands set GIT_OPTIONAL_LOCKS to
zero so background inspection avoids optional lock traffic and interference.

For verification, benchmarking, and regression control, this model matters because bound
tests prove the child is killed rather than merely truncating after allocation. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 5. Revision resolution

Revision syntax can name refs, ancestors, and object IDs. Quinjet validates user-facing
revision categories and passes argv directly, leaving resolution to Git without shell
interpretation.

For verification, benchmarking, and regression control, this model matters because
regression tests encode the race that each generation or replay flag prevents. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 6. Content identity

When a cache key contains every immutable input to a computation, freshness becomes a
property of identity rather than elapsed time. Time-to-live remains appropriate only for
facts that can change under the same key.

For verification, benchmarking, and regression control, this model matters because
documentation links and generated wiki pages are checked before publication. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 7. Objects and snapshots

Git stores file contents as blobs, directory snapshots as trees, and history nodes as
commits. A commit names a tree and parent commits, so comparing commits is fundamentally
comparing immutable snapshots.

For verification, benchmarking, and regression control, this model matters because
clippy restriction lints keep error paths total and collection access defensive. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 8. Refs and object IDs

A ref such as a branch is a movable name. An object ID identifies immutable content.
Quinjet uses refs for user intent and resolved object IDs for workspaces and persistent
cache identity.

For verification, benchmarking, and regression control, this model matters because
measurements separate cold setup, first useful paint, steady frame, and complete fill.
The boundary is semantic as well as computational: an optimization is invalid if it
answers a cheaper but different Git question.

## Representative Git command shapes

### Command 1: Changed-path index

```bash
git diff --name-status -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
path and status index is cheaper to acquire and parse than full patch bodies. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 2: Line-count index

```bash
git diff --numstat -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
same revision range supplies additions and deletions without syntax or hunk parsing.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 3: Selected-path patch

```bash
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 BASE HEAD -- PATH
```

This is a conceptual command shape rather than copyable internal tracing output. The
pathspec makes one interaction pay only for the file or batch it requested. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 4: Local merge base

```bash
git merge-base BASE_OID HEAD_OID
```

This is a conceptual command shape rather than copyable internal tracing output. The
common ancestor defines pull-request contribution semantics when both tips exist
locally. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

### Command 5: Blob-filtered fetch

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=N REMOTE REFSPEC
```

This is a conceptual command shape rather than copyable internal tracing output. Commit
and tree history can arrive without every changed blob body. Quinjet constructs the real
argv directly and applies operation-specific output caps and repository context in the
implementation.

## Implementation walkthrough

### Mechanism 1: Unit tests exercise parsers, scheduling, cache identity, and app state transitions

Mechanics. Unit tests exercise parsers, scheduling, cache identity, and app state
transitions. The relevant flow begins in .github/workflows/ci.yml and crosses only the
layers needed to preserve the shared command and session boundary.

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

Review evidence. Inspect `.github/workflows/ci.yml`, exercise large layout and mailbox
unit tests, and record steady-state frame cost. Compare the cold and warm paths because
cache and workspace reuse intentionally make them different.

### Mechanism 2: Process tests validate argv, exit codes, JSON shape, and destructive previews

Mechanics. Process tests validate argv, exit codes, JSON shape, and destructive
previews. The relevant flow begins in .github/workflows/hygiene.yml and crosses only the
layers needed to preserve the shared command and session boundary.

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

Review evidence. Inspect `.github/workflows/hygiene.yml`, exercise bounded output
subprocess test, and record bytes accepted from child stdout. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 3: Large synthetic documents expose accidental full-document work in rendering

Mechanics. Large synthetic documents expose accidental full-document work in rendering.
The relevant flow begins in src/app.rs and crosses only the layers needed to preserve
the shared command and session boundary.

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

Review evidence. Inspect `src/app.rs`, exercise wiki link and markdown checks, and
record number of Git and gh processes. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 4: Bound tests prove the child is killed rather than merely truncating after allocation

Mechanics. Bound tests prove the child is killed rather than merely truncating after
allocation. The relevant flow begins in Makefile and crosses only the layers needed to
preserve the shared command and session boundary.

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

Review evidence. Inspect `Makefile`, exercise PR 46 through PR 55 rationale ledger, and
record maximum retained document bytes. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 5: Regression tests encode the race that each generation or replay flag prevents

Mechanics. Regression tests encode the race that each generation or replay flag
prevents. The relevant flow begins in .github/workflows/ci.yml and crosses only the
layers needed to preserve the shared command and session boundary.

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

Review evidence. Inspect `.github/workflows/ci.yml`, exercise make ci-fast and make ci
gates, and record cache hit identity and disposition. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 6: Documentation links and generated wiki pages are checked before publication

Mechanics. Documentation links and generated wiki pages are checked before publication.
The relevant flow begins in .github/workflows/hygiene.yml and crosses only the layers
needed to preserve the shared command and session boundary.

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

Review evidence. Inspect `.github/workflows/hygiene.yml`, exercise large layout and
mailbox unit tests, and record stale reply rejection count. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 7: Clippy restriction lints keep error paths total and collection access defensive

Mechanics. Clippy restriction lints keep error paths total and collection access
defensive. The relevant flow begins in src/app.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/app.rs`, exercise bounded output subprocess test, and
record visible continuity after failure. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 8: Measurements separate cold setup, first useful paint, steady frame, and complete fill

Mechanics. Measurements separate cold setup, first useful paint, steady frame, and
complete fill. The relevant flow begins in Makefile and crosses only the layers needed
to preserve the shared command and session boundary.

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

Review evidence. Inspect `Makefile`, exercise wiki link and markdown checks, and record
time to first useful rows. Compare the cold and warm paths because cache and workspace
reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Unit tests
exercise parsers, scheduling, cache identity, and app state transitions. Capture
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

### Scenario 2: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Unit
tests exercise parsers, scheduling, cache identity, and app state transitions. Capture
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

Start with a pull request with generated files. The mechanism under inspection is: Unit
tests exercise parsers, scheduling, cache identity, and app state transitions. Capture
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

Start with a deeply diverged branch. The mechanism under inspection is: Unit tests
exercise parsers, scheduling, cache identity, and app state transitions. Capture maximum
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

Start with a slow or unavailable network. The mechanism under inspection is: Unit tests
exercise parsers, scheduling, cache identity, and app state transitions. Capture cache
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

Start with rapid keyboard navigation. The mechanism under inspection is: Unit tests
exercise parsers, scheduling, cache identity, and app state transitions. Capture stale
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

### Scenario 7: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Unit tests exercise
parsers, scheduling, cache identity, and app state transitions. Capture visible
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
Unit tests exercise parsers, scheduling, cache identity, and app state transitions.
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

Start with a small local repository. The mechanism under inspection is: Process tests
validate argv, exit codes, JSON shape, and destructive previews. Capture steady-state
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

Start with a monorepo with many changed paths. The mechanism under inspection is:
Process tests validate argv, exit codes, JSON shape, and destructive previews. Capture
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

### Scenario 11: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is:
Process tests validate argv, exit codes, JSON shape, and destructive previews. Capture
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

### Scenario 12: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Process tests
validate argv, exit codes, JSON shape, and destructive previews. Capture maximum
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

### Scenario 13: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Process
tests validate argv, exit codes, JSON shape, and destructive previews. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: Process tests
validate argv, exit codes, JSON shape, and destructive previews. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Process tests
validate argv, exit codes, JSON shape, and destructive previews. Capture visible
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

### Scenario 16: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Process tests validate argv, exit codes, JSON shape, and destructive previews. Capture
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

Start with a small local repository. The mechanism under inspection is: Large synthetic
documents expose accidental full-document work in rendering. Capture steady-state frame
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

Start with a monorepo with many changed paths. The mechanism under inspection is: Large
synthetic documents expose accidental full-document work in rendering. Capture bytes
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

### Scenario 19: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Large
synthetic documents expose accidental full-document work in rendering. Capture number of
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

### Scenario 20: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Large synthetic
documents expose accidental full-document work in rendering. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: Large
synthetic documents expose accidental full-document work in rendering. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: Large synthetic
documents expose accidental full-document work in rendering. Capture stale reply
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

### Scenario 23: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Large synthetic
documents expose accidental full-document work in rendering. Capture visible continuity
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
Large synthetic documents expose accidental full-document work in rendering. Capture
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

Start with a small local repository. The mechanism under inspection is: Bound tests
prove the child is killed rather than merely truncating after allocation. Capture
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

Start with a monorepo with many changed paths. The mechanism under inspection is: Bound
tests prove the child is killed rather than merely truncating after allocation. Capture
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

### Scenario 27: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Bound
tests prove the child is killed rather than merely truncating after allocation. Capture
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

### Scenario 28: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Bound tests
prove the child is killed rather than merely truncating after allocation. Capture
maximum retained document bytes before changing the implementation, then repeat with the
same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 29: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Bound tests
prove the child is killed rather than merely truncating after allocation. Capture cache
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

### Scenario 30: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Bound tests
prove the child is killed rather than merely truncating after allocation. Capture stale
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

Start with a linked Git worktree. The mechanism under inspection is: Bound tests prove
the child is killed rather than merely truncating after allocation. Capture visible
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
Bound tests prove the child is killed rather than merely truncating after allocation.
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

A microbenchmark can improve while the first visible result regresses.

Review response. Locate the acquisition boundary in `.github/workflows/ci.yml`, identify
the complete cache or generation key, and prove the outcome under a deeply diverged
branch. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 2

A happy-path test says nothing about stale replies or truncated records.

Review response. Locate the acquisition boundary in `.github/workflows/hygiene.yml`,
identify the complete cache or generation key, and prove the outcome under a slow or
unavailable network. Prefer a test that asserts state and bounds over one that depends
on wall-clock timing.

### Risk 3

Wall-clock assertions in CI become flaky under shared runners.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under rapid keyboard navigation. Prefer a
test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 4

Benchmarking a warm OS cache alone hides cold repository behavior.

Review response. Locate the acquisition boundary in `Makefile`, identify the complete
cache or generation key, and prove the outcome under a linked Git worktree. Prefer a
test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 5

Counting requests without payload bytes misses the dominant transfer cost.

Review response. Locate the acquisition boundary in `.github/workflows/ci.yml`, identify
the complete cache or generation key, and prove the outcome under a cold cache followed
by a warm cache. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

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

Evidence 1. make ci-fast and make ci gates. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 2. large layout and mailbox unit tests. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 3. bounded output subprocess test. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 4. wiki link and markdown checks. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 5. PR 46 through PR 55 rationale ledger. The check should state the repository
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
| 1 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record time to first useful rows |
| 2 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Unit tests exercise parsers, scheduling, cache identity, and app state transitions | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record time to first useful rows |
| 514 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record steady-state frame cost |
| 515 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record stale reply rejection count |
| 520 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a small local repository | Record visible continuity after failure |
| 521 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | Process tests validate argv, exit codes, JSON shape, and destructive previews | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
