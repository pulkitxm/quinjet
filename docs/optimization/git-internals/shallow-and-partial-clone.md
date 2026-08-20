# Shallow and Partial Clone

Quinjet renders a pull-request diff without cloning the repository that contains it. The whole
trick rests on four transfer mechanisms that Git has grown over the years: protocol v2 with its
scoped ref advertisement, shallow fetching with an explicit history boundary, partial clone
filters that defer object classes to a later lazy fetch, and promisor remotes that make the lazy
fetch automatic. This page explains each mechanism from the wire format up, then walks through
the exact fetch choreography in `src/git/github/mod.rs`: the disposable bare workspace, the
`--filter=blob:none --depth=N` fetch with its unfiltered retry, the depth-1 merge-base point
fetch, the deepening ladder, and the benchmark clone the whole stack was measured against.

## Contents

- [The problem: what a pull-request diff actually needs](#the-problem-what-a-pull-request-diff-actually-needs)
- [Pkt-line: the frame beneath the protocol](#pkt-line-the-frame-beneath-the-protocol)
- [Protocol v2](#protocol-v2)
- [Want/have negotiation](#wanthave-negotiation)
- [Shallow clones and the shallow file](#shallow-clones-and-the-shallow-file)
- [Deepening a shallow history](#deepening-a-shallow-history)
- [Partial clone filters](#partial-clone-filters)
- [Promisor remotes and lazy fetch](#promisor-remotes-and-lazy-fetch)
- [Fetching arbitrary refs and exact commits](#fetching-arbitrary-refs-and-exact-commits)
- [Bare repositories as fetch targets](#bare-repositories-as-fetch-targets)
- [The Quinjet fetch choreography](#the-quinjet-fetch-choreography)
- [A worked trace on the benchmark pull request](#a-worked-trace-on-the-benchmark-pull-request)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [The benchmark clone at /tmp/bun-test](#the-benchmark-clone-at-tmpbun-test)
- [Related pages](#related-pages)

## The problem: what a pull-request diff actually needs

A pull-request diff between a merge base `M` and a head commit `H` is a function of a small,
precisely bounded set of objects:

- the two commit objects `M` and `H` themselves,
- every tree object reachable from either commit's root tree,
- the blobs of paths that differ between the two trees, and nothing else.

Everything else in the repository is dead weight for this view: the rest of the commit history,
the blobs of the tens of thousands of files the pull request does not touch, every tag, every
other branch. A full clone transfers all of it anyway, because the classic fetch protocol
answers one question only: give me everything reachable from these tips that I do not already
have.

The four mechanisms this page covers each cut away one slice of that dead weight:

| Mechanism | What it removes from the transfer |
| --- | --- |
| Protocol v2 ref prefixes | The advertisement of every ref the client did not ask about |
| Shallow fetch (`--depth=N`) | All history older than a chosen boundary |
| Partial clone (`--filter=blob:none`) | Every blob, until something actually reads one |
| Promisor lazy fetch | Nothing up front; it is the deferred bill for the filter |

Quinjet has one extra constraint that shapes everything: it must never mutate the repository the
user opened. ARCHITECTURE.md invariant 9 ends with the sentence "The opened repository receives
no checkout, branch, ref, index, or worktree mutation." Fetching a pull request's refs into the
user's clone would violate that, so all network fetching happens inside a disposable bare
repository under the cache root, and the opened repository is only ever read.

The decision between reading locally and fetching remotely is the first thing
`prepare_pull_request_diff` in `src/git/github/mod.rs:767` does:

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
```

The first branch is the network-free path: when both the base and head object ids already exist
in the opened repository (`has_commit` runs `git cat-file -e <oid>^{commit}`, see
`src/git/mod.rs:790`), the diff is computed entirely locally and none of the machinery on this
page runs at all. That is why previewing a pull request for a branch built locally, or one that
was merged with a merge commit, is instant. The second branch is the subject of this page: an
isolated bare workspace, two API hints, and a sequence of shallow partial fetches designed to
transfer the minimum the diff requires.

The benchmark that stress-tested all of this is oven-sh/bun#30412, "Rewrite Bun in Rust": 2,188
changed files and +1,009,257 added lines. A repository of that size makes every wasted object
class visible in wall-clock time, which is why the session notes cited throughout this page
measured against it. The clone it was driven from, a shallow single-branch `blob:none` clone,
occupies 389 MB on disk; that number alone shows how much a filter removes, because it contains
the full current tree and main-branch tip history of a very large project.

## Pkt-line: the frame beneath the protocol

Every byte of the fetch conversation, in both directions, travels inside pkt-line frames. The
format is minimal and worth knowing exactly, because every capability list, every `want`, every
shallow boundary announcement, and the packfile itself are all pkt-lines. The authoritative
description is the [gitprotocol-pack](https://git-scm.com/docs/gitprotocol-pack) and
[gitprotocol-v2](https://git-scm.com/docs/gitprotocol-v2) manual pages.

### Frame layout

| Offset | Size | Content |
| --- | --- | --- |
| 0 | 4 bytes | ASCII lowercase hexadecimal length of the whole frame, including these 4 bytes |
| 4 | 0 to 65516 bytes | Payload, conventionally ending in `\n` for text payloads |

The length field counts itself, so the smallest data-bearing frame is `0005` (four bytes of
length plus one payload byte) and the largest legal frame is `fff0` hex, 65520 bytes: 65516
bytes of payload. A sender must split anything larger across frames; the packfile stream is
routinely thousands of frames.

Three lengths below `0005` are reserved as control packets rather than data:

| Frame | Name | Meaning |
| --- | --- | --- |
| `0000` | flush-pkt | End of a message or section; the peer may act on everything received |
| `0001` | delim-pkt | Protocol v2 only: separates a command's capabilities from its arguments |
| `0002` | response-end-pkt | Protocol v2 over HTTP: marks the end of a stateless response |

### Worked encoding example

The line a client sends to request one object, `want <oid>`, encodes like this for an
illustrative object id (the SHA-1 of the string `test`):

```text
payload:  want a94a8fe5ccb19ba61c4c0873d391e987982fbbd3\n
          5 + 40 + 1 = 46 payload bytes
frame:    46 + 4 = 50 = 0x0032

on wire:  0032want a94a8fe5ccb19ba61c4c0873d391e987982fbbd3\n
```

A complete minimal request section, a `want` followed by `done` and a flush, is therefore:

```text
0032want a94a8fe5ccb19ba61c4c0873d391e987982fbbd3\n
0009done\n
0000
```

Nothing in the frame says what the payload means; meaning comes entirely from position in the
conversation. This is why the protocol documents are written as grammars over pkt-lines rather
than over bytes.

### Sideband multiplexing

The final section of a fetch response carries the packfile, and it is multiplexed so the server
can interleave progress text and errors with pack data. Inside the packfile section, the first
payload byte of every pkt-line is a stream code:

| Code | Stream | Content |
| --- | --- | --- |
| `1` | pack data | Raw bytes of the packfile, to be concatenated in order |
| `2` | progress | Human-readable progress, written to the client's stderr |
| `3` | fatal error | An error message; the client aborts the fetch |

Quinjet passes `--quiet` on every fetch it issues, which asks the server for `no-progress`: the
side channel 2 traffic is suppressed at the source rather than transferred and discarded. The
pack bytes on stream 1 are what the packfile pages describe; see
[packfiles and deltas](./packfiles-and-deltas.md) for what happens to them after they land.

## Protocol v2

Protocol v2, specified in [gitprotocol-v2](https://git-scm.com/docs/gitprotocol-v2), reorganized
the fetch conversation from a fixed script into a command-response model. It matters to Quinjet
for one big reason and several small ones: the big one is that the client controls which refs
are advertised, so fetching one pull-request ref from a repository with an enormous ref
namespace no longer starts by downloading the entire ref listing.

### Discovery and transport

The client requests version 2 out of band, so that a v0-only server simply ignores the request
and the conversation degrades gracefully:

- Over SSH and `git://`, the client sets the `GIT_PROTOCOL=version=2` environment variable or
  the equivalent extra parameter after the NUL in the git-daemon request.
- Over smart HTTP, the client sends the header `Git-Protocol: version=2` on the initial
  `GET <url>/info/refs?service=git-upload-pack` request, and each subsequent command is a
  `POST <url>/git-upload-pack` whose body is the pkt-line request. HTTP is stateless, so every
  POST must restate everything the server needs; this is why v2 was designed around
  self-contained commands.

### The capability advertisement

Instead of v0's combined ref-plus-capability advertisement, a v2 server answers discovery with
capabilities only. A representative advertisement:

```text
000eversion 2\n
0015agent=git/2.43.0\n
0013ls-refs=unborn\n
0027fetch=shallow wait-for-done filter\n
0012server-option\n
0017object-format=sha1\n
0000
```

Each capability is a key, optionally `key=value`, and the value can itself be a space-separated
feature list. The two capabilities Quinjet's workflow depends on are visible here: `ls-refs` (the
scoped ref listing command) and the `fetch` command with its `shallow` and `filter` features. A
server that does not list `filter` under `fetch` cannot serve a partial clone; that exact
situation is why Quinjet's fetch has an unfiltered retry, covered below.

### Command requests

Every v2 request has the same shape: a command name, capability values, a delim-pkt, command
arguments, and a flush:

```text
command-request = command capability-list delim-pkt command-args flush-pkt

0014command=ls-refs\n
0015agent=git/2.43.0\n
0017object-format=sha1\n
0001
...arguments...
0000
```

The `0001` delim-pkt is the v2-specific frame from the pkt-line table: it separates the fixed
preamble from the argument list so both sides can parse without lookahead.

### ls-refs

`ls-refs` replaces the v0 ref advertisement with a query. Its arguments:

| Argument | Effect |
| --- | --- |
| `symrefs` | Annotate symbolic refs with their target (`symref-target:refs/heads/main`) |
| `peel` | Annotate annotated tags with the commit they peel to (`peeled:<oid>`) |
| `ref-prefix <prefix>` | Only advertise refs whose names start with the prefix; repeatable |
| `unborn` | Report an unborn HEAD and the branch it points at |

When `git fetch` runs with an explicit refspec, it sends a `ref-prefix` for the refspec's
source. Fetching Quinjet's pull-request head ref therefore produces a request like:

```text
0014command=ls-refs\n
0015agent=git/2.43.0\n
0017object-format=sha1\n
0001
0009peel\n
000csymrefs\n
0024ref-prefix refs/pull/30412/head\n
0000
```

and the server answers with just the matching refs, one `<oid> <refname>` line each, instead of
the full namespace. On a repository like oven-sh/bun, whose `refs/pull/` hierarchy alone holds
two synthetic refs for every pull request ever opened, the difference between "every ref" and
"the one ref I asked about" is the difference between an advertisement measured in megabytes and
one measured in a few dozen bytes. Quinjet issues at most a handful of fetches per pull-request
load, each with exactly one refspec, so every one of them enjoys a minimal advertisement.

### The fetch command

The v2 `fetch` command carries the actual object negotiation. Its argument vocabulary is the
compact union of everything the v0 protocol grew over the years:

| Argument | Meaning |
| --- | --- |
| `want <oid>` | An object the client wants; not limited to advertised objects |
| `want-ref <ref>` | Want a ref by name; the server reports its resolved oid (`ref-in-want`) |
| `have <oid>` | An object the client already has, for common-ancestor discovery |
| `done` | The client will send no more haves; produce the pack now |
| `thin-pack` | Allow deltas against objects the client already has |
| `ofs-delta` | Allow offset deltas in the pack (see the packfile page) |
| `no-progress` | Suppress sideband-2 progress traffic |
| `include-tag` | Include annotated tags that point into the transferred history |
| `shallow <oid>` | A commit the client currently has as a shallow boundary |
| `deepen <depth>` | Request history to this depth measured from the remote tips |
| `deepen-relative` | Measure `deepen` from the client's current boundary instead |
| `deepen-since <time>` | Boundary by committer date instead of count |
| `deepen-not <rev>` | Exclude history reachable from a rev |
| `filter <spec>` | Partial clone: omit objects per the filter spec |
| `wait-for-done` | Negotiation tuning: the server ACKs but sends no pack until `done` |

Three of these rows are the levers this page is about: `deepen` (shallow), `filter` (partial),
and the wording of `want`, which the specification defines as not limited to advertised
objects, the hook that exact-commit fetching hangs from.

### The fetch response

The response is a sequence of sections, each introduced by a one-line header and separated by
delim-pkts, ending in a flush:

```text
acknowledgments        ACK <oid> / NAK / ready
shallow-info           shallow <oid> / unshallow <oid> lines
wanted-refs            <oid> <refname> for each want-ref
packfile-uris          optional out-of-band pack URLs
packfile               sideband-multiplexed pack data
```

The `shallow-info` section is how the client learns its new history boundary after a `deepen`
request; its `shallow` and `unshallow` lines drive the rewrite of the shallow file described in
the next section but one. The `packfile` section is the payload, framed with the sideband codes
from the pkt-line section above.

### What v2 buys Quinjet

**1. Scoped advertisements make many small fetches cheap.** Quinjet's choreography deliberately
issues several tiny fetches (head, merge-base point fetch, base, ladder rounds) rather than one
big one. Under v0, each of those would re-download the full ref advertisement; under v2, each
sends one `ref-prefix` and receives one line back. The design of `fetch_pull_request` in
`src/git/github/mod.rs:1781` assumes this is cheap, and under v2 it is.

**2. The capability list is inspectable per feature.** The `fetch=shallow wait-for-done filter`
value tells the client up front whether `filter` will be honored. When it is absent, Git fails
the filtered fetch rather than silently transferring blobs, and that clean failure is exactly
what Quinjet's retry-without-filter path catches.

**3. Statelessness matches subprocess usage.** Quinjet spawns a fresh `git fetch` process per
step (see [plumbing and porcelain](./plumbing-and-porcelain.md) for the substrate). Protocol v2
over HTTPS was designed for stateless round trips, so a sequence of independent subprocess
fetches against the same remote is not fighting the protocol's grain.

## Want/have negotiation

Negotiation is the part of the fetch conversation that decides how big the pack will be. The
client names what it wants; then client and server search for commits they both have, so the
server can send only the difference.

### The mechanics

The client walks its own ref tips backwards through history, sending `have <oid>` lines in
batches. The server answers each batch with acknowledgments: `ACK <oid>` for commits it also
has, `NAK` when a batch contains nothing in common. Once the server has seen enough common
commits to compute a cut ("ready" in v2's acknowledgments section), the client sends `done` and
the server emits a pack containing, roughly, the objects reachable from the wants minus the
objects reachable from the acknowledged commons.

The batching is adaptive: Git sends a small first block of haves and grows the block size as
rounds proceed, so a fetch between closely related repositories converges in one or two rounds
while a fetch between distant ones does not flood the server. The algorithm is selectable via
`fetch.negotiationAlgorithm` (the default consecutive walk, a `skipping` variant that strides
exponentially through history for faraway peers, and `noop` which sends no haves at all).

### Negotiation from an empty repository

Quinjet's disposable workspace is created by `git init --bare` seconds before the first fetch.
An empty repository has no refs, therefore no tips to walk, therefore nothing to say in the
`have` phase: negotiation collapses to a single round of wants followed immediately by `done`.
Two consequences follow:

- There is no round-trip cost to negotiate away; the fetch's cost is entirely pack composition
  and transfer. Every optimization must therefore come from shrinking what is wanted, which is
  exactly what `--depth` and `--filter` do.
- The server cannot be told "I already have most of these objects on this machine" through refs
  the workspace does not have. The alternates link described next is what re-introduces that
  knowledge.

### Negotiation and alternates

An alternates file (see the bare-repository section below for how Quinjet writes one) gives a
repository read access to another object store on the same filesystem. Git's fetch machinery
takes alternates into account during negotiation: the tips of refs found in the alternate store
are fed to the negotiator as candidate common commits, so a fetch can advertise history that
lives only in the borrowed store. Independently of negotiation, any object that already exists
locally, including through an alternate, satisfies object lookups directly, so the lazy fetch
machinery of a partial clone never asks the network for a blob the user's clone already holds.

The doc comment on the borrowing function in `src/git/github/mod.rs:1728` states the intent in
one breath: "A merged or locally built pull request usually already has most of its blobs on
disk under other refs, so lazy blob reads resolve from the local store instead of the network.
The opened repository is only read."

## Shallow clones and the shallow file

A shallow repository is one whose history has an artificial floor. Commits at the floor are
present, but their parents are not, and Git records that fact explicitly so every later
operation knows where traversal must stop.

### What --depth changes on the wire

`git fetch --depth=N <remote> <refspec>` adds `deepen N` to the fetch command. The server
counts N commits down from each wanted tip along first-parent-and-all-parents traversal, sends
only that slice of history, and reports the cut points in the `shallow-info` section:

- `shallow <oid>`: this commit is included, but its parents were withheld; treat it as a
  boundary.
- `unshallow <oid>`: this commit used to be a boundary in your repository, but this fetch
  supplied its history; stop treating it as one.

`--depth=1` is the degenerate and most useful case: exactly the tip commit, no parents at all.
Quinjet's merge-base point fetch uses precisely this to materialize one known commit.

### The shallow file

The boundary lives in a plain text file, `shallow`, directly inside the Git directory (the
repository root of a bare repository, or the common directory of a worktree checkout; see
[gitrepository-layout](https://git-scm.com/docs/gitrepository-layout)). Its format is one full
hexadecimal object id per line, nothing else:

```text
$GIT_DIR/shallow

1c0f1c9d0e6c8ff45b0aa8dc9e6b3a3a3d2f1a9b
9b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c
```

Semantically the file is a set of graft points: when the revision machinery loads a commit whose
id appears in `shallow`, it presents that commit as if it had no parents, regardless of the
parent ids actually recorded in the commit object. The commit object itself is unmodified; only
traversal is clipped. Updates to the file are written atomically through a `shallow.lock`
temporary, driven by the `shallow`/`unshallow` lines of each fetch response, so a failed fetch
never leaves a half-rewritten boundary.

Two plumbing surfaces expose the state:

- `git rev-parse --is-shallow-repository` prints `true`/`false`. The session notes record
  exactly this check against the benchmark clone: "/tmp/bun-test, 389 MB on disk,
  `git rev-parse --is-shallow-repository` = true".
- The file's existence is the flag; deleting the last boundary entry (as an `--unshallow` fetch
  eventually does) removes the file and the repository becomes a normal full one.

### What breaks under a shallow boundary

Shallowness is invisible to most read operations, but a few care deeply, and one of them sits at
the center of this page's subject:

**1. Merge-base computation can fail.** `git merge-base A B` walks ancestors of both commits
looking for the best common one. If the true merge base is older than the shallow floor, the
walk hits parentless boundary commits on both sides and finds no intersection: the command
exits non-zero with no output. This is not an error in any meaningful sense; it is the honest
answer "not within my horizon". Quinjet's `try_merge_base` (`src/git/github/mod.rs:1967`)
encodes that reading: a non-zero exit maps to `Ok(None)`, meaning "deepen further", never to a
failure.

**2. Anything that counts history undercounts.** `git log`, ahead/behind counts, and generation
numbers all stop at the floor. Quinjet never asks the disposable workspace for history, so this
costs nothing here; the history pane reads the opened repository, which Quinjet never
shallows.

**3. Tag following would punch through the floor.** Following tags drags in whatever history the
tags point to, which on an old repository means old commits and their trees. Every Quinjet fetch
passes `--no-tags` so the boundary stays where `--depth` put it.

### The shallow state Quinjet creates

After the common-case choreography (head fetched at depth 64, merge base point-fetched at depth
1), the disposable workspace's `shallow` file contains the boundary commits of two disjoint
history fragments: the commit 64 levels below the pull-request head, and the merge base itself
(depth 1 means the fetched commit is its own boundary). The two fragments do not need to
connect. The subsequent `git diff <merge_base> <head>` never walks history; it compares the two
root trees directly, so a disconnected pair of shallow islands is exactly enough.

## Deepening a shallow history

When the shallow floor turns out to be too high, it can be lowered without starting over. Git
offers four dials on `git fetch` (documented in
[git-fetch](https://git-scm.com/docs/git-fetch)):

| Flag | Wire argument | Semantics |
| --- | --- | --- |
| `--depth=<n>` | `deepen n` | Absolute: n commits measured down from the remote tips |
| `--deepen=<n>` | `deepen n` + `deepen-relative` | Relative: n commits below the current boundary |
| `--shallow-since=<date>` | `deepen-since` | Everything newer than a committer date |
| `--shallow-exclude=<rev>` | `deepen-not` | Everything not reachable from the named rev |
| `--unshallow` | `deepen 2147483647` | Effectively infinite depth; converts to a full history |

`--unshallow` is literally a maximal absolute depth: Git sends `INFINITE_DEPTH`, defined as
`0x7fffffff`, and the server's `unshallow` responses erase the boundary entries one by one until
the shallow file disappears.

### Why Quinjet uses absolute depths in a ladder

Quinjet's fallback for finding a merge base is a deepening ladder, and it restates absolute
`--depth` values rather than accumulating `--deepen` increments. The loop, verbatim from
`src/git/github/mod.rs:1848`:

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

Design points worth unpacking:

**1. Both sides deepen together.** A merge base must be reachable from the base tip and from
the head tip within the boundary. Deepening only one side can never terminate for a branch that
diverged long ago, so each rung past the first re-fetches the base refspec from `origin` and the
head refspec from whichever remote won the head fetch.

**2. Absolute depths are idempotent.** Each rung states the desired end state ("history to depth
1,024 from the tips") rather than a delta from wherever the last rung left off. A rung that is
retried, or that partially overlaps history already present, converges to the same boundary.
The server computes the difference; commits already transferred are not sent twice, because the
client's `shallow` lines tell the server where its floor currently sits.

**3. Geometric growth bounds the waste.** The rungs grow by a factor of four. If the merge base
is found at depth D, the total history requested across all rungs is at most
64 + 256 + ... + D, which is less than 4/3 of D. Compare a linear ladder (64, 128, 192, ...),
where reaching a deep merge base costs a number of rounds, and a total transfer, proportional to
D squared over the step size. Four times fewer round trips than doubling, at a worst-case
overshoot of 4x on the final rung, is the deliberate trade.

**4. The ceiling is a policy, not a limit of the mechanism.** The final rung is 16,384 commits;
past it the load fails with the exact message in the excerpt. The pre-stack code capped the
ladder at 4,096 and the session's failure-mode analysis ranked that as a hard failure worth
fixing: "Long-lived rewrite branches on active repos routinely exceed 4,096 commits of
divergence." PR #47 both extended the ceiling to 16,384 and, more importantly, made the ladder a
fallback rather than the primary mechanism, by resolving the merge base through the GitHub
compare API first. The doc comment on `merge_base_from_api` (`src/git/github/mod.rs:1285`)
states the relationship: "One metadata request replaces the deepening fetch ladder, which cannot
reach a merge base thousands of commits behind either tip." The ladder now runs only when the
API hint is unavailable or fails verification; see
[merge bases and history](./merge-bases-and-history.md) for the merge-base side of the story.

## Partial clone filters

Shallowness cuts the time axis; partial clone cuts the object-class axis. A filtered fetch asks
the server to omit whole categories of objects from the pack and to let the client fetch them
later, individually, if and when something actually reads them. The feature is specified across
[git-rev-list](https://git-scm.com/docs/git-rev-list) (the `--filter` grammar),
[git-clone](https://git-scm.com/docs/git-clone), and the partial clone design document shipped
with Git.

### The filter-spec grammar

| Filter spec | Omits |
| --- | --- |
| `blob:none` | Every blob |
| `blob:limit=<n>[kmg]` | Every blob larger than n bytes (or KiB/MiB/GiB with a unit suffix) |
| `tree:<depth>` | Every tree and blob deeper than `<depth>` below the root; `tree:0` omits all |
| `object:type=<type>` | Every object not of the named type |
| `sparse:oid=<blob-ish>` | Blobs not matched by the sparse-checkout spec stored in that blob |
| `combine:<f1>+<f2>` | The union of several filters |

Two rows matter in practice for diff-oriented tooling:

- `blob:none` transfers all commits and all trees in range, and no blobs. The shape of history
  and the shape of every directory arrive; file contents do not.
- `tree:0` transfers commits only. It is smaller still, but every tree read afterwards becomes a
  lazy fetch, and tree reads are the one thing a diff cannot avoid.

### What blob:none transfers and defers

Consider `git diff --name-status M H` inside a `blob:none` workspace:

1. Loading `M` and `H`: local, both commits were fetched.
2. Walking both root trees recursively to find differing entries: local, all trees came with
   the filtered pack. Comparing two tree entries means comparing name, mode, and object id, and
   the ids are right there in the tree objects; no blob content is touched.
3. Producing the status letter per path: still local for added, deleted, and modified paths.
   Exact rename detection is also content-free (an exact rename is the same blob id under a new
   name). Inexact rename detection, however, scores content similarity between candidate pairs,
   and content means blobs: with `--find-renames` on a tree with many unpaired adds and deletes,
   the similarity pass is the first place a name-status walk can trigger lazy blob fetches.
4. Producing an actual patch (`git diff --patch`): blobs of every changed path, necessarily.

This ordering is the economic foundation of Quinjet's whole pull-request pipeline: the changed
file index (step 1 to 3) is nearly free under `blob:none`, while patch text (step 4) costs one
lazy blob pair per file, which is why patches are batched, cached, and prefetched with budgets
(see [diff pipeline](../diff/pipeline.md) and [prefetch](../github/prefetch.md)).

### Server-side consent

Filters are opt-in for servers. `uploadpack.allowFilter` enables the `filter` capability, and
finer-grained `uploadpackfilter.*` settings can allow or ban individual specs. When the server
does not support filtering, the client-side outcome depends on the protocol path: a v2 client
sees no `filter` feature in the `fetch` capability and dies with an error along the lines of
`Server does not support --filter`, while some older v0 paths warn
`filtering not recognized by server, ignoring` and proceed unfiltered. Either behavior is
acceptable to a caller that treats the filter as an optimization, and that is exactly the
posture Quinjet takes with its retry, shown next.

### The exact Quinjet fetch command

Every network fetch in the pull-request choreography goes through one function, `fetch_ref` in
`src/git/github/mod.rs:1876`, quoted here in full because each argument earns its place:

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

Rendered as a command line, the primary attempt is:

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=N <remote> <refspec>
```

and the retry is the identical command without `--filter=blob:none`. Flag by flag:

- `--quiet` suppresses progress output; combined with the 128 KiB stdout cap this keeps the
  subprocess pipe traffic near zero (fetch writes its real payload into the object store, not
  stdout).
- `--force` permits non-fast-forward updates of the destination ref. The refspecs are all
  `+`-prefixed anyway; the flag makes the intent explicit and immune to refspec editing.
- `--no-tags` disables tag following, keeping the shallow boundary tight as described above.
- `--filter=blob:none` is the partial-clone request: commits and trees now, blobs on demand.
- `--depth=N` is the shallow request; N is 64 for initial ref fetches, 1 for the merge-base
  point fetch, and a ladder rung value during deepening.

The retry exists because the filter is the one argument a server may refuse. Shallowness is
universally supported; filtering is not. A fetch that fails with the filter is retried once
without it, trading transfer size for compatibility, and only a second failure surfaces as an
error. The workspace then simply holds blobs it did not strictly need, which costs disk in a
directory that is deleted on drop, and nothing else.

## Promisor remotes and lazy fetch

A filter creates a debt: the repository now references objects it does not contain. Promisor
remotes are the bookkeeping that makes the debt safe, and lazy fetch is how it is paid down.

### The promise and its bookkeeping

After a filtered fetch, Git records two config keys on the remote that served it:

```text
remote.origin.promisor = true
remote.origin.partialclonefilter = blob:none
```

The first is the promise: this remote has committed to serving, on demand, any object reachable
from what it already sent. The second records which filter created the gaps, so later fetches
from the same remote reuse it. Packs downloaded from a promisor remote are marked on disk with a
`.promisor` file beside the `.pack`, and any object referenced by an object inside a promisor
pack is called a promisor object: it may legitimately be absent.

That "may legitimately be absent" is the crucial semantic shift. In a normal repository, a
referenced-but-missing object means corruption, and every command treats it as fatal. In a
partial clone, consistency checks (`git fsck`, connectivity checks after fetch, `git gc`
reachability walks) exempt promisor objects, and `git rev-list` grows flags like
`--exclude-promisor-objects` and `--missing=allow-promisor` so scripted walks can opt into the
same tolerance.

The benchmark clone's config shows the exact shape this leaves behind. The recorded values for
/tmp/bun-test:

```text
core.repositoryformatversion=1
remote.origin.url=https://github.com/oven-sh/bun
remote.origin.fetch=+refs/heads/main:refs/remotes/origin/main
remote.origin.promisor=true
remote.origin.partialclonefilter=blob:none
```

`core.repositoryformatversion=1` is the repository-format bump that permits extensions such as
partial clone; the promisor pair is the debt record described above; and the single-branch
refspec keeps even the ref namespace narrow.

### How a lazy fetch happens

When any code path finally needs a missing object, the read goes through the ordinary object
lookup chain: loose objects, then packs, then alternates (see
[object model](./object-model.md) for the full path). Only when every local source misses does
the promisor machinery engage:

1. Git spawns an internal fetch from the promisor remote naming the missing object ids
   directly, as wants.
2. Negotiation is skipped: the internal fetch uses the no-op negotiator, because the goal is not
   "synchronize histories" but "deliver these exact objects".
3. The fetched objects land in a new promisor pack, and the original read resumes as if the
   object had always been there.

The transparency is the feature and the danger at once. Any command, however innocent looking,
can stall on a network round trip, and a command that touches many missing objects one at a time
turns into many round trips. Modern Git batches the known-in-advance cases (a checkout prefetches
all blobs it is about to write; diff and pack-objects prefetch in batches), but a long chain of
individually-triggered reads still degrades into serial fetches. Managing exactly when and in
what batch sizes blobs get materialized is therefore a first-class design concern for any tool
built on partial clones, and Quinjet's answer occupies the next two subsections.

### The numstat blob storm

The single largest cold-load win in the optimization stack came from noticing a lazy-fetch storm
hiding inside an innocuous command. The original pipeline, after enumerating changed paths, ran
`git diff --numstat` over the merge-base/head range to get per-file added and deleted line
counts for the file headers. Line counts require file contents; file contents in a `blob:none`
workspace are all missing; so this one subprocess forced the lazy download of, in the words of
the session's failure-mode analysis, "essentially every changed blob in one uninterruptible git
invocation while the UI sits at 'Enumerating changed files'". On a 2,188-file pull request that
is thousands of blobs paid for up front, merely to print numbers next to file names.

PR #49 removed the storm by asking GitHub instead. The pull-request files endpoint already knows
every file's additions and deletions, and reading it costs paged metadata requests instead of
blob transfers. The doc comment on `pull_request_file_counts_from_api`
(`src/git/github/mod.rs:1235`) compresses the rationale: "In the blob-less disposable workspace
a local `--numstat` would download every changed blob just to count lines; GitHub already knows
the totals."

The division of labor in the merged code is explicit at the call site in
`changed_files_in_repository`: counts come from `api_counts` when the workspace is disposable,
and only the network-free opened-repository path (where every blob is already local and numstat
is cheap) still runs `git diff --numstat`. The API strategy, its 64-page cap, and the records it
deliberately drops are covered in [API strategy](../github/api-strategy.md); what matters on
this page is the shape of the mistake it fixed: a metadata question accidentally phrased as a
content question, in a repository engineered to not have content.

### Borrowed objects: alternates before the network

The second lazy-fetch optimization, from PR #55, attacks the paydown itself. A pull request the
user cares about frequently overlaps history the user already has: a squash-merged PR's file
contents exist in the local clone's main branch even though the PR head commit does not. The
disposable workspace can be given read access to all of it with a one-line file, and
`borrow_local_objects` in `src/git/github/mod.rs:1732` does exactly that:

```rust
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

The file it writes, `objects/info/alternates`, is the standard Git mechanism for sharing an
object store: each line names another objects directory, and object lookup consults those
directories after the repository's own loose and packed objects, before concluding an object is
missing. Because the promisor lazy fetch only fires after a miss, every blob the opened
repository already holds is now served from local disk at local speed, and the network pays
only for genuinely novel content. The function is deliberately best-effort (every failure path
is a silent return): a workspace without the borrow still works, just slower.

Note the direction of the arrow. The workspace reads the opened repository's objects; nothing
is ever written back. The invariant that the opened repository is never mutated survives
because an alternates file lives in the borrowing repository, not the lending one.

## Fetching arbitrary refs and exact commits

The choreography needs three things by name: a pull request's head, a base branch, and one
specific merge-base commit. None of them is a ref a normal clone would have, which makes the
refspec and exact-oid machinery load-bearing.

### Refspecs and forced updates

A refspec `+<src>:<dst>` maps a source name on the remote to a destination name locally. The
leading `+` permits non-fast-forward updates, which matters here because the workspace refs are
scratch names that get overwritten freely. Quinjet builds exactly two refspec templates, at
`src/git/github/mod.rs:1800`:

```rust
let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);
```

Everything fetched lands under `refs/quinjet/*`, a namespace no other tool uses, so even inside
its own disposable repository Quinjet keeps fetched state clearly labeled and collision-free.

### GitHub's synthetic pull-request refs

GitHub materializes every pull request as synthetic refs on the base repository:
`refs/pull/<n>/head` is the pull request's current head commit, and `refs/pull/<n>/merge`,
when present, is a trial merge of the head into the base. These refs are advertised to fetch
(they are exactly why the `ref-prefix refs/pull/30412/head` example in the protocol section
returns a result) but are not matched by the default fetch refspec, so clones do not
accumulate them.

Fetching `refs/pull/<n>/head` from the base repository is strictly better than fetching the
contributor's branch from their fork: it exists even when the fork branch was renamed, it
requires no second remote in the common case, and it needs no knowledge of the fork's URL. The
fork path exists in Quinjet only as a fallback, for pull requests whose synthetic ref is not
served; the code adds a second remote named `head` pointing at a URL derived from the base
repository's scheme and host plus the fork's `owner/name`, and fetches
`+refs/heads/<head_ref>:refs/quinjet/head` from it. When even that is impossible because the
fork was deleted, the error is contextualized precisely: "the base repository no longer exposes
the PR head and its fork was deleted" (`src/git/github/mod.rs:1807`).

### Exact object-id wants

The merge-base point fetch does not name a ref at all; it names a commit:

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

(`src/git/github/mod.rs:1834`.) The refspec source is a raw 40-character object id obtained from
the GitHub compare API, and the fetch requests it at `--depth=1`: one commit, its trees (the
filter still applies), no parents, no blobs. Under `blob:none` this is close to the cheapest
possible way to materialize a diff endpoint.

Fetching by raw object id needs server cooperation. Historically, upload-pack refused wants
that were not advertised tips; the config knobs `uploadpack.allowTipSHA1InWant`,
`uploadpack.allowReachableSHA1InWant`, and `uploadpack.allowAnySHA1InWant` relaxed that in
increasing order of permissiveness, and protocol v2's `fetch` specification loosened the
default posture by defining wants as not limited to advertised objects. GitHub's servers accept
wants for reachable commits, which is the property this fetch relies on, and the merge base is
reachable by construction: the compare API derived it as an ancestor of the base branch. The
same server behavior is what allows CI systems to fetch an exact commit at depth 1 rather than
cloning, so it is well-trodden ground rather than an exotic dependency.

The guard after the fetch is as important as the fetch. The hint was computed from the
metadata's `base_oid` and `head_oid` snapshot; if the branch was force-pushed between the
metadata read and the fetch, the workspace's freshly fetched head no longer equals the snapshot
head, and pairing the old merge base with the new head would produce a wrong diff and, worse,
cache it immutably under the wrong key pair. The adversarial review of the stack caught exactly
that race, and the fix is the `head == pull_request.head_oid` comparison: the shortcut is taken
only when the world has not moved. Otherwise control falls through to the ladder, which
re-anchors both sides.

### Pinning against ref movement

The same snapshot-pinning idea appears once more in `preferred_fetched_commit`
(`src/git/github/mod.rs:1949`): whenever the metadata carried a full object id, the workspace
resolves that exact id (`git rev-parse --verify <oid>^{commit}`) in preference to whatever the
just-fetched `refs/quinjet/*` ref now points at. A branch that moves mid-load therefore cannot
skew the diff; the view describes the commits the metadata described, or fails honestly.

## Bare repositories as fetch targets

The workspace receiving all these fetches is a bare repository, and both words are doing work.

### Why bare

A bare repository is a Git directory without a working tree: objects, refs, config, and nothing
checked out. For a fetch-and-diff workload this removes entire cost classes:

- No checkout ever happens, so the megabytes (or for bun, hundreds of megabytes) of tree
  content never get written to disk as files, only as compressed objects.
- There is no index, so nothing maintains one; `git diff <commit> <commit>` compares trees
  directly from the object store.
- There is no working tree for any tool to accidentally touch, which pairs with the
  isolation argument: even a badly behaved subprocess cannot leave stray files in a checkout
  that does not exist.

### Naming, placement, and collision handling

`TemporaryBareRepository::new` (`src/git/github/mod.rs:1690`) creates the workspace with
`git init --bare --quiet` under a `tmp` directory inside the Quinjet cache root, falling back
to the system temp directory when the cache root cannot be created. The directory name encodes
ownership and uniqueness:

```text
<cache_root>/tmp/pr-<pid>-<id>.git
```

where `<pid>` is the creating process id and `<id>` comes from a process-wide `AtomicU64`
counter (`TEMPORARY_REPOSITORY_ID`), so concurrent workspaces in one process and concurrent
Quinjet processes on one machine cannot collide. The constructor tries up to 16 candidate
names, skipping any path that already exists, and only then gives up with "unable to allocate a
unique disposable Git repository". The parent directory is created with mode 0700
(`create_private_directory`), consistent with the cache privacy rules in
[caching](../github/caching.md): patches are repository content at rest, so everything under
the cache root stays owner-only.

### Lifetime: drop plus a reaper

Deletion is layered, because processes crash. The primary mechanism is `Drop`
(`src/git/github/mod.rs:1748`):

```rust
impl Drop for TemporaryBareRepository {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}
```

Closing the pull-request view drops the `PreparedPullRequest`, which drops the
`TemporaryBareRepository`, which removes the entire bare directory: workspace lifetime equals
view lifetime, byte for byte. The backstop for crashed or killed processes is
`remove_stale_temporary_repositories` (`src/git/github/mod.rs:1754`), which runs on every
workspace creation, before the new directory is allocated:

```rust
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
```

The reaper scans at most 256 directory entries, matches only the `pr-*.git` naming pattern it
itself creates, and deletes directories whose modification time is at least
`TEMPORARY_REPOSITORY_MAX_AGE` old, defined as 24 hours (`src/git/github/mod.rs:50`). The
threshold is deliberately generous: a workspace stays alive as long as its pull request is open
on screen, and a day comfortably exceeds any legitimate session while still guaranteeing that a
crash cannot leak disk forever. The 256-entry cap keeps the sweep constant-time even if the tmp
directory somehow accumulates unrelated clutter.

Two tests in the same file pin the contract: `temporary_bare_repository_is_removed_on_drop`
(`src/git/github/mod.rs:3080`) for the happy path, and the larger
`disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source`
(`src/git/github/mod.rs:2989`) which additionally asserts, byte for byte, that the source
repository's branches, status, and refs are untouched by a full prepare-and-diff cycle through
the workspace.

### The user-visible layout

For operational purposes the layout under the cache root is worth knowing:

- `~/.cache/quinjet/github/` holds the content-addressed cache entries (opaque hashed
  filenames; see [caching](../github/caching.md)).
- `~/.cache/quinjet/tmp/` holds the disposable `pr-*.git` bare workspaces, removed on drop and
  swept after 24 hours.
- `rm -rf ~/.cache/quinjet` is always safe; everything under it re-fetches on demand, and
  `QUINJET_CACHE_DIR=/some/dir` relocates the whole tree for an isolated run.

## The Quinjet fetch choreography

Everything above now assembles into one sequence. This section walks the disposable path of
`prepare_pull_request_diff` end to end, in execution order, with the exact subprocesses each
step spawns.

### Progress vocabulary

The caller observes the sequence through `PullRequestProgress`
(`src/git/github/mod.rs:237`), a fixed set of stages with fixed percentages, which is why the
loading bar over a huge pull request moves in honest, named steps rather than a fake spinner:

| Stage | Percent | Label |
| --- | --- | --- |
| `LoadingMetadata` | 10 | Fetching pull-request metadata |
| `PreparingRepository` | 20 | Preparing an isolated diff workspace |
| `FetchingBase` | 35 | Fetching the destination commit |
| `FetchingHead` | 50 | Fetching the source commit |
| `FindingMergeBase` | 65 | Finding the merge base |
| `EnumeratingFiles` | 90 | Enumerating changed files |

### Step by step

**Step 0: metadata.** Before any Git process runs, the pull request's identity arrives through
`gh pr view` as an 18-field TSV record: number, refs, both object ids (`baseRefOid`,
`headRefOid`), totals, and repository identity. Everything after this step treats those two
object ids as the snapshot being rendered. The metadata layer, its 5-minute TTL, and its
stale-on-error fallback belong to [API strategy](../github/api-strategy.md).

**Step 1: the local probe.** `has_commit` runs `git cat-file -e <oid>^{commit}` twice against
the opened repository, once per endpoint. `-e` is an existence probe producing no stdout at
all; two subprocess round trips decide the entire network question. Both present: the diff is
served from the opened repository and the remaining steps never execute. The test
`locally_available_pr_objects_avoid_disposable_fetches` (`src/git/github/mod.rs:2946`) proves
the negative space by pointing the base repository URL at an unreachable host and asserting the
whole prepare-and-diff cycle still completes in under 2 seconds: no network I/O can be hiding
in that path.

**Step 2: API hints.** Two GitHub reads happen before any fetch, both cached immutably under
keys containing the object-id pair:

- `merge_base_from_api` asks the compare endpoint for the merge base:

```bash
gh api repos/{owner}/{name}/compare/{base_oid}...{head_oid} --jq .merge_base_commit.sha
```

  The answer is validated as a commit oid and cached under
  `pr-merge-base-v1\n{repo url}\n{base}\n{head}`. Any failure returns `None` and simply means
  the ladder will run; the hint is never load-bearing for correctness.

- `pull_request_file_counts_from_api` pages through `pulls/{number}/files?per_page=100` for
  per-file additions and deletions, the numstat replacement described earlier.

**Step 3: workspace creation.** `TemporaryBareRepository::new` initializes the bare directory,
`borrow_local_objects` writes the alternates line, and `git remote add origin <base repo URL>`
configures the one remote every fetch will use. All Git subprocesses in the workspace run
through `run_repository_git` (`src/git/github/mod.rs:2192`), which applies the same hardening
as every other Quinjet Git call: `git -C <workspace> -c core.quotepath=false ...` with
`LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, and `GIT_TERMINAL_PROMPT=0` in the environment, so output
is unlocalized, no lock files are taken opportunistically, and an authentication problem fails
fast instead of freezing a worker thread on a hidden prompt.

**Step 4: the head fetch.** Progress moves to `FetchingHead` and the workspace fetches
GitHub's synthetic ref at depth 64:

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=64 \
  origin "+refs/pull/<n>/head:refs/quinjet/head"
```

On failure, the fork fallback path runs (second remote, branch refspec, same depth), and only
the deleted-fork case is terminal. Whichever pair of remote and refspec succeeded is remembered
for the ladder, so deepening later re-fetches the head from the same place.

The choice of 64 rather than 1 for the head is deliberate slack: the head-side history is what
the ladder deepens if the merge base has to be found locally, and 64 commits of head history
also cover the common case where the metadata's `head_oid` is a commit or two behind the
current ref tip after a routine push, letting `preferred_fetched_commit` still resolve the
snapshot id inside the fetched slice.

**Step 5: the merge-base shortcut.** Progress moves to `FindingMergeBase`. With a hint in hand,
the workspace point-fetches it:

```bash
git fetch --quiet --force --no-tags --filter=blob:none --depth=1 \
  origin "+<merge_base_oid>:refs/quinjet/merge-base"
```

If the fetch succeeds and the fetched head still equals the metadata's `head_oid`, the function
returns immediately. In this common case the complete history transfer for an arbitrarily large
pull request against an arbitrarily old base is: 64 commits of head history, one merge-base
commit, and the trees of both, with zero blobs. The base branch's history is never fetched at
all.

**Step 6: the ladder.** Only without a usable hint does progress reach `FetchingBase`: the base
ref is fetched at depth 64 and the `[64, 256, 1_024, 4_096, 16_384]` loop from the deepening
section runs `git merge-base` in the workspace after each rung until it answers or the ceiling
bails.

**Step 7: enumeration.** Progress reaches `EnumeratingFiles` and the workspace runs the
name-status walk:

```bash
git diff --name-status -z --find-renames <merge_base> <head> --
```

NUL-separated records, capped at 8 MiB of output and 16,384 parsed entries, cached immutably
under `pr-files-v1\n<merge_base>\n<head>`. As established in the filter section, this is a
tree-level operation under `blob:none`; its parsing, its truncation repair, and its caps belong
to [plumbing and porcelain](./plumbing-and-porcelain.md) and
[pr-workspace](../github/pr-workspace.md).

**Step 8 and onward: patches, on demand and prefetched.** The `PreparedPullRequest` handle now
lives as long as the view. Selecting a file runs a path-scoped patch command in the workspace:

```bash
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 <merge_base> <head> -- <path>
```

and background prefetch batches up to 32 paths into single invocations of the same command
shape. Each such command is where blobs actually materialize: the patch needs contents, the
contents are missing, the promisor machinery fetches them, and the alternates link short-circuits
the ones the local clone already has. Every fetched patch is split, parsed, and cached under
its own immutable `pr-patch-v1` key, so the blob download for any given file happens at most
once per merge-base/head pair, ever.

### Command inventory for one cold load

The complete subprocess sequence for the common disposable-path case, in order:

| # | Command | Purpose |
| --- | --- | --- |
| 1 | `gh pr view <n> --repo <url> --json ... --jq ...` | Metadata snapshot with both oids |
| 2 | `git cat-file -e <base_oid>^{commit}` | Local presence probe, base |
| 3 | `git cat-file -e <head_oid>^{commit}` | Local presence probe, head |
| 4 | `gh api repos/<o>/<r>/compare/<base>...<head> --jq .merge_base_commit.sha` | Merge-base hint |
| 5 | `gh api repos/<o>/<r>/pulls/<n>/files?per_page=100&page=N --jq ...` | Per-file counts, paged |
| 6 | `git init --bare --quiet <tmp>/pr-<pid>-<id>.git` | Workspace creation |
| 7 | `git remote add origin <base repo url>` | Remote wiring |
| 8 | `git fetch ... --filter=blob:none --depth=64 origin +refs/pull/<n>/head:refs/quinjet/head` | Head |
| 9 | `git fetch ... --filter=blob:none --depth=1 origin +<mb_oid>:refs/quinjet/merge-base` | Merge base |
| 10 | `git rev-parse --verify <head_oid>^{commit}` | Snapshot pinning |
| 11 | `git diff --name-status -z --find-renames <mb> <head> --` | Changed-file index |

Eleven bounded subprocesses, two of which touch the GitHub API and two of which move history,
and the history movers transfer commits and trees only. Everything else a million-line pull
request will ever need arrives later, lazily, in budgeted batches, while the reader is already
reading.

## A worked trace on the benchmark pull request

Numbers make the shape concrete. The stack's benchmark target was oven-sh/bun#30412, "Rewrite
Bun in Rust": 2,188 changed files, +1,009,257 added lines, against one of the most active
repositories on GitHub. This section traces what the choreography does for it and what the
session measured.

### The transfer ledger

Following the steps above for this pull request:

- Steps 2 and 5 resolve the merge base through the compare API: one metadata request against a
  history where the base branch had moved on by a large number of commits since the PR
  branched. This is precisely the "merge base thousands of commits behind either tip" scenario
  the `merge_base_from_api` doc comment names; a ladder-only design would have burned several
  progressively larger fetches to find it, or failed.
- Step 4 transfers 64 commits of head-side history plus trees. For a rewrite-scale change the
  trees are the bulk of this fetch, and they are transferred compressed and delta-encoded in
  the pack (see [packfiles and deltas](./packfiles-and-deltas.md)).
- Step 9 transfers exactly one commit plus its trees.
- Step 11 enumerates all 2,188 changed paths without one blob crossing the network, and the
  per-file counts, already fetched from the API in step 5, let every file header render its
  real `+n -n` immediately.
- Blob transfer begins only when patches are read, and lands in batches of at most 32 files
  sized under a 6 MiB estimate budget, anchored to whatever the reader is looking at.

### What the session measured

All figures below are quoted from the session records, with their context, and each was
measured against the /tmp/bun-test clone described in the final section.

First verification round, cold cache, top of the original five-PR stack:

- "Metadata in 1.7s" (`pr view` against bun#30412, cold).
- "The rewrite PR enumerates all 2,188 files with real counts in 18.5s cold." (`pr files`, cold
  cache, includes workspace prepare.)
- Warm re-run of the index: 0.04s.
- Single-file patches: 0.1s.

Second verification round, after the adversarial-review fixes and the restack, final binary:

- "Final numbers on the bun PR: cold index 6.3s, warm 0.04s, conversation 26s with the honest
  truncation notice."
- Summary quote from the session: "2,188-file/1M-line index in 6.3s cold, 0.04s warm, per-file
  patches instant, conversation newest-first in 26s."

The cold-index improvement from 18.5s to 6.3s arrived with the review-fix round, which among
other things rebased the stack and included the counts-cache key fix. After the final build was
installed locally, a warm-metadata smoke test recorded: "`q pr files 30412` lists all 2,188
files of the 1M-line rewrite PR in 1.4s."

The warm numbers deserve a note: 0.04s for the full index is possible because every artifact of
steps 4 through 11 is keyed by the immutable merge-base/head object-id pair and cached on disk;
a warm load replays cached bytes through the same parsers and spawns no fetch at all. The cache
design that makes "warm" equal "no network, no Git" is the subject of
[caching](../github/caching.md).

### The squash-merge trap, live

The most instructive real-world moment in the session came after the stack was built, when the
pull request was driven from a full local clone of bun rather than the benchmark clone, and per
file loads still crawled. The user's question, quoted in the session notes: "Everything is
local. Why is it taking so much time to load this for each of the files here?"

The diagnosis is a perfect illustration of the decision tree's edge:

- bun squash-merged the rewrite pull request. A squash merge writes a brand-new commit on main
  containing the same tree changes; the PR's actual head commit exists only behind GitHub's
  `refs/pull/30412/head`, never in any branch a normal clone fetches.
- The local clone was full, so every blob the diff needed was on disk. But `has_commit` asks
  about commits, and the head commit was absent, so the network-free branch of the decision
  tree could not be taken, and Quinjet correctly refused to fetch the missing ref into the
  user's clone (invariant 9 again). It fell back to the disposable `blob:none` workspace, where
  at the time (before #55's alternates borrow) every expanded file was a lazy blob download.
- The one-time manual remedy was to give the local clone the synthetic ref:

```bash
git fetch origin +refs/pull/30412/head:refs/remotes/origin/pr-30412
```

  After that fetch, both endpoints exist locally, the network-free path applies, the merge base
  is computed locally, and every patch is a local `git diff`.

The permanent remedy became PR #55's `borrow_local_objects`: even when the head commit is
missing and the disposable path must run, the workspace now reads the clone's object store
through alternates, so the blobs a squash merge already delivered locally never cross the
network again. The session verified the borrow "end to end on another merged bun PR whose head
commit is absent from your clone".

### The wire conversation for the head fetch

To close the loop with the protocol sections, here is what step 4 of the choreography looks
like as pkt-lines over smart HTTP. The transcript is representative rather than captured:
object ids are abbreviated to `<oid>`, each standing for 40 hexadecimal characters, and the
frame lengths shown assume that width. First, ref resolution:

```text
client POST /oven-sh/bun/git-upload-pack   (Git-Protocol: version=2)

0014command=ls-refs\n
0015agent=git/2.43.0\n
0017object-format=sha1\n
0001
0009peel\n
000csymrefs\n
0024ref-prefix refs/pull/30412/head\n
0000

server response

0042<oid> refs/pull/30412/head\n
0000
```

One ref requested, one line answered. The client now knows the head object id as the server
sees it and issues the fetch command:

```text
client POST /oven-sh/bun/git-upload-pack

0012command=fetch\n
0015agent=git/2.43.0\n
0017object-format=sha1\n
0001
000ethin-pack\n
0010no-progress\n
000eofs-delta\n
000edeepen 64\n
0015filter blob:none\n
0032want <oid>\n
0009done\n
0000
```

The request carries no `have` lines (the workspace is empty) and ends with an immediate
`done`, so there is nothing to acknowledge and the server proceeds straight to the boundary
report and the pack:

```text
server response

0011shallow-info\n
0035shallow <oid>\n          (one line per new boundary commit)
0001
000dpackfile\n
<length>\x01<pack bytes>     (sideband code 1 frames, repeated)
0000
```

The client records the `shallow` lines into the workspace's shallow file, indexes the pack, and
updates `refs/quinjet/head`. Every other fetch in the choreography, the depth-1 merge-base
point fetch included, is this same conversation with different `want`, `deepen`, and
`ref-prefix` values.

## Design alternatives and why they lost

Each mechanism in the shipped design displaced at least one plausible alternative. Recording
why the alternatives lost is half the value of the design.

**1. Fetching into the opened repository lost to isolation.** The obvious cheap design is
`git fetch origin refs/pull/N/head` in the user's clone: one fetch, full reuse of local
objects, no workspace to manage. It loses on the contract. ARCHITECTURE.md invariant 9
guarantees the opened repository receives no ref mutation, and a fetch writes refs, objects,
and possibly shallow state. It would also race with whatever the user's own Git commands are
doing in that repository at the same moment. The session had a live chance to break this rule
and did not: when a full local bun clone was slow because a squash-merged head commit was
missing, the answer was to tell the user the one-time `git fetch` command to run themselves,
and then to build the alternates borrow, which imports the benefit of local objects into the
workspace without writing a byte into the clone.

**2. A full clone of the base repository lost to arithmetic.** Even the heavily filtered,
shallow, single-branch benchmark clone of bun weighs 389 MB on disk. An unfiltered clone of a
repository that size is a multi-gigabyte, multi-minute proposition, paid before the first file
header can render, for a view that needs two commits and a few thousand blobs. Nothing about a
pull-request view amortizes that cost: the next pull request needs a different pair of commits,
and the 128 MiB disk cache would not even keep one such clone.

**3. `tree:0` lost to the shape of diff.** If `blob:none` is good, omitting trees as well looks
better: the head fetch would shrink to little more than 64 commit objects. But the very first
operation the workspace runs afterwards is `git diff --name-status`, which walks both root
trees recursively. Under `tree:0` every tree along that walk is a missing object, so the walk
becomes a cascade of lazy fetches, roughly one round trip per directory level per subtree, on
the critical path of the first paint. `blob:none` is the precise cut line between "objects the
index needs" (commits, trees: fetched eagerly, cheap) and "objects only patches need" (blobs:
fetched lazily, budgeted, cacheable). The filter grammar offers finer knives; the diff workload
only has one natural joint, and `blob:none` is it.

**4. `blob:limit` lost to redundancy.** A `blob:limit=64k` style filter would eagerly transfer
all small blobs, betting that most patches touch small files. The bet is poor for this
workload: the workspace reads blobs only for changed paths (a few thousand at most, out of
hundreds of thousands in a tree), so eagerly transferring every small blob in the repository
moves orders of magnitude more objects than the diff can ever read. The batching layer already
fetches exactly the blobs that patches touch, at most once each, with the byte budget applied
where it belongs: per read batch, not per repository.

**5. The ladder alone lost to history depth.** The pre-stack design had no API hint; the ladder
was the only merge-base mechanism and it capped at 4,096 with a hard failure the review round
ranked as a top-three defect ("Long-lived rewrite branches on active repos routinely exceed
4,096 commits of divergence"). Resolving the merge base through the compare API turned the
common case into one metadata request plus one depth-1 fetch, with two properties the ladder
cannot match: constant cost regardless of divergence, and no transferred-then-discarded
history. The ladder survives as the fallback, extended to 16,384, because the API can be
unavailable, rate-limited, or wrong about a force-pushed head, and a fallback that works
against any Git server keeps the feature independent of GitHub-specific endpoints.

**6. Local numstat lost to the filter it defeated.** Counting changed lines locally
(`git diff --numstat`) is exact and simple, and in a `blob:none` workspace it silently
re-downloads every changed blob, un-deferring precisely the cost the filter deferred. Reading
counts from the pulls files endpoint keeps the workspace blob-free through enumeration. The
accepted losses are documented and bounded: GitHub reports some huge generated files as 0/0
(rendered as `+·· -··` skeletons rather than fake zeros, then backfilled with real numbers
from the file's own patch once it arrives), the API cannot flag binaries so the binary label
waits for the patch, and a pure rename's honest 0/0 is kept, a distinction the review round
sharpened. The full endpoint mechanics live in [API strategy](../github/api-strategy.md).

**7. Smallest-first prefetch lost to the viewport.** The prefetch ordering policy evolved
inside the stack itself, and both stages are worth recording. PR #50 introduced size tiers: on
huge pull requests (at least 100,000 total changed lines or 1,000 files, the
`HUGE_PULL_REQUEST_LINES` and `HUGE_PULL_REQUEST_FILES` constants), prefetch candidates were
sorted by estimated patch size ascending, so the byte budget covered the largest possible
number of files early. That was the right call while background fill stopped at 400 files:
coverage was the scarce resource, and small files maximize it. PR #55 changed the constraint by
raising the prefetch cap to 4,096 files, enough to cover the whole benchmark index, and once
everything will be fetched eventually, order stops being about coverage and starts being about
latency to the reader. The smallest-first sort and both `HUGE_` constants were removed, and the
current ordering walks the index starting from the first file visible in the Files tree,
wrapping around, so patches land where the reader is looking. The batch shape in the merged
code, from `src/app.rs`: batches of at most `PULL_REQUEST_PREFETCH_BATCH = 32` files, filled
under a `PULL_REQUEST_PREFETCH_BYTE_BUDGET = 6 MiB` estimated-byte budget, against a
`MAX_PREFETCHED_PULL_REQUEST_FILES = 4_096` total cap. The estimate function
(`src/app.rs:7052`) is the bridge to the counts machinery above:

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

80 bytes per changed line (`PULL_REQUEST_PATCH_LINE_ESTIMATE`) plus a 4,096-byte floor per
file, and a 512 KiB fallback (`PULL_REQUEST_PATCH_FALLBACK_ESTIMATE`) for files whose counts
are unknown. The 6 MiB estimate budget deliberately undershoots the hard 8 MiB pipe cap on the
actual `git diff` output, leaving headroom for estimation error so a batch almost never
truncates. The anchor-and-wrap walk itself, and the mailbox slot that keeps a queued batch from
ever displacing a reader's own request, are covered in [prefetch](../github/prefetch.md) and
[progressive loading](../rendering/progressive-loading.md); the point here is that per-file
counts fetched over the API are what make byte-budgeted, blob-aware scheduling possible at all
in a workspace that has no blobs.

**8. An in-process Git library lost to process boundaries.** Linking libgit2 or gitoxide would
avoid process spawns and output parsing. Quinjet links neither; every repository operation is a
spawned `git` subprocess. For the machinery on this page the subprocess design is not a
compromise but the enabler: partial clone, promisor lazy fetch, shallow deepening, and protocol
v2 negotiation are exactly the features where library implementations trail the reference
implementation, and by shelling out, Quinjet inherits upstream Git's behavior, bug fixes, and
server compatibility wholesale. The process boundary is also the enforcement point for the
resource caps: `run_bounded_command` kills a child the moment it crosses its output cap, a
guarantee no in-process API offers as simply. See
[plumbing and porcelain](./plumbing-and-porcelain.md) for the full argument.

**9. A persistent workspace lost to the cache.** Keeping `pr-<pid>-<id>.git` across sessions
would make reopening a pull request cheaper, at the price of unbounded disk growth, staleness
management, and a second cache with different rules. The shipped design deletes the workspace
on drop and lets the content-addressed disk cache carry the reusable value instead: the file
index, numstat bytes, and every patch are cached under merge-base/head object-id keys that can
never go stale, so a reopened pull request re-fetches only the two small history slices and
replays everything else from disk. Invariant 14 makes the ownership explicit: the terminal pays
for a prepared pull request once per session; a subcommand pays the fetch again and relies on
the immutable per-file caches. The 24-hour reaper is the enforcement that nothing persists by
accident.

## Failure modes and edge cases

The choreography crosses a network, a third-party API, another team's server configuration,
and the user's filesystem. Every failure path is explicit in the code, and most were either
designed up front or hardened by the stack's adversarial review round.

**1. The server refuses the filter.** Covered structurally by the `fetch_ref` retry: the exact
command re-runs without `--filter=blob:none`, shallowness intact. The workspace then holds
blobs it did not need; nothing downstream can tell the difference, because the lazy-fetch
machinery simply never fires. Only a second failure surfaces, with the stderr-derived message
from `bounded_command_error`.

**2. The synthetic ref is gone and so is the fork.** The head fetch tries
`refs/pull/<n>/head` on the base repository first, then the fork's branch through a second
remote. A pull request whose fork was deleted and whose synthetic ref is no longer served is
genuinely unrenderable, and the error says so with its cause chain: "the base repository no
longer exposes the PR head and its fork was deleted".

**3. A force-push lands mid-load.** Two independent guards. `preferred_fetched_commit` pins
every resolution to the metadata's snapshot object ids when they parse as full oids, so a
moved ref cannot substitute a different commit silently. And the merge-base shortcut is taken
only when the fetched head equals the snapshot head; otherwise the ladder recomputes the merge
base against whatever was actually fetched. The review round demonstrated why the second guard
must exist: without it, a stale hint paired with a fresh head would have produced a wrong file
list and cached it immutably under the `pr-files-v1\n<mb>\n<head>` key, where nothing could
ever correct it, because immutable entries are by definition never revalidated.

**4. The merge base is deeper than 16,384 commits.** The ladder bails with "Unable to find the
PR merge base within 16,384 commits; refusing an unbounded history fetch". This ceiling is only
reachable when the compare API also failed, so hitting it in practice means GitHub is
unavailable and the branches diverged by more than 16,384 commits. Refusing is the correct
behavior for a tool whose invariants promise bounded resource use: an unbounded deepening loop
against a hostile or degenerate history is a disk-filling denial of service against the local
machine.

**5. GitHub's API is down, rate-limited, or slow.** Both hints are `Option`-shaped and both
degrade independently. A missing merge-base hint routes to the ladder; missing counts leave
file headers with `+·· -··` skeletons that backfill from each file's own patch as it arrives
(`backfill_pull_request_counts`, added in PR #55). The fetch layer talks to the Git protocol
endpoint, not the REST API, so a rate-limited API never blocks the actual object transfer.

**6. The head commit exists locally but hides behind a synthetic ref.** The squash-merge trap
from the worked trace. The decision tree probes commits, not content: a full clone with every
needed blob still takes the disposable path if the head commit object is absent. The alternates
borrow makes this path cheap, and fetching the synthetic ref into the clone by hand upgrades it
to fully local. Recognizing this state and offering the fetch hint in the UI was noted in the
session as a possible future affordance, not built.

**7. The process dies with a workspace on disk.** `Drop` never runs on `SIGKILL` or a panic
that aborts. The reaper deletes any `pr-*.git` older than 24 hours on the next workspace
creation, scanning at most 256 entries. Until then the leak is bounded by what a shallow
blob-less fetch put on disk, in a directory (`~/.cache/quinjet/tmp/`) users are told is always
safe to delete wholesale.

**8. Name collisions in the tmp directory.** The `pr-<pid>-<id>.git` scheme makes collisions
require pid reuse against a surviving directory; the constructor skips existing paths for up
to 16 attempts before failing with "unable to allocate a unique disposable Git repository",
preferring a clean error over clobbering a directory some other process may own.

**9. A credential prompt would freeze a worker thread.** Every workspace Git call runs with
`GIT_TERMINAL_PROMPT=0`, so a fetch against a remote that wants interactive authentication
fails immediately with a readable error instead of blocking the pull-request preview lane
forever on a prompt no terminal will ever show. The `gh` side has the equivalent belt:
`GH_PROMPT_DISABLED=1`.

**10. A subprocess floods its pipes.** Fetch stdout is capped at 128 KiB and stderr at 256 KiB
(`MAX_GH_ERROR_BYTES`); the diff reads are capped at 8 MiB. All of it flows through
`run_bounded_command`, which kills the child the moment a cap is crossed rather than buffering
first and truncating later; the test `bounded_runner_kills_oversized_git_output`
(`src/git/github/mod.rs:3090`) pins the exact behavior with a 256 KiB blob read under a
1,024-byte cap. A fetch writes its payload into the object store rather than stdout, so its
pipe caps only ever clip diagnostics.

**11. Shallow-specific surprises stay contained.** Inside the workspace, `git merge-base`
failing is an expected signal handled as `Ok(None)`; the shallow file holding boundary commits
from two disconnected history islands is normal and sufficient for tree-to-tree diffing; and no
history-walking command (log, blame, ahead/behind) ever runs in the workspace, so no user-facing
feature can trip over the boundary. The opened repository is never made shallow by anything
Quinjet does, so the main views never inherit these caveats.

**12. Time is read defensively.** The reaper compares directory mtimes against a 24-hour
threshold through `duration_since`, discarding entries whose clocks disagree rather than
erroring, and the cache layer treats an unreadable mtime as age zero. A machine with a jumping
clock can at worst delay a sweep or extend a TTL, never crash a load.

## The benchmark clone at /tmp/bun-test

Every measured number in this page and its siblings came from one reproducible setup: a
shallow, single-branch, blob-filtered clone of oven-sh/bun kept at `/tmp/bun-test`. It is
itself a working example of every mechanism this page describes, which makes it worth
documenting precisely.

### Properties on disk

As recorded in the session notes and verified on disk at the time of writing: the clone
occupies 389 MB, `git rev-parse --is-shallow-repository` prints `true`, and its configuration
is exactly the promisor shape shown in the promisor section: `core.repositoryformatversion=1`,
a single-branch fetch refspec for `main`, `remote.origin.promisor=true`, and
`remote.origin.partialclonefilter=blob:none`. In other words: shallow boundary, scoped refs,
deferred blobs, recorded promise. Reproducing an equivalent clone is one command away with
[git-clone](https://git-scm.com/docs/git-clone)'s `--filter=blob:none`, `--single-branch`, and
`--depth` options.

### The benchmark protocol

The stack was exercised through the CLI verbs from inside the clone, all against pull request
30412:

```bash
quinjet pr view 30412
quinjet pr files 30412
quinjet pr diff 30412 [path]
quinjet pr conversation 30412
```

Cold-cache runs were isolated rather than approximated. The session notes record the method
as: `QUINJET_CACHE_DIR=$(mktemp -d) quinjet ...` "points every cache (metadata, immutable
patch/conversation/counts entries, and the disposable pr-*.git workspaces) at a throwaway
root", quoted in the digest as "exactly how I benchmarked the before/after numbers". The
blunter alternative, `rm -rf ~/.cache/quinjet`, is always safe because everything under the
cache root re-fetches. For warm-path measurements, `--refresh` is the interesting flag: it
bypasses the five-minute metadata TTL while keeping the commit-keyed immutable entries, which,
as the session put it, "can never go stale".

### An honest caveat about what the numbers show

The session notes record one methodological caveat that any future benchmark against this
clone must respect: the pre-stack baseline build also succeeded on bun#30412, "because bun#30412
is merged, so its head is reachable in main's shallow history; therefore correctness was not
the differentiator on this exact clone, timing was, and the baseline cold run was measured
separately." A merged pull request's endpoints being locally reachable changes which branch of
the decision tree runs, so the clone is a timing benchmark for the loading stack, not a
demonstration of the deleted-history failure modes; those were exercised separately, including
end to end against "another merged bun PR whose head commit is absent from your clone" for the
alternates borrow.

The measured results themselves are quoted in full in the worked-trace section above: metadata
in 1.7s, the cold 2,188-file index at 18.5s before the review-fix round and 6.3s after, 0.04s
warm, single-file patches in 0.1s, and 1.4s for a warm-metadata `q pr files 30412` after local
install. One adjacent number completes the picture of deliberate trade-offs: the newest-first
conversation paging introduced in the same stack moved the bun conversation fetch from 21s to
26s, "because the fixed code degrades honestly rather than caching a gapped page-1 read", a
regression accepted in exchange for the guarantee that the 500-entry cap only ever drops the
oldest activity. The full measurement story, including what each PR of the stack changed in
measured behavior, is assembled in [benchmarking](../benchmarking.md).

## Related pages

- [Git internals overview](./README.md): the group hub and reading order.
- [Object model](./object-model.md): the object store the fetched packs land in, and the
  lookup path (loose, packs, alternates, promisor) that lazy fetch extends.
- [Packfiles and deltas](./packfiles-and-deltas.md): the format of the bytes inside the
  `packfile` section, thin packs, and promisor packs on disk.
- [Merge bases and history](./merge-bases-and-history.md): merge-base semantics, why shallow
  history breaks local computation, and the compare-API resolution in depth.
- [Plumbing and porcelain](./plumbing-and-porcelain.md): the subprocess substrate, capped
  pipes, and the catalog of exact Git invocations.
- [Refs, index, and worktrees](./refs-index-and-worktrees.md): ref storage, the common
  directory the alternates borrow reads, and lock avoidance.
- [PR workspace](../github/pr-workspace.md): the `PreparedPullRequest` lifecycle around the
  fetches described here.
- [Prefetch](../github/prefetch.md): the batch scheduling that decides when blobs are lazily
  materialized.
- [API strategy](../github/api-strategy.md): the compare and pulls-files endpoints that feed
  the choreography its hints.
- [Caching](../github/caching.md): the immutable-key cache that makes warm loads free.
- [Progressive loading](../rendering/progressive-loading.md): what the reader sees while the
  transfers on this page are still in flight.
- [Benchmarking](../benchmarking.md): the full bun#30412 measurement story.
- [Techniques](../techniques.md): the cross-cutting catalog, including shallow and partial
  fetch, depth-1 point fetches, and API merge-base resolution as reusable patterns.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 2 | Check latency for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 3 | Check latency for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 5 | Check latency for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 8 | Check latency for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 9 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for Shallow and Partial Clone in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for Shallow and Partial Clone in an unavailable network | Record time to first useful rows |
| 34 | Check latency for Shallow and Partial Clone in an unavailable network | Record steady frame cost |
| 35 | Check latency for Shallow and Partial Clone in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for Shallow and Partial Clone in an unavailable network | Record Git and gh process count |
| 37 | Check latency for Shallow and Partial Clone in an unavailable network | Record maximum retained document bytes |
| 38 | Check latency for Shallow and Partial Clone in an unavailable network | Record cache disposition and complete key |
| 39 | Check latency for Shallow and Partial Clone in an unavailable network | Record stale reply rejection |
| 40 | Check latency for Shallow and Partial Clone in an unavailable network | Record visible state after failure |
| 41 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record time to first useful rows |
| 42 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record steady frame cost |
| 43 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record bytes accepted from child output |
| 44 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record Git and gh process count |
| 45 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record cache disposition and complete key |
| 47 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record stale reply rejection |
| 48 | Check latency for Shallow and Partial Clone in rapid keyboard navigation | Record visible state after failure |
| 49 | Check latency for Shallow and Partial Clone in a linked worktree | Record time to first useful rows |
| 50 | Check latency for Shallow and Partial Clone in a linked worktree | Record steady frame cost |
| 51 | Check latency for Shallow and Partial Clone in a linked worktree | Record bytes accepted from child output |
| 52 | Check latency for Shallow and Partial Clone in a linked worktree | Record Git and gh process count |
| 53 | Check latency for Shallow and Partial Clone in a linked worktree | Record maximum retained document bytes |
| 54 | Check latency for Shallow and Partial Clone in a linked worktree | Record cache disposition and complete key |
| 55 | Check latency for Shallow and Partial Clone in a linked worktree | Record stale reply rejection |
| 56 | Check latency for Shallow and Partial Clone in a linked worktree | Record visible state after failure |
| 57 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record time to first useful rows |
| 58 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record steady frame cost |
| 59 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record bytes accepted from child output |
| 60 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record Git and gh process count |
| 61 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record maximum retained document bytes |
| 62 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record cache disposition and complete key |
| 63 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record stale reply rejection |
| 64 | Check latency for Shallow and Partial Clone in cold and warm cache states | Record visible state after failure |
| 65 | Check peak memory for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 66 | Check peak memory for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 67 | Check peak memory for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 68 | Check peak memory for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 69 | Check peak memory for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 70 | Check peak memory for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 71 | Check peak memory for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 72 | Check peak memory for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 73 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 75 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 76 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 77 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 79 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 80 | Check peak memory for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 81 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 82 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 83 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 84 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 85 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 86 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 87 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 88 | Check peak memory for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 89 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 90 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 91 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 92 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 93 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
| 95 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record stale reply rejection |
| 96 | Check peak memory for Shallow and Partial Clone in a deeply diverged branch | Record visible state after failure |
| 97 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record time to first useful rows |
| 98 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record steady frame cost |
| 99 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record bytes accepted from child output |
| 100 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record Git and gh process count |
| 101 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record maximum retained document bytes |
| 102 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record cache disposition and complete key |
| 103 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record stale reply rejection |
| 104 | Check peak memory for Shallow and Partial Clone in an unavailable network | Record visible state after failure |
| 105 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record time to first useful rows |
| 106 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record steady frame cost |
| 107 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record bytes accepted from child output |
| 108 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record Git and gh process count |
| 109 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record cache disposition and complete key |
| 111 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record stale reply rejection |
| 112 | Check peak memory for Shallow and Partial Clone in rapid keyboard navigation | Record visible state after failure |
| 113 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record time to first useful rows |
| 114 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record steady frame cost |
| 115 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record bytes accepted from child output |
| 116 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record Git and gh process count |
| 117 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record maximum retained document bytes |
| 118 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record cache disposition and complete key |
| 119 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record stale reply rejection |
| 120 | Check peak memory for Shallow and Partial Clone in a linked worktree | Record visible state after failure |
| 121 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record time to first useful rows |
| 122 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record steady frame cost |
| 123 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record bytes accepted from child output |
| 124 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record Git and gh process count |
| 125 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record maximum retained document bytes |
| 126 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record cache disposition and complete key |
| 127 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record stale reply rejection |
| 128 | Check peak memory for Shallow and Partial Clone in cold and warm cache states | Record visible state after failure |
| 129 | Check network transfer for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 130 | Check network transfer for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 131 | Check network transfer for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 132 | Check network transfer for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 133 | Check network transfer for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 134 | Check network transfer for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 135 | Check network transfer for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 136 | Check network transfer for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 137 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 139 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 140 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 141 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 143 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 144 | Check network transfer for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 145 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 146 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 147 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 148 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 149 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 150 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 151 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 152 | Check network transfer for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 153 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 154 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 155 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 156 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 157 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
| 159 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record stale reply rejection |
| 160 | Check network transfer for Shallow and Partial Clone in a deeply diverged branch | Record visible state after failure |
| 161 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record time to first useful rows |
| 162 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record steady frame cost |
| 163 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record bytes accepted from child output |
| 164 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record Git and gh process count |
| 165 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record maximum retained document bytes |
| 166 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record cache disposition and complete key |
| 167 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record stale reply rejection |
| 168 | Check network transfer for Shallow and Partial Clone in an unavailable network | Record visible state after failure |
| 169 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record time to first useful rows |
| 170 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record steady frame cost |
| 171 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record bytes accepted from child output |
| 172 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record Git and gh process count |
| 173 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record cache disposition and complete key |
| 175 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record stale reply rejection |
| 176 | Check network transfer for Shallow and Partial Clone in rapid keyboard navigation | Record visible state after failure |
| 177 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record time to first useful rows |
| 178 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record steady frame cost |
| 179 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record bytes accepted from child output |
| 180 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record Git and gh process count |
| 181 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record maximum retained document bytes |
| 182 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record cache disposition and complete key |
| 183 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record stale reply rejection |
| 184 | Check network transfer for Shallow and Partial Clone in a linked worktree | Record visible state after failure |
| 185 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record time to first useful rows |
| 186 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record steady frame cost |
| 187 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record bytes accepted from child output |
| 188 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record Git and gh process count |
| 189 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record maximum retained document bytes |
| 190 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record cache disposition and complete key |
| 191 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record stale reply rejection |
| 192 | Check network transfer for Shallow and Partial Clone in cold and warm cache states | Record visible state after failure |
| 193 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 194 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 195 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 196 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 197 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 198 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 199 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 200 | Check subprocess count for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 201 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 203 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 204 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 205 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 207 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 208 | Check subprocess count for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 209 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 210 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 211 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 212 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 213 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 214 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 215 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 216 | Check subprocess count for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 217 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 218 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 219 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 220 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 221 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
| 223 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record stale reply rejection |
| 224 | Check subprocess count for Shallow and Partial Clone in a deeply diverged branch | Record visible state after failure |
| 225 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record time to first useful rows |
| 226 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record steady frame cost |
| 227 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record bytes accepted from child output |
| 228 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record Git and gh process count |
| 229 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record maximum retained document bytes |
| 230 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record cache disposition and complete key |
| 231 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record stale reply rejection |
| 232 | Check subprocess count for Shallow and Partial Clone in an unavailable network | Record visible state after failure |
| 233 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record time to first useful rows |
| 234 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record steady frame cost |
| 235 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record bytes accepted from child output |
| 236 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record Git and gh process count |
| 237 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record maximum retained document bytes |
| 238 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record cache disposition and complete key |
| 239 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record stale reply rejection |
| 240 | Check subprocess count for Shallow and Partial Clone in rapid keyboard navigation | Record visible state after failure |
| 241 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record time to first useful rows |
| 242 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record steady frame cost |
| 243 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record bytes accepted from child output |
| 244 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record Git and gh process count |
| 245 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record maximum retained document bytes |
| 246 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record cache disposition and complete key |
| 247 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record stale reply rejection |
| 248 | Check subprocess count for Shallow and Partial Clone in a linked worktree | Record visible state after failure |
| 249 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record time to first useful rows |
| 250 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record steady frame cost |
| 251 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record bytes accepted from child output |
| 252 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record Git and gh process count |
| 253 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record maximum retained document bytes |
| 254 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record cache disposition and complete key |
| 255 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record stale reply rejection |
| 256 | Check subprocess count for Shallow and Partial Clone in cold and warm cache states | Record visible state after failure |
| 257 | Check cache identity for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 258 | Check cache identity for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 259 | Check cache identity for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 260 | Check cache identity for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 261 | Check cache identity for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 262 | Check cache identity for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 263 | Check cache identity for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 264 | Check cache identity for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 265 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 266 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 267 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 268 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 269 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 270 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 271 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 272 | Check cache identity for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 273 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 274 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 275 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 276 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 277 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 278 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 279 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 280 | Check cache identity for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 281 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 282 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 283 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 284 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 285 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 286 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
| 287 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record stale reply rejection |
| 288 | Check cache identity for Shallow and Partial Clone in a deeply diverged branch | Record visible state after failure |
| 289 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record time to first useful rows |
| 290 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record steady frame cost |
| 291 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record bytes accepted from child output |
| 292 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record Git and gh process count |
| 293 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record maximum retained document bytes |
| 294 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record cache disposition and complete key |
| 295 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record stale reply rejection |
| 296 | Check cache identity for Shallow and Partial Clone in an unavailable network | Record visible state after failure |
| 297 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record time to first useful rows |
| 298 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record steady frame cost |
| 299 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record bytes accepted from child output |
| 300 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record Git and gh process count |
| 301 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record maximum retained document bytes |
| 302 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record cache disposition and complete key |
| 303 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record stale reply rejection |
| 304 | Check cache identity for Shallow and Partial Clone in rapid keyboard navigation | Record visible state after failure |
| 305 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record time to first useful rows |
| 306 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record steady frame cost |
| 307 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record bytes accepted from child output |
| 308 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record Git and gh process count |
| 309 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record maximum retained document bytes |
| 310 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record cache disposition and complete key |
| 311 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record stale reply rejection |
| 312 | Check cache identity for Shallow and Partial Clone in a linked worktree | Record visible state after failure |
| 313 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record time to first useful rows |
| 314 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record steady frame cost |
| 315 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record bytes accepted from child output |
| 316 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record Git and gh process count |
| 317 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record maximum retained document bytes |
| 318 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record cache disposition and complete key |
| 319 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record stale reply rejection |
| 320 | Check cache identity for Shallow and Partial Clone in cold and warm cache states | Record visible state after failure |
| 321 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 322 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 323 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 324 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 325 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 326 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 327 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 328 | Check concurrency ordering for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 329 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 330 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 331 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 332 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 333 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 334 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 335 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 336 | Check concurrency ordering for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 337 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 338 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 339 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 340 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 341 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 342 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 343 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 344 | Check concurrency ordering for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 345 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 346 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 347 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 348 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 349 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 350 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
| 351 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record stale reply rejection |
| 352 | Check concurrency ordering for Shallow and Partial Clone in a deeply diverged branch | Record visible state after failure |
| 353 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record time to first useful rows |
| 354 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record steady frame cost |
| 355 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record bytes accepted from child output |
| 356 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record Git and gh process count |
| 357 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record maximum retained document bytes |
| 358 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record cache disposition and complete key |
| 359 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record stale reply rejection |
| 360 | Check concurrency ordering for Shallow and Partial Clone in an unavailable network | Record visible state after failure |
| 361 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record time to first useful rows |
| 362 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record steady frame cost |
| 363 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record bytes accepted from child output |
| 364 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record Git and gh process count |
| 365 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record maximum retained document bytes |
| 366 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record cache disposition and complete key |
| 367 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record stale reply rejection |
| 368 | Check concurrency ordering for Shallow and Partial Clone in rapid keyboard navigation | Record visible state after failure |
| 369 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record time to first useful rows |
| 370 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record steady frame cost |
| 371 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record bytes accepted from child output |
| 372 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record Git and gh process count |
| 373 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record maximum retained document bytes |
| 374 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record cache disposition and complete key |
| 375 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record stale reply rejection |
| 376 | Check concurrency ordering for Shallow and Partial Clone in a linked worktree | Record visible state after failure |
| 377 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record time to first useful rows |
| 378 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record steady frame cost |
| 379 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record bytes accepted from child output |
| 380 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record Git and gh process count |
| 381 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record maximum retained document bytes |
| 382 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record cache disposition and complete key |
| 383 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record stale reply rejection |
| 384 | Check concurrency ordering for Shallow and Partial Clone in cold and warm cache states | Record visible state after failure |
| 385 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record time to first useful rows |
| 386 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record steady frame cost |
| 387 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record bytes accepted from child output |
| 388 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record Git and gh process count |
| 389 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record maximum retained document bytes |
| 390 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record cache disposition and complete key |
| 391 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record stale reply rejection |
| 392 | Check failure degradation for Shallow and Partial Clone in a small local repository | Record visible state after failure |
| 393 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record time to first useful rows |
| 394 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record steady frame cost |
| 395 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record bytes accepted from child output |
| 396 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record Git and gh process count |
| 397 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record maximum retained document bytes |
| 398 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record cache disposition and complete key |
| 399 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record stale reply rejection |
| 400 | Check failure degradation for Shallow and Partial Clone in a monorepo with many changed paths | Record visible state after failure |
| 401 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record time to first useful rows |
| 402 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record steady frame cost |
| 403 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record bytes accepted from child output |
| 404 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record Git and gh process count |
| 405 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record maximum retained document bytes |
| 406 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record cache disposition and complete key |
| 407 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record stale reply rejection |
| 408 | Check failure degradation for Shallow and Partial Clone in a pull request containing generated files | Record visible state after failure |
| 409 | Check failure degradation for Shallow and Partial Clone in a deeply diverged branch | Record time to first useful rows |
| 410 | Check failure degradation for Shallow and Partial Clone in a deeply diverged branch | Record steady frame cost |
| 411 | Check failure degradation for Shallow and Partial Clone in a deeply diverged branch | Record bytes accepted from child output |
| 412 | Check failure degradation for Shallow and Partial Clone in a deeply diverged branch | Record Git and gh process count |
| 413 | Check failure degradation for Shallow and Partial Clone in a deeply diverged branch | Record maximum retained document bytes |
| 414 | Check failure degradation for Shallow and Partial Clone in a deeply diverged branch | Record cache disposition and complete key |
