# Rendering Optimization

Ratatui frames stay responsive when layout construction, intraline analysis, wrapping,
and hit-map work are restricted to visible rows or reused until their inputs change.

## Reading map

- [Viewport rendering](./viewport.md)
- [Progressive loading](./progressive-loading.md)
- [Concurrency, generations, mailboxes, and worker lanes](./concurrency.md)

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

- [`src/ui/mod.rs`](../../../src/ui/mod.rs)
- [`src/app.rs`](../../../src/app.rs)
- [`src/git/diff.rs`](../../../src/git/diff.rs)
- [`src/theme.rs`](../../../src/theme.rs)

- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)

## Operational contract

1. Unified and side-by-side row mappings are cached by document generation and layout
mode.

2. Intraline emphasis is computed only for rows that intersect the viewport.

3. Pull-request overview rows are rebuilt only when content or pane width changes.

4. The PR file tree is built once per index or fold-state change.

5. Only viewport rows become ratatui lines and mouse hit targets.

6. Horizontal scrolling is enabled only for row kinds that can exceed pane width.

7. Headers remain anchored while code and log content pan in display columns.

8. End navigation records intent and lets drawing clamp against current content length.

## Git and systems foundations

### 1. Content identity

When a cache key contains every immutable input to a computation, freshness becomes a
property of identity rather than elapsed time. Time-to-live remains appropriate only for
facts that can change under the same key.

For viewport rendering and layout caches, this model matters because unified and
side-by-side row mappings are cached by document generation and layout mode. The
boundary is semantic as well as computational: an optimization is invalid if it answers
a cheaper but different Git question.

### 2. Objects and snapshots

Git stores file contents as blobs, directory snapshots as trees, and history nodes as
commits. A commit names a tree and parent commits, so comparing commits is fundamentally
comparing immutable snapshots.

For viewport rendering and layout caches, this model matters because intraline emphasis
is computed only for rows that intersect the viewport. The boundary is semantic as well
as computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 3. Refs and object IDs

A ref such as a branch is a movable name. An object ID identifies immutable content.
Quinjet uses refs for user intent and resolved object IDs for workspaces and persistent
cache identity.

For viewport rendering and layout caches, this model matters because pull-request
overview rows are rebuilt only when content or pane width changes. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

### 4. The three trees

HEAD, the index, and the working tree represent committed, staged, and filesystem state.
Separate comparisons between these trees are what produce staged and unstaged views.

For viewport rendering and layout caches, this model matters because the pr file tree is
built once per index or fold-state change. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 5. Merge-base semantics

A pull-request diff starts at the best common ancestor and ends at the head commit. This
isolates the contribution from unrelated changes later added to the base branch.

For viewport rendering and layout caches, this model matters because only viewport rows
become ratatui lines and mouse hit targets. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 6. Path-limited diff

A pathspec narrows diff output after the comparison endpoints are fixed. It preserves
Git semantics while avoiding patch generation for files the current interaction does not
need.

For viewport rendering and layout caches, this model matters because horizontal
scrolling is enabled only for row kinds that can exceed pane width. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

### 7. Machine protocols

NUL-delimited status and diff-index records separate paths without relying on quoting or
locale. Explicit pretty-format delimiters do the same for commit history fields and
records.

For viewport rendering and layout caches, this model matters because headers remain
anchored while code and log content pan in display columns. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

### 8. Partial clone

Blob filtering permits commits and trees to arrive without every file body. This is
valuable only when later commands also avoid accidentally demanding all omitted blobs.

For viewport rendering and layout caches, this model matters because end navigation
records intent and lets drawing clamp against current content length. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

## Representative Git command shapes

### Command 1: Line-count index

```bash
git diff --numstat -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
same revision range supplies additions and deletions without syntax or hunk parsing.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 2: Selected-path patch

```bash
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 BASE HEAD -- PATH
```

This is a conceptual command shape rather than copyable internal tracing output. The
pathspec makes one interaction pay only for the file or batch it requested. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 3: Local merge base

```bash
git merge-base BASE_OID HEAD_OID
```

This is a conceptual command shape rather than copyable internal tracing output. The
common ancestor defines pull-request contribution semantics when both tips exist
locally. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

### Command 4: Blob-filtered fetch

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=N REMOTE REFSPEC
```

This is a conceptual command shape rather than copyable internal tracing output. Commit
and tree history can arrive without every changed blob body. Quinjet constructs the real
argv directly and applies operation-specific output caps and repository context in the
implementation.

### Command 5: Revision validation

```bash
git rev-parse --verify --quiet REVISION^{commit}
```

This is a conceptual command shape rather than copyable internal tracing output. Git
validates object type and resolves revision syntax without a checkout. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

## Implementation walkthrough

### Mechanism 1: Unified and side-by-side row mappings are cached by document generation and layout mode

Mechanics. Unified and side-by-side row mappings are cached by document generation and
layout mode. The relevant flow begins in src/app.rs and crosses only the layers needed
to preserve the shared command and session boundary.

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

Review evidence. Inspect `src/app.rs`, exercise viewport row render tests, and record
steady-state frame cost. Compare the cold and warm paths because cache and workspace
reuse intentionally make them different.

### Mechanism 2: Intraline emphasis is computed only for rows that intersect the viewport

Mechanics. Intraline emphasis is computed only for rows that intersect the viewport. The
relevant flow begins in src/git/diff.rs and crosses only the layers needed to preserve
the shared command and session boundary.

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

Review evidence. Inspect `src/git/diff.rs`, exercise horizontal scroll eligibility
tests, and record bytes accepted from child stdout. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 3: Pull-request overview rows are rebuilt only when content or pane width changes

Mechanics. Pull-request overview rows are rebuilt only when content or pane width
changes. The relevant flow begins in src/theme.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/theme.rs`, exercise tree collapse and compaction tests,
and record number of Git and gh processes. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 4: The PR file tree is built once per index or fold-state change

Mechanics. The PR file tree is built once per index or fold-state change. The relevant
flow begins in src/ui/mod.rs and crosses only the layers needed to preserve the shared
command and session boundary.

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

Review evidence. Inspect `src/ui/mod.rs`, exercise pane resize clamp tests, and record
maximum retained document bytes. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 5: Only viewport rows become ratatui lines and mouse hit targets

Mechanics. Only viewport rows become ratatui lines and mouse hit targets. The relevant
flow begins in src/app.rs and crosses only the layers needed to preserve the shared
command and session boundary.

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

Review evidence. Inspect `src/app.rs`, exercise PR 46 layout invalidation commits, and
record cache hit identity and disposition. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 6: Horizontal scrolling is enabled only for row kinds that can exceed pane width

Mechanics. Horizontal scrolling is enabled only for row kinds that can exceed pane
width. The relevant flow begins in src/git/diff.rs and crosses only the layers needed to
preserve the shared command and session boundary.

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

Review evidence. Inspect `src/git/diff.rs`, exercise viewport row render tests, and
record stale reply rejection count. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 7: Headers remain anchored while code and log content pan in display columns

Mechanics. Headers remain anchored while code and log content pan in display columns.
The relevant flow begins in src/theme.rs and crosses only the layers needed to preserve
the shared command and session boundary.

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

Review evidence. Inspect `src/theme.rs`, exercise horizontal scroll eligibility tests,
and record visible continuity after failure. Compare the cold and warm paths because
cache and workspace reuse intentionally make them different.

### Mechanism 8: End navigation records intent and lets drawing clamp against current content length

Mechanics. End navigation records intent and lets drawing clamp against current content
length. The relevant flow begins in src/ui/mod.rs and crosses only the layers needed to
preserve the shared command and session boundary.

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

Review evidence. Inspect `src/ui/mod.rs`, exercise tree collapse and compaction tests,
and record time to first useful rows. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Unified and
side-by-side row mappings are cached by document generation and layout mode. Capture
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

Start with a monorepo with many changed paths. The mechanism under inspection is:
Unified and side-by-side row mappings are cached by document generation and layout mode.
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

### Scenario 3: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is:
Unified and side-by-side row mappings are cached by document generation and layout mode.
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

### Scenario 4: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Unified and
side-by-side row mappings are cached by document generation and layout mode. Capture
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

### Scenario 5: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Unified and
side-by-side row mappings are cached by document generation and layout mode. Capture
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

Start with rapid keyboard navigation. The mechanism under inspection is: Unified and
side-by-side row mappings are cached by document generation and layout mode. Capture
stale reply rejection count before changing the implementation, then repeat with the
same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 7: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Unified and
side-by-side row mappings are cached by document generation and layout mode. Capture
visible continuity after failure before changing the implementation, then repeat with
the same repository identity and selection path after the change.

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
Unified and side-by-side row mappings are cached by document generation and layout mode.
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

Start with a small local repository. The mechanism under inspection is: Intraline
emphasis is computed only for rows that intersect the viewport. Capture steady-state
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
Intraline emphasis is computed only for rows that intersect the viewport. Capture bytes
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

Start with a pull request with generated files. The mechanism under inspection is:
Intraline emphasis is computed only for rows that intersect the viewport. Capture number
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

Start with a deeply diverged branch. The mechanism under inspection is: Intraline
emphasis is computed only for rows that intersect the viewport. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: Intraline
emphasis is computed only for rows that intersect the viewport. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: Intraline
emphasis is computed only for rows that intersect the viewport. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Intraline emphasis
is computed only for rows that intersect the viewport. Capture visible continuity after
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

### Scenario 16: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Intraline emphasis is computed only for rows that intersect the viewport. Capture time
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

### Scenario 17: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Pull-request
overview rows are rebuilt only when content or pane width changes. Capture steady-state
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

### Scenario 18: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is:
Pull-request overview rows are rebuilt only when content or pane width changes. Capture
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
Pull-request overview rows are rebuilt only when content or pane width changes. Capture
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

Start with a deeply diverged branch. The mechanism under inspection is: Pull-request
overview rows are rebuilt only when content or pane width changes. Capture maximum
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

### Scenario 21: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is:
Pull-request overview rows are rebuilt only when content or pane width changes. Capture
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

### Scenario 22: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Pull-request
overview rows are rebuilt only when content or pane width changes. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Pull-request
overview rows are rebuilt only when content or pane width changes. Capture visible
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

### Scenario 24: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Pull-request overview rows are rebuilt only when content or pane width changes. Capture
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

Start with a small local repository. The mechanism under inspection is: The PR file tree
is built once per index or fold-state change. Capture steady-state frame cost before
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

### Scenario 26: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: The PR
file tree is built once per index or fold-state change. Capture bytes accepted from
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

### Scenario 27: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: The
PR file tree is built once per index or fold-state change. Capture number of Git and gh
processes before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 28: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: The PR file tree
is built once per index or fold-state change. Capture maximum retained document bytes
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

### Scenario 29: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: The PR file
tree is built once per index or fold-state change. Capture cache hit identity and
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

### Scenario 30: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: The PR file
tree is built once per index or fold-state change. Capture stale reply rejection count
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

### Scenario 31: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: The PR file tree is
built once per index or fold-state change. Capture visible continuity after failure
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

### Scenario 32: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is: The
PR file tree is built once per index or fold-state change. Capture time to first useful
rows before changing the implementation, then repeat with the same repository identity
and selection path after the change.

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

Recomputing intraline differences for hidden rows makes every frame document-sized.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a slow or unavailable network.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 2

A layout cache missing width renders wrapping and hit targets at old geometry.

Review response. Locate the acquisition boundary in `src/git/diff.rs`, identify the
complete cache or generation key, and prove the outcome under rapid keyboard navigation.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 3

Caching borrowed display data beyond its source lifetime creates invalid state.

Review response. Locate the acquisition boundary in `src/theme.rs`, identify the
complete cache or generation key, and prove the outcome under a linked Git worktree.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 4

Byte offsets used as terminal columns break multibyte and wide characters.

Review response. Locate the acquisition boundary in `src/ui/mod.rs`, identify the
complete cache or generation key, and prove the outcome under a cold cache followed by a
warm cache. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

### Risk 5

Building hit targets for hidden rows wastes memory and can capture wrong clicks.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a small local repository. Prefer a
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

Evidence 1. PR 46 layout invalidation commits. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 2. viewport row render tests. The check should state the repository question,
the optimized boundary, the expected bounded behavior, and the state that must remain
unchanged. When the behavior is asynchronous, include both the accepted reply and a
stale or replayed reply.

Evidence 3. horizontal scroll eligibility tests. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 4. tree collapse and compaction tests. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 5. pane resize clamp tests. The check should state the repository question, the
optimized boundary, the expected bounded behavior, and the state that must remain
unchanged. When the behavior is asynchronous, include both the accepted reply and a
stale or replayed reply.

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
| 1 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record time to first useful rows |
| 2 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Unified and side-by-side row mappings are cached by document generation and layout mode | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record time to first useful rows |
| 514 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record steady-state frame cost |
| 515 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record stale reply rejection count |
| 520 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a small local repository | Record visible continuity after failure |
| 521 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | Intraline emphasis is computed only for rows that intersect the viewport | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | Intraline emphasis is computed only for rows that intersect the viewport | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | Intraline emphasis is computed only for rows that intersect the viewport | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | Intraline emphasis is computed only for rows that intersect the viewport | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | Intraline emphasis is computed only for rows that intersect the viewport | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | Intraline emphasis is computed only for rows that intersect the viewport | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | Intraline emphasis is computed only for rows that intersect the viewport | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 1015 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 1016 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 1017 | Intraline emphasis is computed only for rows that intersect the viewport | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
