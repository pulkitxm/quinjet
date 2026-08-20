# GitHub Optimization

A pull-request workspace prefers objects already in the repository and falls back to an
isolated bare repository, keeping previews network-light and mutation-free.

## Reading map

- [Pull-request workspace](./pr-workspace.md)
- [GitHub API strategy](./api-strategy.md)
- [Caching](./caching.md)
- [Background prefetch](./prefetch.md)
- [Conversation, checks, and logs](./conversation-and-checks.md)

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

- [`src/git/github/mod.rs`](../../../src/git/github/mod.rs)
- [`src/cli/command.rs`](../../../src/cli/command.rs)
- [`src/app.rs`](../../../src/app.rs)
- [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)

## Operational contract

1. Immutable base and head object IDs define the comparison independently from branch
movement.

2. The opened repository is reused when it already contains both required commits.

3. A bare temporary repository isolates fetched refs from the user's repository.

4. The pull-request head ref is fetched shallowly with blob filtering.

5. The workspace persists while the reader selects files and is removed on drop.

6. One prepared workspace serves terminal requests through a session-owned handle.

7. Every explicit GitHub operation carries the canonical base repository identity.

8. No preview creates a checkout, worktree, branch, index entry, or user-visible ref.

## Git and systems foundations

### 1. Pack storage

Loose objects and packfiles are storage details behind the same object database.
Delegating to Git lets Quinjet benefit from delta compression and repository maintenance
without reimplementing them.

For pull request workspace, this model matters because immutable base and head object
ids define the comparison independently from branch movement. The boundary is semantic
as well as computational: an optimization is invalid if it answers a cheaper but
different Git question.

### 2. Diffcore

Git transforms raw tree differences through rename detection and other diffcore stages
before formatting a patch. Quinjet consumes the resulting machine and patch formats
instead of approximating those rules.

For pull request workspace, this model matters because the opened repository is reused
when it already contains both required commits. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 3. Index locking

Many mutations lock and rewrite the index. Read-only commands set GIT_OPTIONAL_LOCKS to
zero so background inspection avoids optional lock traffic and interference.

For pull request workspace, this model matters because a bare temporary repository
isolates fetched refs from the user's repository. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 4. Revision resolution

Revision syntax can name refs, ancestors, and object IDs. Quinjet validates user-facing
revision categories and passes argv directly, leaving resolution to Git without shell
interpretation.

For pull request workspace, this model matters because the pull-request head ref is
fetched shallowly with blob filtering. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 5. Content identity

When a cache key contains every immutable input to a computation, freshness becomes a
property of identity rather than elapsed time. Time-to-live remains appropriate only for
facts that can change under the same key.

For pull request workspace, this model matters because the workspace persists while the
reader selects files and is removed on drop. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 6. Objects and snapshots

Git stores file contents as blobs, directory snapshots as trees, and history nodes as
commits. A commit names a tree and parent commits, so comparing commits is fundamentally
comparing immutable snapshots.

For pull request workspace, this model matters because one prepared workspace serves
terminal requests through a session-owned handle. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 7. Refs and object IDs

A ref such as a branch is a movable name. An object ID identifies immutable content.
Quinjet uses refs for user intent and resolved object IDs for workspaces and persistent
cache identity.

For pull request workspace, this model matters because every explicit github operation
carries the canonical base repository identity. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

### 8. The three trees

HEAD, the index, and the working tree represent committed, staged, and filesystem state.
Separate comparisons between these trees are what produce staged and unstaged views.

For pull request workspace, this model matters because no preview creates a checkout,
worktree, branch, index entry, or user-visible ref. The boundary is semantic as well as
computational: an optimization is invalid if it answers a cheaper but different Git
question.

## Representative Git command shapes

### Command 1: Revision validation

```bash
git rev-parse --verify --quiet REVISION^{commit}
```

This is a conceptual command shape rather than copyable internal tracing output. Git
validates object type and resolves revision syntax without a checkout. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 2: Status snapshot

```bash
git status --porcelain=v2 --branch -z --untracked-files=all --ignore-submodules=none
```

This is a conceptual command shape rather than copyable internal tracing output.
Porcelain version 2 and NUL records provide a stable byte protocol for branch and path
state. Quinjet constructs the real argv directly and applies operation-specific output
caps and repository context in the implementation.

### Command 3: Bounded history page

```bash
git log --topo-order --decorate=short --no-color --skip=N --max-count=N --format=FORMAT REV --
```

This is a conceptual command shape rather than copyable internal tracing output. An
explicit revision and page bound avoid ambient HEAD races and repository-sized output.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 4: Changed-path index

```bash
git diff --name-status -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
path and status index is cheaper to acquire and parse than full patch bodies. Quinjet
constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

### Command 5: Line-count index

```bash
git diff --numstat -z --find-renames BASE HEAD --
```

This is a conceptual command shape rather than copyable internal tracing output. The
same revision range supplies additions and deletions without syntax or hunk parsing.
Quinjet constructs the real argv directly and applies operation-specific output caps and
repository context in the implementation.

## Implementation walkthrough

### Mechanism 1: Immutable base and head object IDs define the comparison independently from branch movement

Mechanics. Immutable base and head object IDs define the comparison independently from
branch movement. The relevant flow begins in src/cli/command.rs and crosses only the
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

Review evidence. Inspect `src/cli/command.rs`, exercise temporary repository diff
preparation test, and record steady-state frame cost. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 2: The opened repository is reused when it already contains both required commits

Mechanics. The opened repository is reused when it already contains both required
commits. The relevant flow begins in src/app.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/app.rs`, exercise canonical repository scope in gh argv
tests, and record bytes accepted from child stdout. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 3: A bare temporary repository isolates fetched refs from the user's repository

Mechanics. A bare temporary repository isolates fetched refs from the user's repository.
The relevant flow begins in ARCHITECTURE.md and crosses only the layers needed to
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

Review evidence. Inspect `ARCHITECTURE.md`, exercise prepared workspace handle
validation, and record number of Git and gh processes. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 4: The pull-request head ref is fetched shallowly with blob filtering

Mechanics. The pull-request head ref is fetched shallowly with blob filtering. The
relevant flow begins in src/git/github/mod.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/git/github/mod.rs`, exercise reader never pushes its own
branch test, and record maximum retained document bytes. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 5: The workspace persists while the reader selects files and is removed on drop

Mechanics. The workspace persists while the reader selects files and is removed on drop.
The relevant flow begins in src/cli/command.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/cli/command.rs`, exercise local object reuse workspace
test, and record cache hit identity and disposition. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 6: One prepared workspace serves terminal requests through a session-owned handle

Mechanics. One prepared workspace serves terminal requests through a session-owned
handle. The relevant flow begins in src/app.rs and crosses only the layers needed to
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

Review evidence. Inspect `src/app.rs`, exercise temporary repository diff preparation
test, and record stale reply rejection count. Compare the cold and warm paths because
cache and workspace reuse intentionally make them different.

### Mechanism 7: Every explicit GitHub operation carries the canonical base repository identity

Mechanics. Every explicit GitHub operation carries the canonical base repository
identity. The relevant flow begins in ARCHITECTURE.md and crosses only the layers needed
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

Review evidence. Inspect `ARCHITECTURE.md`, exercise canonical repository scope in gh
argv tests, and record visible continuity after failure. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

### Mechanism 8: No preview creates a checkout, worktree, branch, index entry, or user-visible ref

Mechanics. No preview creates a checkout, worktree, branch, index entry, or user-visible
ref. The relevant flow begins in src/git/github/mod.rs and crosses only the layers
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

Review evidence. Inspect `src/git/github/mod.rs`, exercise prepared workspace handle
validation, and record time to first useful rows. Compare the cold and warm paths
because cache and workspace reuse intentionally make them different.

## End-to-end scenarios

### Scenario 1: A Small Local Repository

Start with a small local repository. The mechanism under inspection is: Immutable base
and head object IDs define the comparison independently from branch movement. Capture
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
Immutable base and head object IDs define the comparison independently from branch
movement. Capture bytes accepted from child stdout before changing the implementation,
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

Start with a pull request with generated files. The mechanism under inspection is:
Immutable base and head object IDs define the comparison independently from branch
movement. Capture number of Git and gh processes before changing the implementation,
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

Start with a deeply diverged branch. The mechanism under inspection is: Immutable base
and head object IDs define the comparison independently from branch movement. Capture
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

Start with a slow or unavailable network. The mechanism under inspection is: Immutable
base and head object IDs define the comparison independently from branch movement.
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

Start with rapid keyboard navigation. The mechanism under inspection is: Immutable base
and head object IDs define the comparison independently from branch movement. Capture
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

Start with a linked Git worktree. The mechanism under inspection is: Immutable base and
head object IDs define the comparison independently from branch movement. Capture
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
Immutable base and head object IDs define the comparison independently from branch
movement. Capture time to first useful rows before changing the implementation, then
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

Start with a small local repository. The mechanism under inspection is: The opened
repository is reused when it already contains both required commits. Capture
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

### Scenario 10: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: The
opened repository is reused when it already contains both required commits. Capture
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

Start with a pull request with generated files. The mechanism under inspection is: The
opened repository is reused when it already contains both required commits. Capture
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

Start with a deeply diverged branch. The mechanism under inspection is: The opened
repository is reused when it already contains both required commits. Capture maximum
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

Start with a slow or unavailable network. The mechanism under inspection is: The opened
repository is reused when it already contains both required commits. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: The opened
repository is reused when it already contains both required commits. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: The opened
repository is reused when it already contains both required commits. Capture visible
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

Start with a cold cache followed by a warm cache. The mechanism under inspection is: The
opened repository is reused when it already contains both required commits. Capture time
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

Start with a small local repository. The mechanism under inspection is: A bare temporary
repository isolates fetched refs from the user's repository. Capture steady-state frame
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

Start with a monorepo with many changed paths. The mechanism under inspection is: A bare
temporary repository isolates fetched refs from the user's repository. Capture bytes
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

Start with a pull request with generated files. The mechanism under inspection is: A
bare temporary repository isolates fetched refs from the user's repository. Capture
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

Start with a deeply diverged branch. The mechanism under inspection is: A bare temporary
repository isolates fetched refs from the user's repository. Capture maximum retained
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

Start with a slow or unavailable network. The mechanism under inspection is: A bare
temporary repository isolates fetched refs from the user's repository. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: A bare
temporary repository isolates fetched refs from the user's repository. Capture stale
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

Start with a linked Git worktree. The mechanism under inspection is: A bare temporary
repository isolates fetched refs from the user's repository. Capture visible continuity
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

Start with a cold cache followed by a warm cache. The mechanism under inspection is: A
bare temporary repository isolates fetched refs from the user's repository. Capture time
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

Start with a small local repository. The mechanism under inspection is: The pull-request
head ref is fetched shallowly with blob filtering. Capture steady-state frame cost
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

### Scenario 26: A Monorepo With Many Changed Paths

Start with a monorepo with many changed paths. The mechanism under inspection is: The
pull-request head ref is fetched shallowly with blob filtering. Capture bytes accepted
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

### Scenario 27: A Pull Request With Generated Files

Start with a pull request with generated files. The mechanism under inspection is: The
pull-request head ref is fetched shallowly with blob filtering. Capture number of Git
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

### Scenario 28: A Deeply Diverged Branch

Start with a deeply diverged branch. The mechanism under inspection is: The pull-request
head ref is fetched shallowly with blob filtering. Capture maximum retained document
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

### Scenario 29: A Slow Or Unavailable Network

Start with a slow or unavailable network. The mechanism under inspection is: The
pull-request head ref is fetched shallowly with blob filtering. Capture cache hit
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

Start with rapid keyboard navigation. The mechanism under inspection is: The
pull-request head ref is fetched shallowly with blob filtering. Capture stale reply
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

Start with a linked Git worktree. The mechanism under inspection is: The pull-request
head ref is fetched shallowly with blob filtering. Capture visible continuity after
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

Start with a cold cache followed by a warm cache. The mechanism under inspection is: The
pull-request head ref is fetched shallowly with blob filtering. Capture time to first
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

## Failure modes and review responses

### Risk 1

Fetching into the opened repository leaves refs and objects the user did not request.

Review response. Locate the acquisition boundary in `src/cli/command.rs`, identify the
complete cache or generation key, and prove the outcome under a small local repository.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 2

Using ambient gh repository discovery can open the same PR number in the wrong fork.

Review response. Locate the acquisition boundary in `src/app.rs`, identify the complete
cache or generation key, and prove the outcome under a monorepo with many changed paths.
Prefer a test that asserts state and bounds over one that depends on wall-clock timing.

### Risk 3

A temporary clone per selected file converts navigation into repeated network setup.

Review response. Locate the acquisition boundary in `ARCHITECTURE.md`, identify the
complete cache or generation key, and prove the outcome under a pull request with
generated files. Prefer a test that asserts state and bounds over one that depends on
wall-clock timing.

### Risk 4

Using branch names after metadata load permits ref movement to change the comparison.

Review response. Locate the acquisition boundary in `src/git/github/mod.rs`, identify
the complete cache or generation key, and prove the outcome under a deeply diverged
branch. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

### Risk 5

Cleanup that runs before the workspace dies invalidates later path reads.

Review response. Locate the acquisition boundary in `src/cli/command.rs`, identify the
complete cache or generation key, and prove the outcome under a slow or unavailable
network. Prefer a test that asserts state and bounds over one that depends on wall-clock
timing.

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

Evidence 1. local object reuse workspace test. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 2. temporary repository diff preparation test. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 3. canonical repository scope in gh argv tests. The check should state the
repository question, the optimized boundary, the expected bounded behavior, and the
state that must remain unchanged. When the behavior is asynchronous, include both the
accepted reply and a stale or replayed reply.

Evidence 4. prepared workspace handle validation. The check should state the repository
question, the optimized boundary, the expected bounded behavior, and the state that must
remain unchanged. When the behavior is asynchronous, include both the accepted reply and
a stale or replayed reply.

Evidence 5. reader never pushes its own branch test. The check should state the
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
| 1 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record time to first useful rows |
| 2 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record steady-state frame cost |
| 3 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record bytes accepted from child stdout |
| 4 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record number of Git and gh processes |
| 5 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record maximum retained document bytes |
| 6 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record cache hit identity and disposition |
| 7 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record stale reply rejection count |
| 8 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a small local repository | Record visible continuity after failure |
| 9 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 11 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 12 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 13 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 15 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 16 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 17 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record time to first useful rows |
| 18 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record steady-state frame cost |
| 19 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 20 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 21 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 22 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 23 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record stale reply rejection count |
| 24 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a pull request with generated files | Record visible continuity after failure |
| 25 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record time to first useful rows |
| 26 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 27 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 28 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 29 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 31 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 32 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 33 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record time to first useful rows |
| 34 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 35 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 36 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 37 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 38 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 39 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 40 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 41 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 42 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 43 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 44 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 45 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 47 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 48 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 49 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record time to first useful rows |
| 50 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record steady-state frame cost |
| 51 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 52 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 53 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 54 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 55 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record stale reply rejection count |
| 56 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a linked Git worktree | Record visible continuity after failure |
| 57 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 58 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 59 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 60 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 61 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 62 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 63 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 64 | Immutable base and head object IDs define the comparison independently from branch movement | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 65 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record time to first useful rows |
| 66 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record steady-state frame cost |
| 67 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 68 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record number of Git and gh processes |
| 69 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record maximum retained document bytes |
| 70 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 71 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record stale reply rejection count |
| 72 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a small local repository | Record visible continuity after failure |
| 73 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 75 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 76 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 77 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 79 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 80 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 81 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 82 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 83 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 84 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 85 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 86 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 87 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 88 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 89 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 90 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 91 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 92 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 93 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 95 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 96 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 97 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 98 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 99 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 100 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 101 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 102 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 103 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 104 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 105 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 106 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 107 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 108 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 109 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 111 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 112 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 113 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 114 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 115 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 116 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 117 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 118 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 119 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 120 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 121 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 122 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 123 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 124 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 125 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 126 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 127 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 128 | Immutable base and head object IDs define the comparison independently from branch movement | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 129 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record time to first useful rows |
| 130 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record steady-state frame cost |
| 131 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 132 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record number of Git and gh processes |
| 133 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record maximum retained document bytes |
| 134 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 135 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record stale reply rejection count |
| 136 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a small local repository | Record visible continuity after failure |
| 137 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 139 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 140 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 141 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 143 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 144 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 145 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 146 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 147 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 148 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 149 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 150 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 151 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 152 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 153 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 154 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 155 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 156 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 157 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 159 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 160 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 161 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 162 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 163 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 164 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 165 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 166 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 167 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 168 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 169 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 170 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 171 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 172 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 173 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 175 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 176 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 177 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 178 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 179 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 180 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 181 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 182 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 183 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 184 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 185 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 186 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 187 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 188 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 189 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 190 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 191 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 192 | Immutable base and head object IDs define the comparison independently from branch movement | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 193 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record time to first useful rows |
| 194 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record steady-state frame cost |
| 195 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 196 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 197 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 198 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 199 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record stale reply rejection count |
| 200 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a small local repository | Record visible continuity after failure |
| 201 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 203 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 204 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 205 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 207 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 208 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 209 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 210 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 211 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 212 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 213 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 214 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 215 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 216 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 217 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 218 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 219 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 220 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 221 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 223 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 224 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 225 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 226 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 227 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 228 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 229 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 230 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 231 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 232 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 233 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 234 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 235 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 236 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 237 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 239 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 240 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 241 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 242 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 243 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 244 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 245 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 246 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 247 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 248 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 249 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 250 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 251 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 252 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 253 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 254 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 255 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 256 | Immutable base and head object IDs define the comparison independently from branch movement | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 257 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record time to first useful rows |
| 258 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record steady-state frame cost |
| 259 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 260 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 261 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 262 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 263 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record stale reply rejection count |
| 264 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a small local repository | Record visible continuity after failure |
| 265 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 267 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 268 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 269 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 271 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 272 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 273 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 274 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 275 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 276 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 277 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 278 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 279 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 280 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 281 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 282 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 283 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 284 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 285 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 287 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 288 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 289 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 290 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 291 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 292 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 293 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 294 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 295 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 296 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 297 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 298 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 299 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 300 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 301 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 303 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 304 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 305 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 306 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 307 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 308 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 309 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 310 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 311 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 312 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 313 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 314 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 315 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 316 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 317 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 318 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 319 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 320 | Immutable base and head object IDs define the comparison independently from branch movement | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 321 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 322 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 323 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 324 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 325 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 326 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 327 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 328 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 329 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 331 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 332 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 333 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 335 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 336 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 337 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 338 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 339 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 340 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 341 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 342 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 343 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 344 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 345 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 346 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 347 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 348 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 349 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 351 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 352 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 353 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 354 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 355 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 356 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 357 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 358 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 359 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 360 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 361 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 362 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 363 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 364 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 365 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 367 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 368 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 369 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 370 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 371 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 372 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 373 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 374 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 375 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 376 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 377 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 378 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 379 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 380 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 381 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 382 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 383 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 384 | Immutable base and head object IDs define the comparison independently from branch movement | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 385 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record time to first useful rows |
| 386 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record steady-state frame cost |
| 387 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 388 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 389 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 390 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 391 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record stale reply rejection count |
| 392 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a small local repository | Record visible continuity after failure |
| 393 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 395 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 396 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 397 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 399 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 400 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 401 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 402 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 403 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 404 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 405 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 406 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 407 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 408 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 409 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 410 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 411 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 412 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 413 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 415 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 416 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 417 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 418 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 419 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 420 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 421 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 422 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 423 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 424 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 425 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 426 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 427 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 428 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 429 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 430 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 431 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 432 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 433 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 434 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 435 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 436 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 437 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 438 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 439 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 440 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 441 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 442 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 443 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 444 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 445 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 446 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 447 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 448 | Immutable base and head object IDs define the comparison independently from branch movement | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 449 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 450 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 451 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 452 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 453 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 454 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 455 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 456 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 457 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 458 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 459 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 460 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 461 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 462 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 463 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 464 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 465 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 466 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 467 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 468 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 469 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 470 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 471 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 472 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 473 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 474 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 475 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 476 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 477 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 478 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 479 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 480 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 481 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 482 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 483 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 484 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 485 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 486 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 487 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 488 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 489 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 490 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 491 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 492 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 493 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 494 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 495 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 496 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 497 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 498 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 499 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 500 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 501 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 502 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 503 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 504 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
| 505 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record time to first useful rows |
| 506 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record steady-state frame cost |
| 507 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 508 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 509 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 510 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 511 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record stale reply rejection count |
| 512 | Immutable base and head object IDs define the comparison independently from branch movement | Check user-visible continuity in a cold cache followed by a warm cache | Record visible continuity after failure |
| 513 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record time to first useful rows |
| 514 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record steady-state frame cost |
| 515 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record bytes accepted from child stdout |
| 516 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record number of Git and gh processes |
| 517 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record maximum retained document bytes |
| 518 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record cache hit identity and disposition |
| 519 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record stale reply rejection count |
| 520 | The opened repository is reused when it already contains both required commits | Check latency in a small local repository | Record visible continuity after failure |
| 521 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record time to first useful rows |
| 522 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record steady-state frame cost |
| 523 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 524 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record number of Git and gh processes |
| 525 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record maximum retained document bytes |
| 526 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record cache hit identity and disposition |
| 527 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record stale reply rejection count |
| 528 | The opened repository is reused when it already contains both required commits | Check latency in a monorepo with many changed paths | Record visible continuity after failure |
| 529 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record time to first useful rows |
| 530 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record steady-state frame cost |
| 531 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record bytes accepted from child stdout |
| 532 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record number of Git and gh processes |
| 533 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record maximum retained document bytes |
| 534 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record cache hit identity and disposition |
| 535 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record stale reply rejection count |
| 536 | The opened repository is reused when it already contains both required commits | Check latency in a pull request with generated files | Record visible continuity after failure |
| 537 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record time to first useful rows |
| 538 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record steady-state frame cost |
| 539 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record bytes accepted from child stdout |
| 540 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record number of Git and gh processes |
| 541 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record maximum retained document bytes |
| 542 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record cache hit identity and disposition |
| 543 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record stale reply rejection count |
| 544 | The opened repository is reused when it already contains both required commits | Check latency in a deeply diverged branch | Record visible continuity after failure |
| 545 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record time to first useful rows |
| 546 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record steady-state frame cost |
| 547 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record bytes accepted from child stdout |
| 548 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record number of Git and gh processes |
| 549 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record maximum retained document bytes |
| 550 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record cache hit identity and disposition |
| 551 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record stale reply rejection count |
| 552 | The opened repository is reused when it already contains both required commits | Check latency in a slow or unavailable network | Record visible continuity after failure |
| 553 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record time to first useful rows |
| 554 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record steady-state frame cost |
| 555 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record bytes accepted from child stdout |
| 556 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record number of Git and gh processes |
| 557 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record maximum retained document bytes |
| 558 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record cache hit identity and disposition |
| 559 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record stale reply rejection count |
| 560 | The opened repository is reused when it already contains both required commits | Check latency in rapid keyboard navigation | Record visible continuity after failure |
| 561 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record time to first useful rows |
| 562 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record steady-state frame cost |
| 563 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record bytes accepted from child stdout |
| 564 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record number of Git and gh processes |
| 565 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record maximum retained document bytes |
| 566 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record cache hit identity and disposition |
| 567 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record stale reply rejection count |
| 568 | The opened repository is reused when it already contains both required commits | Check latency in a linked Git worktree | Record visible continuity after failure |
| 569 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record time to first useful rows |
| 570 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record steady-state frame cost |
| 571 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 572 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 573 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 574 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 575 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record stale reply rejection count |
| 576 | The opened repository is reused when it already contains both required commits | Check latency in a cold cache followed by a warm cache | Record visible continuity after failure |
| 577 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record time to first useful rows |
| 578 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record steady-state frame cost |
| 579 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record bytes accepted from child stdout |
| 580 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record number of Git and gh processes |
| 581 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record maximum retained document bytes |
| 582 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record cache hit identity and disposition |
| 583 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record stale reply rejection count |
| 584 | The opened repository is reused when it already contains both required commits | Check peak memory in a small local repository | Record visible continuity after failure |
| 585 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record time to first useful rows |
| 586 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record steady-state frame cost |
| 587 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 588 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record number of Git and gh processes |
| 589 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record maximum retained document bytes |
| 590 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record cache hit identity and disposition |
| 591 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record stale reply rejection count |
| 592 | The opened repository is reused when it already contains both required commits | Check peak memory in a monorepo with many changed paths | Record visible continuity after failure |
| 593 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record time to first useful rows |
| 594 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record steady-state frame cost |
| 595 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record bytes accepted from child stdout |
| 596 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record number of Git and gh processes |
| 597 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record maximum retained document bytes |
| 598 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record cache hit identity and disposition |
| 599 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record stale reply rejection count |
| 600 | The opened repository is reused when it already contains both required commits | Check peak memory in a pull request with generated files | Record visible continuity after failure |
| 601 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record time to first useful rows |
| 602 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record steady-state frame cost |
| 603 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record bytes accepted from child stdout |
| 604 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record number of Git and gh processes |
| 605 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record maximum retained document bytes |
| 606 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record cache hit identity and disposition |
| 607 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record stale reply rejection count |
| 608 | The opened repository is reused when it already contains both required commits | Check peak memory in a deeply diverged branch | Record visible continuity after failure |
| 609 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record time to first useful rows |
| 610 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record steady-state frame cost |
| 611 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record bytes accepted from child stdout |
| 612 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record number of Git and gh processes |
| 613 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record maximum retained document bytes |
| 614 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record cache hit identity and disposition |
| 615 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record stale reply rejection count |
| 616 | The opened repository is reused when it already contains both required commits | Check peak memory in a slow or unavailable network | Record visible continuity after failure |
| 617 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record time to first useful rows |
| 618 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record steady-state frame cost |
| 619 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record bytes accepted from child stdout |
| 620 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record number of Git and gh processes |
| 621 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record maximum retained document bytes |
| 622 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record cache hit identity and disposition |
| 623 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record stale reply rejection count |
| 624 | The opened repository is reused when it already contains both required commits | Check peak memory in rapid keyboard navigation | Record visible continuity after failure |
| 625 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record time to first useful rows |
| 626 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record steady-state frame cost |
| 627 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record bytes accepted from child stdout |
| 628 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record number of Git and gh processes |
| 629 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record maximum retained document bytes |
| 630 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record cache hit identity and disposition |
| 631 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record stale reply rejection count |
| 632 | The opened repository is reused when it already contains both required commits | Check peak memory in a linked Git worktree | Record visible continuity after failure |
| 633 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record time to first useful rows |
| 634 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record steady-state frame cost |
| 635 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 636 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 637 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 638 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 639 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record stale reply rejection count |
| 640 | The opened repository is reused when it already contains both required commits | Check peak memory in a cold cache followed by a warm cache | Record visible continuity after failure |
| 641 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record time to first useful rows |
| 642 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record steady-state frame cost |
| 643 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record bytes accepted from child stdout |
| 644 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record number of Git and gh processes |
| 645 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record maximum retained document bytes |
| 646 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record cache hit identity and disposition |
| 647 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record stale reply rejection count |
| 648 | The opened repository is reused when it already contains both required commits | Check network transfer in a small local repository | Record visible continuity after failure |
| 649 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record time to first useful rows |
| 650 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record steady-state frame cost |
| 651 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 652 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record number of Git and gh processes |
| 653 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record maximum retained document bytes |
| 654 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record cache hit identity and disposition |
| 655 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record stale reply rejection count |
| 656 | The opened repository is reused when it already contains both required commits | Check network transfer in a monorepo with many changed paths | Record visible continuity after failure |
| 657 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record time to first useful rows |
| 658 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record steady-state frame cost |
| 659 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record bytes accepted from child stdout |
| 660 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record number of Git and gh processes |
| 661 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record maximum retained document bytes |
| 662 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record cache hit identity and disposition |
| 663 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record stale reply rejection count |
| 664 | The opened repository is reused when it already contains both required commits | Check network transfer in a pull request with generated files | Record visible continuity after failure |
| 665 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record time to first useful rows |
| 666 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record steady-state frame cost |
| 667 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record bytes accepted from child stdout |
| 668 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record number of Git and gh processes |
| 669 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record maximum retained document bytes |
| 670 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record cache hit identity and disposition |
| 671 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record stale reply rejection count |
| 672 | The opened repository is reused when it already contains both required commits | Check network transfer in a deeply diverged branch | Record visible continuity after failure |
| 673 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record time to first useful rows |
| 674 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record steady-state frame cost |
| 675 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record bytes accepted from child stdout |
| 676 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record number of Git and gh processes |
| 677 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record maximum retained document bytes |
| 678 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record cache hit identity and disposition |
| 679 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record stale reply rejection count |
| 680 | The opened repository is reused when it already contains both required commits | Check network transfer in a slow or unavailable network | Record visible continuity after failure |
| 681 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record time to first useful rows |
| 682 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record steady-state frame cost |
| 683 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record bytes accepted from child stdout |
| 684 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record number of Git and gh processes |
| 685 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record maximum retained document bytes |
| 686 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record cache hit identity and disposition |
| 687 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record stale reply rejection count |
| 688 | The opened repository is reused when it already contains both required commits | Check network transfer in rapid keyboard navigation | Record visible continuity after failure |
| 689 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record time to first useful rows |
| 690 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record steady-state frame cost |
| 691 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record bytes accepted from child stdout |
| 692 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record number of Git and gh processes |
| 693 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record maximum retained document bytes |
| 694 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record cache hit identity and disposition |
| 695 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record stale reply rejection count |
| 696 | The opened repository is reused when it already contains both required commits | Check network transfer in a linked Git worktree | Record visible continuity after failure |
| 697 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record time to first useful rows |
| 698 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record steady-state frame cost |
| 699 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 700 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 701 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 702 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 703 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record stale reply rejection count |
| 704 | The opened repository is reused when it already contains both required commits | Check network transfer in a cold cache followed by a warm cache | Record visible continuity after failure |
| 705 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record time to first useful rows |
| 706 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record steady-state frame cost |
| 707 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record bytes accepted from child stdout |
| 708 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record number of Git and gh processes |
| 709 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record maximum retained document bytes |
| 710 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record cache hit identity and disposition |
| 711 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record stale reply rejection count |
| 712 | The opened repository is reused when it already contains both required commits | Check subprocess count in a small local repository | Record visible continuity after failure |
| 713 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record time to first useful rows |
| 714 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record steady-state frame cost |
| 715 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 716 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record number of Git and gh processes |
| 717 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record maximum retained document bytes |
| 718 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record cache hit identity and disposition |
| 719 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record stale reply rejection count |
| 720 | The opened repository is reused when it already contains both required commits | Check subprocess count in a monorepo with many changed paths | Record visible continuity after failure |
| 721 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record time to first useful rows |
| 722 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record steady-state frame cost |
| 723 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record bytes accepted from child stdout |
| 724 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record number of Git and gh processes |
| 725 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record maximum retained document bytes |
| 726 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record cache hit identity and disposition |
| 727 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record stale reply rejection count |
| 728 | The opened repository is reused when it already contains both required commits | Check subprocess count in a pull request with generated files | Record visible continuity after failure |
| 729 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record time to first useful rows |
| 730 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record steady-state frame cost |
| 731 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record bytes accepted from child stdout |
| 732 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record number of Git and gh processes |
| 733 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record maximum retained document bytes |
| 734 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record cache hit identity and disposition |
| 735 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record stale reply rejection count |
| 736 | The opened repository is reused when it already contains both required commits | Check subprocess count in a deeply diverged branch | Record visible continuity after failure |
| 737 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record time to first useful rows |
| 738 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record steady-state frame cost |
| 739 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record bytes accepted from child stdout |
| 740 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record number of Git and gh processes |
| 741 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record maximum retained document bytes |
| 742 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record cache hit identity and disposition |
| 743 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record stale reply rejection count |
| 744 | The opened repository is reused when it already contains both required commits | Check subprocess count in a slow or unavailable network | Record visible continuity after failure |
| 745 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record time to first useful rows |
| 746 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record steady-state frame cost |
| 747 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record bytes accepted from child stdout |
| 748 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record number of Git and gh processes |
| 749 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record maximum retained document bytes |
| 750 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record cache hit identity and disposition |
| 751 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record stale reply rejection count |
| 752 | The opened repository is reused when it already contains both required commits | Check subprocess count in rapid keyboard navigation | Record visible continuity after failure |
| 753 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record time to first useful rows |
| 754 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record steady-state frame cost |
| 755 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record bytes accepted from child stdout |
| 756 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record number of Git and gh processes |
| 757 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record maximum retained document bytes |
| 758 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record cache hit identity and disposition |
| 759 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record stale reply rejection count |
| 760 | The opened repository is reused when it already contains both required commits | Check subprocess count in a linked Git worktree | Record visible continuity after failure |
| 761 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record time to first useful rows |
| 762 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record steady-state frame cost |
| 763 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 764 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 765 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 766 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 767 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record stale reply rejection count |
| 768 | The opened repository is reused when it already contains both required commits | Check subprocess count in a cold cache followed by a warm cache | Record visible continuity after failure |
| 769 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record time to first useful rows |
| 770 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record steady-state frame cost |
| 771 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record bytes accepted from child stdout |
| 772 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record number of Git and gh processes |
| 773 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record maximum retained document bytes |
| 774 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record cache hit identity and disposition |
| 775 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record stale reply rejection count |
| 776 | The opened repository is reused when it already contains both required commits | Check cache correctness in a small local repository | Record visible continuity after failure |
| 777 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record time to first useful rows |
| 778 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record steady-state frame cost |
| 779 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 780 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record number of Git and gh processes |
| 781 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record maximum retained document bytes |
| 782 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record cache hit identity and disposition |
| 783 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record stale reply rejection count |
| 784 | The opened repository is reused when it already contains both required commits | Check cache correctness in a monorepo with many changed paths | Record visible continuity after failure |
| 785 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record time to first useful rows |
| 786 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record steady-state frame cost |
| 787 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record bytes accepted from child stdout |
| 788 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record number of Git and gh processes |
| 789 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record maximum retained document bytes |
| 790 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record cache hit identity and disposition |
| 791 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record stale reply rejection count |
| 792 | The opened repository is reused when it already contains both required commits | Check cache correctness in a pull request with generated files | Record visible continuity after failure |
| 793 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record time to first useful rows |
| 794 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record steady-state frame cost |
| 795 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record bytes accepted from child stdout |
| 796 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record number of Git and gh processes |
| 797 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record maximum retained document bytes |
| 798 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record cache hit identity and disposition |
| 799 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record stale reply rejection count |
| 800 | The opened repository is reused when it already contains both required commits | Check cache correctness in a deeply diverged branch | Record visible continuity after failure |
| 801 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record time to first useful rows |
| 802 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record steady-state frame cost |
| 803 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record bytes accepted from child stdout |
| 804 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record number of Git and gh processes |
| 805 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record maximum retained document bytes |
| 806 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record cache hit identity and disposition |
| 807 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record stale reply rejection count |
| 808 | The opened repository is reused when it already contains both required commits | Check cache correctness in a slow or unavailable network | Record visible continuity after failure |
| 809 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record time to first useful rows |
| 810 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record steady-state frame cost |
| 811 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record bytes accepted from child stdout |
| 812 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record number of Git and gh processes |
| 813 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record maximum retained document bytes |
| 814 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record cache hit identity and disposition |
| 815 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record stale reply rejection count |
| 816 | The opened repository is reused when it already contains both required commits | Check cache correctness in rapid keyboard navigation | Record visible continuity after failure |
| 817 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record time to first useful rows |
| 818 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record steady-state frame cost |
| 819 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record bytes accepted from child stdout |
| 820 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record number of Git and gh processes |
| 821 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record maximum retained document bytes |
| 822 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record cache hit identity and disposition |
| 823 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record stale reply rejection count |
| 824 | The opened repository is reused when it already contains both required commits | Check cache correctness in a linked Git worktree | Record visible continuity after failure |
| 825 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record time to first useful rows |
| 826 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record steady-state frame cost |
| 827 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 828 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 829 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 830 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 831 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record stale reply rejection count |
| 832 | The opened repository is reused when it already contains both required commits | Check cache correctness in a cold cache followed by a warm cache | Record visible continuity after failure |
| 833 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record time to first useful rows |
| 834 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record steady-state frame cost |
| 835 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record bytes accepted from child stdout |
| 836 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record number of Git and gh processes |
| 837 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record maximum retained document bytes |
| 838 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record cache hit identity and disposition |
| 839 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record stale reply rejection count |
| 840 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a small local repository | Record visible continuity after failure |
| 841 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record time to first useful rows |
| 842 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record steady-state frame cost |
| 843 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 844 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record number of Git and gh processes |
| 845 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record maximum retained document bytes |
| 846 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record cache hit identity and disposition |
| 847 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record stale reply rejection count |
| 848 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a monorepo with many changed paths | Record visible continuity after failure |
| 849 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record time to first useful rows |
| 850 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record steady-state frame cost |
| 851 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record bytes accepted from child stdout |
| 852 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record number of Git and gh processes |
| 853 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record maximum retained document bytes |
| 854 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record cache hit identity and disposition |
| 855 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record stale reply rejection count |
| 856 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a pull request with generated files | Record visible continuity after failure |
| 857 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record time to first useful rows |
| 858 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record steady-state frame cost |
| 859 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record bytes accepted from child stdout |
| 860 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record number of Git and gh processes |
| 861 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record maximum retained document bytes |
| 862 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record cache hit identity and disposition |
| 863 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record stale reply rejection count |
| 864 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a deeply diverged branch | Record visible continuity after failure |
| 865 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record time to first useful rows |
| 866 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record steady-state frame cost |
| 867 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record bytes accepted from child stdout |
| 868 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record number of Git and gh processes |
| 869 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record maximum retained document bytes |
| 870 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record cache hit identity and disposition |
| 871 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record stale reply rejection count |
| 872 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a slow or unavailable network | Record visible continuity after failure |
| 873 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record time to first useful rows |
| 874 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record steady-state frame cost |
| 875 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record bytes accepted from child stdout |
| 876 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record number of Git and gh processes |
| 877 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record maximum retained document bytes |
| 878 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record cache hit identity and disposition |
| 879 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record stale reply rejection count |
| 880 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in rapid keyboard navigation | Record visible continuity after failure |
| 881 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record time to first useful rows |
| 882 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record steady-state frame cost |
| 883 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record bytes accepted from child stdout |
| 884 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record number of Git and gh processes |
| 885 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record maximum retained document bytes |
| 886 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record cache hit identity and disposition |
| 887 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record stale reply rejection count |
| 888 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a linked Git worktree | Record visible continuity after failure |
| 889 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record time to first useful rows |
| 890 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record steady-state frame cost |
| 891 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 892 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 893 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 894 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 895 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record stale reply rejection count |
| 896 | The opened repository is reused when it already contains both required commits | Check concurrency ordering in a cold cache followed by a warm cache | Record visible continuity after failure |
| 897 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record time to first useful rows |
| 898 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record steady-state frame cost |
| 899 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record bytes accepted from child stdout |
| 900 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record number of Git and gh processes |
| 901 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record maximum retained document bytes |
| 902 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record cache hit identity and disposition |
| 903 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record stale reply rejection count |
| 904 | The opened repository is reused when it already contains both required commits | Check failure degradation in a small local repository | Record visible continuity after failure |
| 905 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record time to first useful rows |
| 906 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record steady-state frame cost |
| 907 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 908 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record number of Git and gh processes |
| 909 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record maximum retained document bytes |
| 910 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record cache hit identity and disposition |
| 911 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record stale reply rejection count |
| 912 | The opened repository is reused when it already contains both required commits | Check failure degradation in a monorepo with many changed paths | Record visible continuity after failure |
| 913 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record time to first useful rows |
| 914 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record steady-state frame cost |
| 915 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record bytes accepted from child stdout |
| 916 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record number of Git and gh processes |
| 917 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record maximum retained document bytes |
| 918 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record cache hit identity and disposition |
| 919 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record stale reply rejection count |
| 920 | The opened repository is reused when it already contains both required commits | Check failure degradation in a pull request with generated files | Record visible continuity after failure |
| 921 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record time to first useful rows |
| 922 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record steady-state frame cost |
| 923 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record bytes accepted from child stdout |
| 924 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record number of Git and gh processes |
| 925 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record maximum retained document bytes |
| 926 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record cache hit identity and disposition |
| 927 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record stale reply rejection count |
| 928 | The opened repository is reused when it already contains both required commits | Check failure degradation in a deeply diverged branch | Record visible continuity after failure |
| 929 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record time to first useful rows |
| 930 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record steady-state frame cost |
| 931 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record bytes accepted from child stdout |
| 932 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record number of Git and gh processes |
| 933 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record maximum retained document bytes |
| 934 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record cache hit identity and disposition |
| 935 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record stale reply rejection count |
| 936 | The opened repository is reused when it already contains both required commits | Check failure degradation in a slow or unavailable network | Record visible continuity after failure |
| 937 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record time to first useful rows |
| 938 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record steady-state frame cost |
| 939 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record bytes accepted from child stdout |
| 940 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record number of Git and gh processes |
| 941 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record maximum retained document bytes |
| 942 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record cache hit identity and disposition |
| 943 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record stale reply rejection count |
| 944 | The opened repository is reused when it already contains both required commits | Check failure degradation in rapid keyboard navigation | Record visible continuity after failure |
| 945 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record time to first useful rows |
| 946 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record steady-state frame cost |
| 947 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record bytes accepted from child stdout |
| 948 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record number of Git and gh processes |
| 949 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record maximum retained document bytes |
| 950 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record cache hit identity and disposition |
| 951 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record stale reply rejection count |
| 952 | The opened repository is reused when it already contains both required commits | Check failure degradation in a linked Git worktree | Record visible continuity after failure |
| 953 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record time to first useful rows |
| 954 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record steady-state frame cost |
| 955 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record bytes accepted from child stdout |
| 956 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record number of Git and gh processes |
| 957 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record maximum retained document bytes |
| 958 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record cache hit identity and disposition |
| 959 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record stale reply rejection count |
| 960 | The opened repository is reused when it already contains both required commits | Check failure degradation in a cold cache followed by a warm cache | Record visible continuity after failure |
| 961 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record time to first useful rows |
| 962 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record steady-state frame cost |
| 963 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record bytes accepted from child stdout |
| 964 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record number of Git and gh processes |
| 965 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record maximum retained document bytes |
| 966 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record cache hit identity and disposition |
| 967 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record stale reply rejection count |
| 968 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a small local repository | Record visible continuity after failure |
| 969 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record time to first useful rows |
| 970 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record steady-state frame cost |
| 971 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record bytes accepted from child stdout |
| 972 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record number of Git and gh processes |
| 973 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record maximum retained document bytes |
| 974 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record cache hit identity and disposition |
| 975 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record stale reply rejection count |
| 976 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a monorepo with many changed paths | Record visible continuity after failure |
| 977 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record time to first useful rows |
| 978 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record steady-state frame cost |
| 979 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record bytes accepted from child stdout |
| 980 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record number of Git and gh processes |
| 981 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record maximum retained document bytes |
| 982 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record cache hit identity and disposition |
| 983 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record stale reply rejection count |
| 984 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a pull request with generated files | Record visible continuity after failure |
| 985 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record time to first useful rows |
| 986 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record steady-state frame cost |
| 987 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record bytes accepted from child stdout |
| 988 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record number of Git and gh processes |
| 989 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record maximum retained document bytes |
| 990 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record cache hit identity and disposition |
| 991 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record stale reply rejection count |
| 992 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a deeply diverged branch | Record visible continuity after failure |
| 993 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record time to first useful rows |
| 994 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record steady-state frame cost |
| 995 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record bytes accepted from child stdout |
| 996 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record number of Git and gh processes |
| 997 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record maximum retained document bytes |
| 998 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record cache hit identity and disposition |
| 999 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record stale reply rejection count |
| 1000 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a slow or unavailable network | Record visible continuity after failure |
| 1001 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record time to first useful rows |
| 1002 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record steady-state frame cost |
| 1003 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record bytes accepted from child stdout |
| 1004 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record number of Git and gh processes |
| 1005 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record maximum retained document bytes |
| 1006 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record cache hit identity and disposition |
| 1007 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record stale reply rejection count |
| 1008 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in rapid keyboard navigation | Record visible continuity after failure |
| 1009 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record time to first useful rows |
| 1010 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record steady-state frame cost |
| 1011 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record bytes accepted from child stdout |
| 1012 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record number of Git and gh processes |
| 1013 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record maximum retained document bytes |
| 1014 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record cache hit identity and disposition |
| 1015 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record stale reply rejection count |
| 1016 | The opened repository is reused when it already contains both required commits | Check user-visible continuity in a linked Git worktree | Record visible continuity after failure |
