# Diff Engineering

Diff work is split into cheap indexes, bounded raw patches, structured documents, and
viewport rendering so cost is paid only when each representation is needed.

## Reading map

- [The diff pipeline](./pipeline.md)
- [Diff algorithms](./algorithms.md)
- [Intraline emphasis and syntax highlighting](./intraline-and-highlighting.md)

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

- [`src/git/diff.rs`](../../../src/git/diff.rs)
- [`src/git/mod.rs`](../../../src/git/mod.rs)
- [`src/ui/mod.rs`](../../../src/ui/mod.rs)
- [`src/app.rs`](../../../src/app.rs)

- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)

## Operational contract

1. Name-status output creates the file index without requiring patch bodies.

2. Numstat output supplies additions and deletions independently from patch parsing.

3. Unified patches are parsed into semantic rows with old and new line numbers.

4. Transport headers are separated from user-visible file and hunk content.

5. Syntax highlighting uses the path to select a grammar and semantic theme roles.

6. Grammar work stops for oversized patches and exceptionally long rows.

7. Batched patches are split at diff headers back into path-keyed documents.

8. Collapsed files contribute headers but do not clone cached patch bodies.

## Git and systems foundations

### 1. Machine protocols

NUL-delimited status and diff-index records separate paths without relying on quoting or
locale. Explicit pretty-format delimiters do the same for commit history fields and
records.

For diff construction and parsing, this model matters because name-status output creates
the file index without requiring patch bodies. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 2. Partial clone

Blob filtering permits commits and trees to arrive without every file body. This is
valuable only when later commands also avoid accidentally demanding all omitted blobs.

For diff construction and parsing, this model matters because numstat output supplies
additions and deletions independently from patch parsing. The boundary is semantic as
well as computational: an optimization is invalid if it answers a cheaper but different
Git question.

### 3. Pack storage

Loose objects and packfiles are storage details behind the same object database.
Delegating to Git lets Quinjet benefit from delta compression and repository maintenance
without reimplementing them.

For diff construction and parsing, this model matters because unified patches are parsed
into semantic rows with old and new line numbers. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 4. Diffcore

Git transforms raw tree differences through rename detection and other diffcore stages
before formatting a patch. Quinjet consumes the resulting machine and patch formats
instead of approximating those rules.

For diff construction and parsing, this model matters because transport headers are
separated from user-visible file and hunk content. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 5. Index locking

Many mutations lock and rewrite the index. Read-only commands set GIT_OPTIONAL_LOCKS to
zero so background inspection avoids optional lock traffic and interference.

For diff construction and parsing, this model matters because syntax highlighting uses
the path to select a grammar and semantic theme roles. The boundary is semantic as well
as computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 6. Revision resolution

Revision syntax can name refs, ancestors, and object IDs. Quinjet validates user-facing
revision categories and passes argv directly, leaving resolution to Git without shell
interpretation.

For diff construction and parsing, this model matters because grammar work stops for
oversized patches and exceptionally long rows. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 7. Content identity

When a cache key contains every immutable input to a computation, freshness becomes a
property of identity rather than elapsed time. Time-to-live remains appropriate only for
facts that can change under the same key.

For diff construction and parsing, this model matters because batched patches are split
at diff headers back into path-keyed documents. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 8. Objects and snapshots

Git stores file contents as blobs, directory snapshots as trees, and history nodes as
commits. A commit names a tree and parent commits, so comparing commits is fundamentally
comparing immutable snapshots.

For diff construction and parsing, this model matters because collapsed files contribute
headers but do not clone cached patch bodies. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

## Representative Git command shapes

### Command 1: Local merge base

```bash
git merge-base BASE_OID HEAD_OID
```

This is a conceptual command shape rather than copyable internal tracing output. The
common ancestor defines pull-request contribution semantics when both tips exist
locally. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

### Command 2: Blob-filtered fetch

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=N REMOTE REFSPEC
```

This is a conceptual command shape rather than copyable internal tracing output. Commit
and tree history can arrive without every changed blob body. Quinjet constructs the real
argv directly and applies operation-specific output caps and repository context in the
implementation.

### Command 3: Revision validation

```bash
git rev-parse --verify --quiet REVISION^{commit}
```

This is a conceptual command shape rather than copyable internal tracing output. Git
validates object type and resolves revision syntax without a checkout. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 4: Status snapshot

```bash
git status --porcelain=v2 --branch -z --untracked-files=all --ignore-submodules=none
```

This is a conceptual command shape rather than copyable internal tracing output.
Porcelain version 2 and NUL records provide a stable byte protocol for branch and path
state. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

### Command 5: Bounded history page

```bash
git log --topo-order --decorate=short --no-color --skip=N --max-count=N --format=FORMAT REV --
```

This is a conceptual command shape rather than copyable internal tracing output. An
explicit revision and page bound avoid ambient HEAD races and repository-sized output.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

## Implementation walkthrough

### Mechanism 1: Name-status output creates the file index without requiring patch bodies

Mechanics. Name-status output creates the file index without requiring patch bodies. The
relevant flow begins in src/git/mod.rs and crosses only the layers needed to preserve
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

Review evidence. Inspect `src/git/mod.rs`, exercise split_patch_by_file batch fixture,
and record steady-state frame cost. Compare the cold and warm paths because cache and
workspace reuse intentionally make them different.

### Mechanism 2: Numstat output supplies additions and deletions independently from patch parsing

Mechanics. Numstat output supplies additions and deletions independently from patch
parsing. The relevant flow begins in src/ui/mod.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/ui/mod.rs`, exercise large patch and long-line grammar
limits, and record bytes accepted from child stdout. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 3: Unified patches are parsed into semantic rows with old and new line numbers

Mechanics. Unified patches are parsed into semantic rows with old and new line numbers.
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

Review evidence. Inspect `src/app.rs`, exercise indexed totals independent of loaded
patches, and record number of Git and gh processes. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 4: Transport headers are separated from user-visible file and hunk content

Mechanics. Transport headers are separated from user-visible file and hunk content. The
relevant flow begins in src/git/diff.rs and crosses only the layers needed to preserve
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

Review evidence. Inspect `src/git/diff.rs`, exercise hunk line-number parsing tests, and
record maximum retained document bytes. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 5: Syntax highlighting uses the path to select a grammar and semantic theme roles

Mechanics. Syntax highlighting uses the path to select a grammar and semantic theme
roles. The relevant flow begins in src/git/mod.rs and crosses only the layers needed to
preserve the shared command and session boundary.

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

Review evidence. Inspect `src/git/mod.rs`, exercise parse_numstat rename and binary
fixtures, and record cache hit identity and disposition. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 6: Grammar work stops for oversized patches and exceptionally long rows

Mechanics. Grammar work stops for oversized patches and exceptionally long rows. The
relevant flow begins in src/ui/mod.rs and crosses only the layers needed to preserve the
shared command and session boundary.

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

Review evidence. Inspect `src/ui/mod.rs`, exercise split_patch_by_file batch fixture,
and record stale reply rejection count. Compare the cold and warm paths because cache
and workspace reuse intentionally make them different.

### Mechanism 7: Batched patches are split at diff headers back into path-keyed documents

Mechanics. Batched patches are split at diff headers back into path-keyed documents. The
relevant flow begins in src/app.rs and crosses only the layers needed to preserve the
shared command and session boundary.

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

Review evidence. Inspect `src/app.rs`, exercise large patch and long-line grammar
limits, and record visible continuity after failure. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 8: Collapsed files contribute headers but do not clone cached patch bodies

Mechanics. Collapsed files contribute headers but do not clone cached patch bodies. The
relevant flow begins in src/git/diff.rs and crosses only the layers needed to preserve
the shared command and session boundary.

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

Review evidence. Inspect `src/git/diff.rs`, exercise indexed totals independent of
loaded patches, and record time to first useful rows. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Name-status
output creates the file index without requiring patch bodies. Capture steady-state frame
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

### Scenario 2: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is:
Name-status output creates the file index without requiring patch bodies. Capture bytes
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

Start with a pull request with generated files. The mechanism under inspection is:
Name-status output creates the file index without requiring patch bodies. Capture number
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

Start with a deeply diverged branch. The mechanism under inspection is: Name-status
output creates the file index without requiring patch bodies. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: Name-status
output creates the file index without requiring patch bodies. Capture cache hit identity
and disposition before changing the implementation, then repeat with the same repository
identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 6: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Name-status
output creates the file index without requiring patch bodies. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Name-status output
creates the file index without requiring patch bodies. Capture visible continuity after
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

### Scenario 8: A Cold Cache Followed By A Warm Cache

Start with a cold cache followed by a warm cache. The mechanism under inspection is:
Name-status output creates the file index without requiring patch bodies. Capture time
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

### Scenario 9: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Numstat output
supplies additions and deletions independently from patch parsing. Capture steady-state
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
Numstat output supplies additions and deletions independently from patch parsing.
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

### Scenario 11: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is:
Numstat output supplies additions and deletions independently from patch parsing.
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

### Scenario 12: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Numstat output
supplies additions and deletions independently from patch parsing. Capture maximum
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

Start with a slow or unavailable network. The mechanism under inspection is: Numstat
output supplies additions and deletions independently from patch parsing. Capture cache
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

### Scenario 14: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Numstat output
supplies additions and deletions independently from patch parsing. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Numstat output
supplies additions and deletions independently from patch parsing. Capture visible
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
Numstat output supplies additions and deletions independently from patch parsing.
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

### Scenario 17: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Unified patches
are parsed into semantic rows with old and new line numbers. Capture steady-state frame
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
Unified patches are parsed into semantic rows with old and new line numbers. Capture
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
Unified patches are parsed into semantic rows with old and new line numbers. Capture
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

Start with a deeply diverged branch. The mechanism under inspection is: Unified patches
are parsed into semantic rows with old and new line numbers. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: Unified
patches are parsed into semantic rows with old and new line numbers. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: Unified patches
are parsed into semantic rows with old and new line numbers. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Unified patches are
parsed into semantic rows with old and new line numbers. Capture visible continuity
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
Unified patches are parsed into semantic rows with old and new line numbers. Capture
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

Start with a small local repository. The mechanism under inspection is: Transport
headers are separated from user-visible file and hunk content. Capture steady-state
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

### Scenario 26: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is:
Transport headers are separated from user-visible file and hunk content. Capture bytes
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

Start with a pull request with generated files. The mechanism under inspection is:
Transport headers are separated from user-visible file and hunk content. Capture number
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

### Scenario 28: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Transport
headers are separated from user-visible file and hunk content. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: Transport
headers are separated from user-visible file and hunk content. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: Transport
headers are separated from user-visible file and hunk content. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Transport headers
are separated from user-visible file and hunk content. Capture visible continuity after
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
Transport headers are separated from user-visible file and hunk content. Capture time to
first useful rows before changing the implementation, then repeat with the same
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

Parsing the whole repository patch before showing names delays first paint.

Review response. Locate the acquisition boundary in `src/git/mod.rs`, identify the
complete cache or generation key, and prove the outcome under a linked Git worktree.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 2

Counting plus and minus characters mistakes file headers for changes.

Review response. Locate the acquisition boundary in `src/ui/mod.rs`, identify the
complete cache or generation key, and prove the outcome under a cold cache followed by a
warm cache. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

### Risk 3

A batch splitter can attach a rename section to the wrong display path.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a small local repository. Prefer a
test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 4

Syntax parsing can dominate Git execution on generated or minified files.

Review response. Locate the acquisition boundary in `src/git/diff.rs`, identify the
complete cache or generation key, and prove the outcome under a monorepo with many
changed paths. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

### Risk 5

Cloning every cached row doubles memory during combined-document assembly.

Review response. Locate the acquisition boundary in `src/git/mod.rs`, identify the
complete cache or generation key, and prove the outcome under a pull request with
generated files. Prefer a test that asserts state and bounds over one that depends on
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

Evidence 1. parse_numstat rename and binary fixtures. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 2. split_patch_by_file batch fixture. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 3. large patch and long-line grammar limits. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 4. indexed totals independent of loaded patches. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 5. hunk line-number parsing tests. The check should state the repository
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
| 1 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record time to first useful rows |
| 2 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Name-status output creates the file index without requiring patch bodies | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Name-status output creates the file index without requiring patch bodies | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Name-status output creates the file index without requiring patch bodies | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Name-status output creates the file index without requiring patch bodies | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Name-status output creates the file index without requiring patch bodies | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Name-status output creates the file index without requiring patch bodies | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Name-status output creates the file index without requiring patch bodies | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Name-status output creates the file index without requiring patch bodies | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Name-status output creates the file index without requiring patch bodies | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Name-status output creates the file index without requiring patch bodies | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Name-status output creates the file index without requiring patch bodies | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Name-status output creates the file index without requiring patch bodies | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Name-status output creates the file index without requiring patch bodies | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Name-status output creates the file index without requiring patch bodies | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Name-status output creates the file index without requiring patch bodies | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Name-status output creates the file index without requiring patch bodies | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Name-status output creates the file index without requiring patch bodies | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record time to first useful rows |
| 514 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record steady-state frame cost |
| 515 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record stale reply rejection count |
| 520 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a small local repository | Record visible continuity after failure |
| 521 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | Numstat output supplies additions and deletions independently from patch parsing | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | Numstat output supplies additions and deletions independently from patch parsing | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | Numstat output supplies additions and deletions independently from patch parsing | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | Numstat output supplies additions and deletions independently from patch parsing | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | Numstat output supplies additions and deletions independently from patch parsing | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | Numstat output supplies additions and deletions independently from patch parsing | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | Numstat output supplies additions and deletions independently from patch parsing | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 1015 | Numstat output supplies additions and deletions independently from patch parsing | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
