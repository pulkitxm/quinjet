# Git Internals

Quinjet gains speed by asking Git questions in the language of immutable objects, refs,
the index, and path-scoped comparisons instead of reproducing repository semantics.

## Reading map

- [The object model](./object-model.md)
- [Packfiles and deltas](./packfiles-and-deltas.md)
- [Shallow and partial clone](./shallow-and-partial-clone.md)
- [Merge bases and history](./merge-bases-and-history.md)
- [Plumbing and porcelain](./plumbing-and-porcelain.md)
- [Refs, index, and worktrees](./refs-index-and-worktrees.md)

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

- [`src/git/mod.rs`](../../../src/git/mod.rs)
- [`src/git/github/mod.rs`](../../../src/git/github/mod.rs)
- [`src/git/history.rs`](../../../src/git/history.rs)
- [`src/git/status.rs`](../../../src/git/status.rs)

- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)

## Operational contract

1. Blob, tree, commit, and tag objects are content-addressed and immutable once named by
an object ID.

2. Refs are movable names while object IDs are stable cache identities.

3. The index is a staging snapshot distinct from both HEAD and the working tree.

4. A merge base selects the common ancestor used for a pull-request comparison.

5. Packfiles compress storage while Git commands preserve logical object semantics.

6. Partial clone can omit blobs until a command actually needs their contents.

7. Pathspecs let Git prune comparison output to the file the reader requested.

8. Quinjet passes argv directly so Git, not a shell, interprets revisions and paths.

## Git and systems foundations

### 1. The three trees

HEAD, the index, and the working tree represent committed, staged, and filesystem state.
Separate comparisons between these trees are what produce staged and unstaged views.

For git storage and object model, this model matters because blob, tree, commit, and tag
objects are content-addressed and immutable once named by an object id. The boundary is
semantic as well as computational: an optimization is invalid if it answers a cheaper
but different Git question.

### 2. Merge-base semantics

A pull-request diff starts at the best common ancestor and ends at the head commit. This
isolates the contribution from unrelated changes later added to the base branch.

For git storage and object model, this model matters because refs are movable names
while object ids are stable cache identities. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 3. Path-limited diff

A pathspec narrows diff output after the comparison endpoints are fixed. It preserves
Git semantics while avoiding patch generation for files the current interaction does not
need.

For git storage and object model, this model matters because the index is a staging
snapshot distinct from both head and the working tree. The boundary is semantic as well
as computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 4. Machine protocols

NUL-delimited status and diff-index records separate paths without relying on quoting or
locale. Explicit pretty-format delimiters do the same for commit history fields and
records.

For git storage and object model, this model matters because a merge base selects the
common ancestor used for a pull-request comparison. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 5. Partial clone

Blob filtering permits commits and trees to arrive without every file body. This is
valuable only when later commands also avoid accidentally demanding all omitted blobs.

For git storage and object model, this model matters because packfiles compress storage
while git commands preserve logical object semantics. The boundary is semantic as well
as computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 6. Pack storage

Loose objects and packfiles are storage details behind the same object database.
Delegating to Git lets Quinjet benefit from delta compression and repository maintenance
without reimplementing them.

For git storage and object model, this model matters because partial clone can omit
blobs until a command actually needs their contents. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 7. Diffcore

Git transforms raw tree differences through rename detection and other diffcore stages
before formatting a patch. Quinjet consumes the resulting machine and patch formats
instead of approximating those rules.

For git storage and object model, this model matters because pathspecs let git prune
comparison output to the file the reader requested. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 8. Index locking

Many mutations lock and rewrite the index. Read-only commands set GIT_OPTIONAL_LOCKS to
zero so background inspection avoids optional lock traffic and interference.

For git storage and object model, this model matters because quinjet passes argv
directly so git, not a shell, interprets revisions and paths. The boundary is semantic
as well as computational: an optimization is invalid if it answers a cheaper but
different Git question.

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

### Mechanism 1: Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID

Mechanics. Blob, tree, commit, and tag objects are content-addressed and immutable once
named by an object ID. The relevant flow begins in src/git/github/mod.rs and crosses
only the layers needed to preserve the shared command and session boundary.

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

Review evidence. Inspect `src/git/github/mod.rs`, exercise root-commit and
unborn-repository branches, and record steady-state frame cost. Compare the cold and
warm paths because cache and workspace reuse intentionally make them different.

### Mechanism 2: Refs are movable names while object IDs are stable cache identities

Mechanics. Refs are movable names while object IDs are stable cache identities. The
relevant flow begins in src/git/history.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/history.rs`, exercise PreparedRepository local versus
temporary selection, and record bytes accepted from child stdout. Compare the cold and
warm paths because cache and workspace reuse intentionally make them different.

### Mechanism 3: The index is a staging snapshot distinct from both HEAD and the working tree

Mechanics. The index is a staging snapshot distinct from both HEAD and the working tree.
The relevant flow begins in src/git/status.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/status.rs`, exercise merge-base resolution and
fallback tests, and record number of Git and gh processes. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 4: A merge base selects the common ancestor used for a pull-request comparison

Mechanics. A merge base selects the common ancestor used for a pull-request comparison.
The relevant flow begins in src/git/mod.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/mod.rs`, exercise path safety and argv construction
tests, and record maximum retained document bytes. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 5: Packfiles compress storage while Git commands preserve logical object semantics

Mechanics. Packfiles compress storage while Git commands preserve logical object
semantics. The relevant flow begins in src/git/github/mod.rs and crosses only the layers
needed to preserve the shared command and session boundary.

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

Review evidence. Inspect `src/git/github/mod.rs`, exercise Repository::resolve_revision
and rev_parse, and record cache hit identity and disposition. Compare the cold and warm
paths because cache and workspace reuse intentionally make them different.

### Mechanism 6: Partial clone can omit blobs until a command actually needs their contents

Mechanics. Partial clone can omit blobs until a command actually needs their contents.
The relevant flow begins in src/git/history.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/history.rs`, exercise root-commit and
unborn-repository branches, and record stale reply rejection count. Compare the cold and
warm paths because cache and workspace reuse intentionally make them different.

### Mechanism 7: Pathspecs let Git prune comparison output to the file the reader requested

Mechanics. Pathspecs let Git prune comparison output to the file the reader requested.
The relevant flow begins in src/git/status.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/status.rs`, exercise PreparedRepository local versus
temporary selection, and record visible continuity after failure. Compare the cold and
warm paths because cache and workspace reuse intentionally make them different.

### Mechanism 8: Quinjet passes argv directly so Git, not a shell, interprets revisions and paths

Mechanics. Quinjet passes argv directly so Git, not a shell, interprets revisions and
paths. The relevant flow begins in src/git/mod.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/mod.rs`, exercise merge-base resolution and fallback
tests, and record time to first useful rows. Compare the cold and warm paths because
cache and workspace reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Blob, tree,
commit, and tag objects are content-addressed and immutable once named by an object ID.
Capture steady-state frame cost before changing the implementation, then repeat with the
same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 2: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Blob,
tree, commit, and tag objects are content-addressed and immutable once named by an
object ID. Capture bytes accepted from child stdout before changing the implementation,
then repeat with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 3: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: Blob,
tree, commit, and tag objects are content-addressed and immutable once named by an
object ID. Capture number of Git and gh processes before changing the implementation,
then repeat with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 4: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: Blob, tree,
commit, and tag objects are content-addressed and immutable once named by an object ID.
Capture maximum retained document bytes before changing the implementation, then repeat
with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 5: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Blob, tree,
commit, and tag objects are content-addressed and immutable once named by an object ID.
Capture cache hit identity and disposition before changing the implementation, then
repeat with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 6: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Blob, tree,
commit, and tag objects are content-addressed and immutable once named by an object ID.
Capture stale reply rejection count before changing the implementation, then repeat with
the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 7: A Linked Git Worktree

Start with a linked Git worktree. The mechanism under inspection is: Blob, tree, commit,
and tag objects are content-addressed and immutable once named by an object ID. Capture
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
Blob, tree, commit, and tag objects are content-addressed and immutable once named by an
object ID. Capture time to first useful rows before changing the implementation, then
repeat with the same repository identity and selection path after the change.

The expected result is not merely lower elapsed time. Repository state must remain
unchanged for reads, the visible selection must still name the intended item, caps must
still terminate acquisition, and a delayed reply must be rejected if the user has moved
to a different generation.

Probe the failure path by removing one assumption at a time: make the cache cold, hide
an object, slow GitHub, return an oversized response, or change the active selection.
The result should match the documented degradation contract rather than falling back to
unbounded work.

### Scenario 9: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Refs are movable
names while object IDs are stable cache identities. Capture steady-state frame cost
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

### Scenario 10: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: Refs
are movable names while object IDs are stable cache identities. Capture bytes accepted
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

Start with a pull request with generated files. The mechanism under inspection is: Refs
are movable names while object IDs are stable cache identities. Capture number of Git
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

Start with a deeply diverged branch. The mechanism under inspection is: Refs are movable
names while object IDs are stable cache identities. Capture maximum retained document
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

### Scenario 13: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: Refs are
movable names while object IDs are stable cache identities. Capture cache hit identity
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

### Scenario 14: Rapid Keyboard Navigation

Start with rapid keyboard navigation. The mechanism under inspection is: Refs are
movable names while object IDs are stable cache identities. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: Refs are movable
names while object IDs are stable cache identities. Capture visible continuity after
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
Refs are movable names while object IDs are stable cache identities. Capture time to
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

### Scenario 17: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: The index is a
staging snapshot distinct from both HEAD and the working tree. Capture steady-state
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

Start with a monorepo with many changed paths. The mechanism under inspection is: The
index is a staging snapshot distinct from both HEAD and the working tree. Capture bytes
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

Start with a pull request with generated files. The mechanism under inspection is: The
index is a staging snapshot distinct from both HEAD and the working tree. Capture number
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

### Scenario 20: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: The index is a
staging snapshot distinct from both HEAD and the working tree. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: The index
is a staging snapshot distinct from both HEAD and the working tree. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: The index is a
staging snapshot distinct from both HEAD and the working tree. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: The index is a
staging snapshot distinct from both HEAD and the working tree. Capture visible
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

Start with a cold cache followed by a warm cache. The mechanism under inspection is: The
index is a staging snapshot distinct from both HEAD and the working tree. Capture time
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

### Scenario 25: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: A merge base
selects the common ancestor used for a pull-request comparison. Capture steady-state
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

Start with a monorepo with many changed paths. The mechanism under inspection is: A
merge base selects the common ancestor used for a pull-request comparison. Capture bytes
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

Start with a pull request with generated files. The mechanism under inspection is: A
merge base selects the common ancestor used for a pull-request comparison. Capture
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

Start with a deeply diverged branch. The mechanism under inspection is: A merge base
selects the common ancestor used for a pull-request comparison. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: A merge
base selects the common ancestor used for a pull-request comparison. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: A merge base
selects the common ancestor used for a pull-request comparison. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: A merge base
selects the common ancestor used for a pull-request comparison. Capture visible
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

Start with a cold cache followed by a warm cache. The mechanism under inspection is: A
merge base selects the common ancestor used for a pull-request comparison. Capture time
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

Treating a branch name as immutable makes cache entries lie after the ref moves.

Review response. Locate the acquisition boundary in `src/git/github/mod.rs`, identify
the complete cache or generation key, and prove the outcome under a deeply diverged
branch. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 2

Comparing tips instead of a merge base includes unrelated base-branch work.

Review response. Locate the acquisition boundary in `src/git/history.rs`, identify the
complete cache or generation key, and prove the outcome under a slow or unavailable
network. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 3

Materializing every blob defeats a blob-filtered temporary repository.

Review response. Locate the acquisition boundary in `src/git/status.rs`, identify the
complete cache or generation key, and prove the outcome under rapid keyboard navigation.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 4

Shell interpolation changes the meaning and safety of path arguments.

Review response. Locate the acquisition boundary in `src/git/mod.rs`, identify the
complete cache or generation key, and prove the outcome under a linked Git worktree.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 5

Assuming every repository has HEAD breaks unborn repositories.

Review response. Locate the acquisition boundary in `src/git/github/mod.rs`, identify
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

Evidence 1. Repository::resolve_revision and rev_parse. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 2. root-commit and unborn-repository branches. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 3. PreparedRepository local versus temporary selection. The check should state
the repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 4. merge-base resolution and fallback tests. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 5. path safety and argv construction tests. The check should state the
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
| 1 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record time to first useful rows |
| 2 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Blob, tree, commit, and tag objects are content-addressed and immutable once named by an object ID | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record time to first useful rows |
| 514 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record steady-state frame cost |
| 515 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record stale reply rejection count |
| 520 | Refs are movable names while object IDs are stable cache identities | Check latency in a small local repository | Record visible continuity after failure |
| 521 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | Refs are movable names while object IDs are stable cache identities | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | Refs are movable names while object IDs are stable cache identities | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | Refs are movable names while object IDs are stable cache identities | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | Refs are movable names while object IDs are stable cache identities | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | Refs are movable names while object IDs are stable cache identities | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | Refs are movable names while object IDs are stable cache identities | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | Refs are movable names while object IDs are stable cache identities | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | Refs are movable names while object IDs are stable cache identities | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | Refs are movable names while object IDs are stable cache identities | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | Refs are movable names while object IDs are stable cache identities | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | Refs are movable names while object IDs are stable cache identities | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | Refs are movable names while object IDs are stable cache identities | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | Refs are movable names while object IDs are stable cache identities | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | Refs are movable names while object IDs are stable cache identities | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | Refs are movable names while object IDs are stable cache identities | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 1015 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 1016 | Refs are movable names while object IDs are stable cache identities | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
