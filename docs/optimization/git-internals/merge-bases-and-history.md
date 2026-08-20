# Merge Bases and History

Every pull-request diff Quinjet renders is a diff against a merge base, and finding that one
commit is the difference between a view that opens in seconds and a load that fetches thousands
of commits it will never show. This page covers the commit DAG from first principles: what a
merge base is, why there can be more than one, what two-dot and three-dot notation actually
select, how commit-graph generation numbers make ancestry walks cheap, and why shallow history
breaks local merge-base computation. It then walks the exact code Quinjet uses to resolve a
merge base: the local `git merge-base` fast path, the GitHub compare API hint cached under
`pr-merge-base-v1`, the depth-1 hint fetch that short-circuits the deepening ladder, the ladder
fallback that now reaches 16,384 commits instead of hard-failing at 4,096, and the review
finding about stale hints after a force-push that shaped the final code.

## Contents

- [The commit DAG](#the-commit-dag)
- [Merge-base semantics](#merge-base-semantics)
- [Two-dot versus three-dot](#two-dot-versus-three-dot)
- [Why a PR diff is a merge-base diff](#why-a-pr-diff-is-a-merge-base-diff)
- [Commit-graph files and generation numbers](#commit-graph-files-and-generation-numbers)
- [Shallow history breaks local merge-base](#shallow-history-breaks-local-merge-base)
- [Quinjet's merge-base pipeline](#quinjets-merge-base-pipeline)
- [The stale hint after a force-push](#the-stale-hint-after-a-force-push)
- [Caching on immutable history](#caching-on-immutable-history)
- [Topological order in the history pane](#topological-order-in-the-history-pane)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Measured behavior on the benchmark PR](#measured-behavior-on-the-benchmark-pr)

## The commit DAG

### Commit objects are the edges

Git history is not a data structure layered on top of the object store; it is the object store.
A commit object is a small text blob whose header names a tree, zero or more parents, an author,
and a committer. The parent lines are the entire graph: there is no separate index of edges, no
adjacency table, nothing to update when history grows. Reading history means reading commit
objects and following their parent fields.

The byte layout of a commit object body (after the `"commit <size>\0"` object header described
in [object-model](./object-model.md)) is a sequence of LF-terminated header lines followed by a
blank line and the free-form message:

```text
tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904
parent 3a672e3d4f558fe8a67e0197ff510dd488adf9ec
parent b75be8274893de8a56a1be72793c527d83693b03
author A Committer <a@example.com> 1755680000 +0530
committer A Committer <a@example.com> 1755680000 +0530

Merge the feature branch
```

Field by field:

| Field | Count | Meaning |
|---|---|---|
| `tree` | exactly 1 | The root tree snapshot this commit records |
| `parent` | 0 or more | Full OID of each parent, in order; first parent is the merged-into line |
| `author` | exactly 1 | Name, email, epoch seconds, UTC offset of the change author |
| `committer` | exactly 1 | Same shape; the identity and instant of commit creation |
| message | rest | Free text after the first blank line |

Three structural facts follow directly from this layout, and all three matter to Quinjet:

**1. The graph is append-only and immutable.** A commit's OID is the hash of these exact bytes,
parents included. Changing a parent, a message, or a tree produces a different OID, which means a
different commit. History is never edited in place; a rebase or an amend writes new commits and
moves refs. Consequently a pair of commit OIDs names a fixed slice of history forever, which is
the entire basis of Quinjet's immutable cache keys (see
[Caching on immutable history](#caching-on-immutable-history)).

**2. The graph is directed and acyclic.** Edges point from child to parent, and a cycle is
cryptographically impossible: a commit's OID depends on its parents' OIDs, so a commit cannot
name a descendant of itself without predicting a hash of data that includes the prediction.
Every ancestry question is therefore a question about a DAG, and DAG algorithms (reachability,
lowest common ancestor, topological sort) apply without cycle checks.

**3. A commit records a snapshot, not a delta.** The `tree` line names the complete state of the
project. Diffs are computed on demand between two trees; they are not stored. This is why a
pull-request diff needs exactly two commits to exist locally, the merge base and the head, and
nothing between them. Git compares their root trees, recurses into subtrees whose OIDs differ,
and never looks at the intervening commits. Quinjet's whole fetch strategy is built on this
fact: it goes to great lengths to obtain those two commits and deliberately avoids obtaining
anything else.

### Reachability as a partial order

Commit A *reaches* commit B when B is A itself or B can be found by repeatedly following parent
edges from A. Reachability is reflexive, transitive, and antisymmetric, so it defines a partial
order on commits: B is an *ancestor* of A when A reaches B, and two commits with no order
between them are the tips of diverged lines of work. Almost every Git operation is phrased in
this order:

- `git log X` enumerates the down-set of X: every commit X reaches.
- `git log X --not Y` (equivalently `Y..X`) enumerates the down-set of X minus the down-set of
  Y.
- A branch is *merged* when its tip is in the down-set of the target branch.
- A push is a *fast-forward* when the old tip is in the down-set of the new tip.
- A merge base of A and B is a maximal element of the intersection of their down-sets.

The partial order is exactly why "which commit do I diff against" is a real question. In a
linear history the answer is trivial: the older endpoint. In a DAG two branch tips generally
have no order between them, so "the changes on this branch" has to be defined relative to a
third commit, and choosing that commit well is the subject of this page.

### A worked DAG

Every later section refers back to this ten-commit history. `main` moved on after the feature
branch forked, and the feature branch absorbed one merge from `main` midway:

```text
        A --- B --- C --- D --- E --- F        <- main
               \               /
                G --- H --- I-'                (I merged into F)
                 \
                  J --- K                      <- feature
```

Written as parent lists, child to parent:

```text
B <- A          F <- E, I
C <- B          G <- B
D <- C          H <- G
E <- D          I <- H
                J <- G
K <- J          (K is the tip of feature)
```

Down-sets of the two interesting tips:

```text
reach(F) = {F, E, I, D, H, C, G, B, A}
reach(K) = {K, J, G, B, A}
common   = {G, B, A}
```

The common ancestors of `main` and `feature` are G, B, and A. Among them G reaches B and A, and
nothing in the common set reaches G, so G is the unique *best* common ancestor: the merge base.
Note what G is in project terms: the last commit the feature branch actually started from.
Everything in `reach(F)` that is not in `reach(K)` (C, D, E, H, I, F) is work the feature branch
never contained, and a correct "what does this branch change" answer must not include or invert
any of it.

### The cost model of walking the DAG

Answering a reachability question by brute force means loading commit objects and following
parents until the question is settled. Three properties determine the cost:

- **Object load cost.** Each step inflates a commit object, from a loose zlib file or from a
  packfile via its delta chain (see [packfiles-and-deltas](./packfiles-and-deltas.md)). Commits
  are small, but a walk over a large repository touches hundreds of thousands of them.
- **Frontier width.** A DAG walk keeps a priority queue of unexplored commits. Wide histories
  (many concurrent branches, octopus merges) widen the frontier and the queue.
- **Stopping conditions.** Without extra metadata, Git can only bound a walk with heuristics
  such as commit timestamps, which are supplied by machines with wrong clocks and are therefore
  only advisory. The commit-graph file replaces the heuristic with a sound bound
  (see [Commit-graph files and generation numbers](#commit-graph-files-and-generation-numbers)).

For Quinjet the cost model has a fourth axis that dominates the other three: **whether the
commits exist locally at all**. In a disposable pull-request workspace history is fetched over
the network before it can be walked, so the true cost of a merge-base computation is the bytes
of history transferred, not the CPU of the walk. Quinjet's design goal is to make the transfer
proportional to the answer (one commit) rather than to the divergence (potentially tens of
thousands of commits), and the rest of this page shows how it gets there.

## Merge-base semantics

### The definition

For commits A and B, a *common ancestor* is any commit both A and B reach. A *best* common
ancestor, a merge base, is a common ancestor that is not an ancestor of any other common
ancestor. Formally, the merge bases of A and B are the maximal elements of
`reach(A) ∩ reach(B)` under the reachability order. The definition deliberately says "a", not
"the": on a DAG the intersection of two down-sets can have several maximal elements, and Git's
[`git merge-base`](https://git-scm.com/docs/git-merge-base) documentation is explicit that more
than one merge base can exist.

Three consequences of the definition are worth internalizing before reading any of the code:

**1. A merge base is a real commit, not a synthetic point.** It has a tree, and that tree can be
diffed against. This is what makes "diff against the merge base" a well-defined, computable
operation rather than an abstraction.

**2. The merge base moves only when history moves.** For a fixed pair of OIDs the merge base is
a pure function of the immutable DAG below them. Ask the same question about the same two
commits next year and the answer is byte-identical. This purity is why Quinjet caches API
merge-base answers under `CacheLife::Immutable` and never expires them.

**3. The merge base is a property of the pair, not of either commit.** Retarget a pull request
to a different base branch and the base OID changes, so the question changes, so the cached
answer for the old pair remains true and a new cache entry is created for the new pair. Cache
correctness by key construction, not by invalidation.

### A single base: the common case

In the worked DAG above, `git merge-base F K` prints G. The classic algorithm behind it is
called paint-down-to-common in Git's source: walk from both tips simultaneously, painting
commits with a PARENT1 flag when reached from the first tip and PARENT2 when reached from the
second, using a priority queue ordered by commit date (or generation number when a commit-graph
is present). A commit painted with both flags is a common ancestor; its ancestors are then
painted STALE because they are dominated and cannot be maximal. When the queue holds only STALE
entries the surviving both-flags commits are the merge bases.

Step by step on the worked DAG for tips F and K:

```text
queue: F(P1), K(P2)
pop F   -> paint E(P1), I(P1)
pop K   -> paint J(P2)
pop E   -> paint D(P1)
pop I   -> paint H(P1)
pop J   -> paint G(P2)
pop D   -> paint C(P1)
pop H   -> G gains P1  => G now P1|P2: common ancestor; mark G's ancestors STALE
pop C   -> paint B(P1) (B already below G: STALE propagates)
queue drains to STALE-only entries; result = {G}
```

The walk visited nine commits to answer a question about a ten-commit repository. On a
real repository the same shape of walk can visit hundreds of thousands of commits, which is
acceptable locally and catastrophic when each commit must first be fetched.

### Multiple merge bases

Nothing in the definition forces a unique maximum, only maximal elements. The canonical shape
that produces two merge bases is two branches that each merged the other once:

```text
        A --- B ------- D ---- F      <- branch-x
               \      /   \
                \    /     \
                 -- C ------ E        <- branch-y
                   (D merges C; E merges B via D's line)
```

A cleaner minimal construction, the one Git's own documentation uses, is below in the
criss-cross section. When multiple bases exist:

- `git merge-base A B` prints one of them (which one is deliberately unspecified).
- `git merge-base --all A B` prints all of them.
- `git merge` with the default strategy does something smarter than picking one: it recursively
  merges the merge bases with each other to synthesize a *virtual ancestor*, then uses that as
  the base for the real merge. That recursion is where the historical `recursive` strategy got
  its name, and its successor `ort` keeps the behavior.

For diffs the multiplicity is quietly resolved by fiat: `git diff A...B` diffs against *a*
merge base, the one plain `git merge-base` returns. A pull-request diff inherits that
arbitrariness. In practice this is acceptable because multiple-base situations are rare on
GitHub-style workflows (they require merging in both directions between the same two lines),
and because both sides of Quinjet's pipeline, the compare API and local `git merge-base`,
resolve the ambiguity the same way: one base, deterministically chosen by the walk, then used
consistently as half of every cache key.

### Criss-cross histories

The criss-cross is the minimal history with two best common ancestors, and it is worth building
by hand once. Start with a fork, then merge each side into the other *concurrently*:

```text
        ---1---o---A          <- tip A
            \ / \
             X   \
            / \   \
        ---2---o---B          <- tip B
```

Reading the picture: commit 1 and commit 2 are two lines of work; each line then merges the
other line's pre-merge tip (the crossing edges through X), producing one merge on each side; A
and B are later commits on each line. Now compute the common ancestors of A and B: both 1 and 2
are reachable from both tips (1 through A's own line and through B's merge; 2 symmetrically).
Neither 1 nor 2 reaches the other. Both are maximal. Two merge bases.

```text
reach(A) ⊇ {1, 2, ...}    reach(B) ⊇ {1, 2, ...}
1 not in reach-from(2), 2 not in reach-from(1)
merge bases(A, B) = {1, 2}
```

Criss-crosses arise in real life from back-and-forth merging between long-lived branches
(release branch and mainline merging into each other), from mirrored repositories that merge
both directions, and from automation that syncs branches bidirectionally. Their practical
effect on this page's subject is mild but real: a single "the merge base" does not exist, so
any system that stores one merge base per commit pair (as Quinjet's cache does) is storing *a*
representative, and correctness requires only that the representative be used consistently on
both sides of the diff. Quinjet satisfies that by construction: whichever base OID the resolver
produced is the OID passed to every subsequent `git diff` and baked into every cache key, so
the file list, the numstat counts, and every patch all describe the same base.

### The merge-base command family

The plumbing around merge bases is broader than the one command, and several relatives appear
either in Quinjet's code or in the mental model behind it. All are documented at
[git-merge-base](https://git-scm.com/docs/git-merge-base):

| Invocation | Question it answers |
|---|---|
| `git merge-base A B` | One best common ancestor of A and B |
| `git merge-base --all A B` | Every best common ancestor |
| `git merge-base --octopus A B C...` | A base for an octopus merge over all listed tips |
| `git merge-base --independent A B C...` | Which of the listed commits are not reachable from the others |
| `git merge-base --is-ancestor A B` | Exit 0 iff A is an ancestor of B (no output) |
| `git merge-base --fork-point ref A` | Reflog-assisted guess of where A forked off ref |

Exit-code semantics matter to Quinjet: `git merge-base A B` exits 1 with empty stdout when the
two commits share no common ancestor at all. In a complete repository that means genuinely
unrelated histories (two roots), which is rare. In a *shallow* repository it happens routinely,
because the walk hits the shallow boundary before reaching the real common ancestor. Quinjet's
`try_merge_base` in `src/git/github/mod.rs` leans on exactly this: a non-zero exit is not an
error, it is the signal "the answer is not inside the history fetched so far, deepen and retry".
The function is quoted in full in
[Quinjet's merge-base pipeline](#quinjets-merge-base-pipeline).

`--fork-point` deserves a caution because it looks like a better merge base and is not: it
consults the *reflog* of the ref to find where a branch actually forked, which produces nicer
rebases locally but depends on private, mutable, machine-local state. It can disagree with the
DAG, it changes as reflog entries expire, and it is meaningless in a freshly created bare
workspace that has no reflog history at all. A cacheable, reproducible system must use the pure
DAG definition, and Quinjet does.

### Merge bases and rename detection

One subtlety connects merge-base choice to diff quality. Git detects renames per diff, between
the two endpoint trees, with `--find-renames` scoring content similarity (see
[algorithms](../diff/algorithms.md)). The further apart the endpoints, the more unrelated churn
sits between them and the more candidates rename detection must consider. Diffing against the
merge base rather than against an older or newer unrelated commit keeps the endpoint trees as
close as the branch actually is to its fork, which keeps rename pairing accurate: a file moved
on `main` after the fork does not appear moved in the PR diff, because the merge-base tree
still holds it at the old path only if the *branch* saw it there. Quinjet passes
`--find-renames` on every index and patch command against the merge-base/head pair, so rename
records in the PR file tree reflect what the branch did, not what `main` did meanwhile.

## Two-dot versus three-dot

Git overloads `..` and `...` with different meanings in *revision walks* (`git log`,
`git rev-list`) and in *diffs* (`git diff`). The two tables are easy to conflate and the
confusion is common enough that this section pins both down precisely, because the pull-request
diff definition lives exactly in the gap between them.

### In revision walks

For [`git log`](https://git-scm.com/docs/git-log) and
[`git rev-list`](https://git-scm.com/docs/git-rev-list), the dots are set operations on
down-sets:

| Notation | Set produced |
|---|---|
| `A..B` | `reach(B) - reach(A)`: commits reachable from B but not from A |
| `A...B` | `(reach(A) ∪ reach(B)) - (reach(A) ∩ reach(B))`: the symmetric difference |

`A..B` is sugar for `B ^A` ("B, not A"). On the worked DAG:

```text
git log F..K   = {K, J}                      (feature's own commits)
git log K..F   = {F, E, I, D, H, C}          (what main gained since the fork)
git log F...K  = {K, J, F, E, I, D, H, C}    (both sides, no common history)
```

`git log --left-right A...B` additionally marks each commit with `<` or `>` for which side
contributed it, which is how "ahead 2, behind 6" summaries are computed. Note that the
symmetric difference never contains a merge base: the base is in both down-sets, so it is
subtracted.

### In diffs

For [`git diff`](https://git-scm.com/docs/git-diff) the operands are two *endpoints*, not two
sets, and the dots select which endpoints:

| Notation | Equivalent | Endpoints diffed |
|---|---|---|
| `git diff A B` | itself | tree of A against tree of B |
| `git diff A..B` | `git diff A B` | identical to the two-argument form |
| `git diff A...B` | `git diff $(git merge-base A B) B` | tree of the merge base against tree of B |

So in diff-land, two-dot is a no-op alias and three-dot is the interesting one: it silently
substitutes the merge base for the left endpoint. The mnemonic that survives the overloading:
*in a walk, dots subtract history; in a diff, three dots substitute the merge base*.

### A worked comparison

Apply both diffs to the worked DAG, asking about `feature` (tip K) relative to `main` (tip F).
Suppose main's post-fork commits C, D, E edited `runtime.c`, and feature's commits J and K
edited `parser.c`.

`git diff F K` (two-dot semantics) compares the trees of F and K directly. K's tree lacks every
`runtime.c` change because the branch forked before them, so the diff shows `parser.c` changes
*plus the removal of every main-side change since G*. The output claims the feature branch
deletes work it has simply never seen. On the bun benchmark PR this is not a small distortion:
the base branch moved from May 9 to Aug 11 while the PR was open, so a two-dot diff would bury
the PR's real content under three months of inverted mainline history.

`git diff F...K` (three-dot semantics) first computes `merge-base F K = G`, then compares the
trees of G and K. The result is exactly `parser.c` as the branch changed it: the changes the
branch author made, no more, no less. This is the diff a reviewer means when asking "what does
this branch change", and it is the only one of the two that stays stable while the base branch
moves: G is fixed by the fork, so mainline commits landing after the fork do not perturb the
diff at all.

One asymmetry worth noting: three-dot is not symmetric. `git diff A...B` diffs base against B;
`git diff B...A` diffs base against A. A pull request always puts the head on the right.

### Counting with the dots: ahead and behind

The walk-flavored dots also power every "ahead N, behind M" summary in the Git ecosystem, and
the counts are merge-base facts in disguise. `git rev-list --count --left-right A...B` walks
the symmetric difference once and prints two numbers: how many commits are only on A's side
and how many only on B's. On the worked DAG:

```text
git rev-list --count --left-right F...K
6       2
```

`main` is 6 commits ahead of the fork (C, D, E, H, I, F) and `feature` has 2 of its own
(J, K). Both counts measure distance *to the merge base*, even though the notation never names
it: the symmetric difference is exactly "everything above the common history", and the common
history's ceiling is the merge base.

The same numbers surface in two places Quinjet reads:

- **GitHub's compare response** carries a `status` (`ahead`, `behind`, `diverged`,
  `identical`) plus `ahead_by` and `behind_by` fields, which are these two counts computed on
  the server graph. Quinjet's jq projection discards them (only `merge_base_commit.sha`
  survives), but they are the same computation as its merge base, one field over.
- **Local branch state.** `git status --porcelain=v2 --branch` emits a `# branch.ab +N -M`
  header line, the ahead/behind of the current branch against its upstream, and Quinjet's
  status parser (`src/git/status.rs`) reads it into `BranchState { ahead, behind, .. }` for
  the sidebar. One status invocation therefore ships a small ancestry summary with every
  refresh, computed by Git against the upstream's merge base, at no extra subprocess cost
  (the parser is detailed in [plumbing-and-porcelain](./plumbing-and-porcelain.md)).

### Why Quinjet spells it explicitly

Quinjet never passes literal `...` notation to `git diff`. It resolves the merge base once,
holds it as a concrete OID in `PreparedPullRequest { merge_base, head, .. }`
(`src/git/github/mod.rs:385-391`), and issues every subsequent command with the two OIDs spelled
out, for example the index read in `changed_files_in_repository`:

```text
git diff --name-status -z --find-renames <merge_base> <head> --
```

The reasons are practical, and each maps to a mechanism elsewhere in this documentation:

- **One resolution, many reads.** `A...B` recomputes the merge base inside every git
  invocation. Quinjet issues one name-status read, one numstat read, and dozens of batched
  patch reads per pull request ([pipeline](../diff/pipeline.md)); resolving once and pinning
  the OID does the ancestry walk exactly once per workspace.
- **The OID is the cache key.** `pr-files-v1\n{merge_base}\n{head}`,
  `pr-numstat-v1\n{merge_base}\n{head}`, and `pr-patch-v1\n{merge_base}\n{head}\n{path}` all
  need a concrete base OID to be immutable keys. A notation that re-resolves per call could
  silently produce a different base between two reads (if the workspace deepened in between)
  and split one pull request's entries across inconsistent keys.
- **The resolver is pluggable.** The base OID may come from local `git merge-base`, from the
  GitHub compare API, or from the deepening ladder. Downstream code neither knows nor cares;
  it receives two OIDs. Spelling `...` would hard-wire local resolution back in.
- **A fetched hint has no local meaning.** In the hint short-circuit path the workspace holds
  the base *commit* but not the base *branch*; `base_ref...head` would be unresolvable there,
  while the explicit pair works because both OIDs exist locally.

## Why a PR diff is a merge-base diff

### The contract a pull request states

A pull request proposes: *apply what this branch did, on top of wherever the base branch is
when the merge happens*. The reviewable content is therefore "what this branch did", which is
the three-dot diff: merge base against head. Every hosting platform renders it this way, and
GitHub's own UI ("Files changed") is the three-dot diff of the PR's base and head.

The two-dot alternative fails the contract twice on an active repository:

**1. It misattributes mainline work.** As shown above, everything the base branch gained since
the fork appears as a *reversal* in a two-dot diff. A reviewer would see hundreds of files the
author never touched, all seemingly deleted or reverted.

**2. It is unstable under base movement.** Every push to the base branch changes the base tip
and therefore the two-dot diff, even though the PR branch is untouched. Review comments would
detach, file lists would churn, and caches keyed on the diff would invalidate constantly. The
merge-base diff is invariant under base-branch movement (the fork point does not move when
mainline advances), so it only changes when the PR itself changes: a new head commit, a
force-push, or an explicit retarget.

That stability is not a nicety for Quinjet, it is load-bearing: invariant 12 in ARCHITECTURE.md
rests on the fact that the pair (merge base, head) fully names the diff content, so "a new head
or a new comment therefore asks a different question rather than aging an old answer, so a
stale read is impossible and only eviction applies."

### GitHub's compare endpoint

GitHub exposes the underlying computation directly through the REST compare endpoint,
documented under [the GitHub REST API](https://docs.github.com/en/rest):

```text
GET /repos/{owner}/{repo}/compare/{base}...{head}
```

The `{base}...{head}` path segment is the three-dot notation embedded in a URL, and the operands
may be branch names, tags, or raw commit OIDs. The response describes the relationship between
the two commits: an ahead/behind status, the commit list between them, a capped file list, and,
crucially for this page, a `merge_base_commit` object whose `sha` field is the merge base
GitHub's own graph computed. GitHub maintains server-side ancestry data for every repository it
hosts, so this answer costs GitHub a graph lookup and costs the client one HTTPS round trip,
regardless of whether the merge base is one commit or forty thousand commits behind the tips.

Quinjet reads exactly one field of that response and discards the rest with a server-side `jq`
projection (the full mechanism is in [api-strategy](../github/api-strategy.md)):

```text
gh api repos/{owner}/{repo}/compare/{base_oid}...{head_oid} --jq .merge_base_commit.sha
```

Two details of how Quinjet phrases the question are deliberate:

- **It compares OIDs, not ref names.** The PR metadata snapshot carries `baseRefOid` and
  `headRefOid`; the compare URL is built from those, not from `main...feature-branch`. Asking
  about OIDs makes the question immutable (and therefore cacheable forever), and it pins the
  answer to the metadata snapshot rather than to whatever the branches point at during the
  request. The force-push consequences of that pinning are covered in
  [The stale hint after a force-push](#the-stale-hint-after-a-force-push).
- **It asks the base repository.** For a fork PR, base and head live in different repositories,
  but GitHub's compare on the base repository can see fork commits attached to a pull request,
  the same visibility that makes `refs/pull/N/head` fetchable from the base repository.

### The synthetic PR refs and the merge preview

GitHub materializes each pull request as real refs on the base repository, fetchable by any
client, and the choice between them is itself a merge-base decision:

| Ref | Points at | Stability |
|---|---|---|
| `refs/pull/{n}/head` | The PR branch's current tip, exactly as pushed | Moves only when the PR branch moves |
| `refs/pull/{n}/merge` | A test merge of head into the current base | Recomputed as the base moves; absent when conflicted |

Quinjet fetches the head ref, never the merge ref. `fetch_pull_request` builds the refspec
directly from the PR number (`src/git/github/mod.rs:1800-1801`):

```rust
let base_refspec = format!("+refs/heads/{}:refs/quinjet/base", pull_request.base_ref);
let pull_refspec = format!("+refs/pull/{}/head:refs/quinjet/head", pull_request.number);
```

The merge ref looks attractive at first glance (it is "the PR as it would land"), and every
reason it loses traces back to this page's theory:

- **It answers a different question.** Diffing the merge preview against the base tip shows
  the effect of merging *now*, which mixes the branch's changes with conflict resolutions and
  with the base's current state. The review contract is the three-dot diff of what the branch
  did; that is the head ref against the merge base.
- **It is mutable.** The merge preview is recomputed as the base branch moves, so it cannot
  anchor an immutable cache key, and a fetched copy goes stale the moment mainline advances.
  The head OID is immutable and pins everything.
- **It is conditional.** The merge ref exists only while GitHub considers the PR mergeable and
  its computation is refreshed lazily; a fetch strategy depending on it would need a fallback
  for conflicted or just-opened PRs anyway. `refs/pull/{n}/head` exists for every open PR and
  survives even fork deletion in most cases, which is why it is the first fetch attempt and
  the fork remote is only the fallback.

The head ref has one more property the pipeline quietly relies on: it exists *on the base
repository*, so a single `origin` remote reaches both the base branch and a fork's PR
commits, and the depth-1 merge-base hint fetch can use the same remote as everything else.

### The pulls files endpoint sees the same diff

The compare API is not the only place the merge-base diff surfaces. The
`pulls/{number}/files` endpoint, which Quinjet reads for per-file addition/deletion counts
(PR #49, detailed in [api-strategy](../github/api-strategy.md)), reports files *of the
three-dot diff*. This coherence matters: the counts Quinjet fetches from the API and attaches
to file headers describe the same base-to-head comparison that the local
`git diff <merge_base> <head>` in the workspace produces, so API-sourced counts and locally
generated patches agree file by file. If GitHub's files endpoint reported a two-dot diff, the
count-before-patch rendering strategy (invariant 8a) would show numbers that the arriving
patches then contradicted.

## Commit-graph files and generation numbers

Merge-base computation is an ancestry walk, and ancestry walks over large repositories are
bounded by two costs: inflating commit objects one by one, and not knowing when to stop. The
commit-graph file attacks both. It matters to this page for two reasons: it is why *local*
merge-base computation is fast in a normal clone (Quinjet's network-free fast path), and its
absence in shallow repositories is part of why the disposable-workspace path must be designed
around the network instead.

### What the file stores

The commit-graph is a supplementary, disposable index of commit metadata, stored at
`.git/objects/info/commit-graph` (or as a chain of files under
`.git/objects/info/commit-graphs/`). It duplicates, in a flat binary layout, exactly the fields
an ancestry walk needs: parents, commit date, root tree, and a generation number. A walk that
finds a commit in the graph file reads a fixed-size record by index instead of inflating and
parsing a zlib-compressed object, and octopus parents aside, follows parent *positions*
(integers) instead of resolving OIDs through the object store.

The file is managed by [`git commit-graph`](https://git-scm.com/docs/git-commit-graph)
(`git commit-graph write --reachable` builds it by hand) and modern Git maintains it
automatically: recent versions enable `core.commitGraph` by default and write the file during
garbage collection, and `fetch.writeCommitGraph` can extend it incrementally on fetch. It is
pure acceleration: deleting it changes no behavior, only speed, because every fact in it is
recomputable from the commit objects.

### The file format

The layout is a chunk-based binary format. The header:

| Offset | Size | Content |
|---|---|---|
| 0 | 4 bytes | Magic `CGPH` (0x43 0x47 0x50 0x48) |
| 4 | 1 byte | Format version (1) |
| 5 | 1 byte | Hash version (1 = SHA-1, 2 = SHA-256) |
| 6 | 1 byte | Number of chunks |
| 7 | 1 byte | Number of base commit-graph files (for chains) |

A chunk lookup table follows (4-byte chunk ID plus 8-byte file offset per chunk, terminated by
a null chunk ID), then the chunks themselves. The chunks that matter for ancestry:

| Chunk ID | Content |
|---|---|
| `OIDF` | Fanout: 256 4-byte counts, cumulative by first OID byte, for binary search |
| `OIDL` | The sorted list of commit OIDs in the graph |
| `CDAT` | One fixed-size record per commit: tree OID, two parent positions, generation, date |
| `EDGE` | Overflow list of extra parent positions for octopus merges |
| `GDA2` | Generation v2 data: corrected commit-date offsets |
| `BIDX` / `BDAT` | Optional changed-path Bloom filters (accelerate path-limited walks) |
| `BASE` | OIDs of base graph files when the graph is an incremental chain |

The `CDAT` record for one commit, with a SHA-1 hash:

| Bytes | Content |
|---|---|
| 0..20 | Root tree OID |
| 20..24 | Position of the first parent in the graph, or `0xFFFFFFFF` for none |
| 24..28 | Position of the second parent; high bit set means an index into `EDGE` instead |
| 28..36 | Packed: 30-bit generation number, then 34-bit commit time in epoch seconds |

The packing in the last eight bytes is the detail worth staring at: the generation number and
the commit date share one 64-bit field, generation in the upper 30 bits. A walk sorting its
frontier by "generation, then date" can compare these fields as single integers. Positions
instead of OIDs mean that following a parent edge inside the graph is array indexing, no hash
lookup at all.

### Generation numbers v1: topological levels

The generation number of a commit C is defined recursively:

```text
gen(C) = 1                          if C has no parents
gen(C) = max(gen(P) for P in parents(C)) + 1
```

Root commits are generation 1; every commit is strictly deeper than all of its parents. The
single theorem that makes the number useful:

```text
if A reaches B and A != B, then gen(A) > gen(B)
```

Contrapositive, in the form a walk uses: **if `gen(X) <= gen(B)` and `X != B`, then X cannot
reach B**, so a search for B can discard X and everything the queue would have explored beneath
it. This converts "when do I stop walking" from a date heuristic into a sound cutoff. The date
heuristic it replaces (`commitGraph` predates none of this logic; Git walked by committer date
for years) is unsound because commit dates come from machine clocks: a commit created on a
machine with a slow clock can be *older-dated than its own parent*, and a date-bounded walk can
then stop early and return a wrong or missed merge base. Generation numbers cannot be skewed;
they are derived from the graph itself.

Worked on the DAG from earlier:

```text
gen: A=1  B=2  C=3  D=4  E=5  G=3  H=4  I=5  F=6  J=4  K=5
```

A merge-base walk for F (gen 6) and K (gen 5) processes its frontier highest-generation first:
F expands to E (5) and I (5); K expands to J (4); the frontier drains in generation order
6, 5, 5, 5, 4, 4, ... and when G (gen 3) is painted from both sides, every remaining queue
entry has generation <= 3, so nothing unexplored can dominate G and the walk can conclude
without touching A at all. On two tips whose merge base is recent, the walk now touches a
neighborhood of the tips instead of the whole history, regardless of how many hundreds of
thousands of commits sit below.

Commits absent from the graph file (fresh commits since the last graph write) are treated as
having infinite generation, which is safe: infinity only forces the walk to explore them, never
to skip them. Older graph files may store zero, which similarly disables the cutoff for that
commit rather than corrupting it.

### Corrected commit dates: generation v2

Topological levels have one weakness: they compress badly under wide history. A repository
whose imported history contains one very long chain forces high generation numbers everywhere
above it, weakening the cutoff (many commits share high levels, so `gen(X) <= gen(B)` rarely
triggers). Generation v2 keeps the same theorem but changes the function to the *corrected
commit date*:

```text
cdate(C) = max(date(C), max(cdate(P) for P in parents(C)) + 1)
```

The corrected date is the commit's own date, bumped just enough to be strictly greater than
every parent's corrected date. For history with sane clocks, `cdate` equals `date` almost
everywhere, so it retains the excellent discriminating power of real timestamps while repairing
exactly the clock-skewed spots that made raw dates unsound. The `GDA2` chunk stores each
commit's offset (`cdate - date`) so the file stays compact. The reachability theorem holds in
the same form, `A reaches B implies cdate(A) > cdate(B)` (strictness guaranteed by the `+ 1`),
so the same cutoff logic applies with a sharper bound.

### What this buys Quinjet, and where it cannot help

Quinjet does not manage commit-graph files, read them, or depend on their presence, and that is
the point: it treats `git` as the authority for graph computation (the same stance it takes for
diffs, see [algorithms](../diff/algorithms.md)), so it inherits the acceleration wherever Git
maintains a graph file, transparently.

- **The local fast path inherits it fully.** When both PR OIDs already exist in the opened
  repository, Quinjet runs plain `git merge-base <base_oid> <head_oid>` there
  (`src/git/github/mod.rs:852-863`). In a normally maintained clone that repository has a
  commit-graph, so the ancestry walk is generation-bounded and array-indexed. This is a
  component of why local-branch PR previews are effectively instant (invariant 9).
- **The disposable workspace cannot use it.** The temporary bare repository is seconds old,
  shallow, and blob-filtered. No graph file has been written, and more fundamentally Git
  declines to build or use commit-graph data in shallow repositories, because a graph built
  over grafted history would bake in wrong parent lists (the next section explains the graft).
  So inside the workspace every `git merge-base` attempt in the deepening ladder is an
  uncached, unaccelerated object walk over whatever shallow slice has been fetched so far.
  That is acceptable only because the slices are small; it would be another reason, on top of
  transfer cost, why "fetch until the ladder finds it" had to stop being the primary strategy.
- **GitHub's side has its own graph.** The compare API answers from server-side ancestry data
  over the *complete* repository. When Quinjet asks the API instead of walking locally, it is
  effectively borrowing a fully built, always-current reachability index it never has to
  download. One HTTPS request substitutes for both the history transfer and the walk.

## Shallow history breaks local merge-base

### What a shallow boundary does to the DAG

A shallow clone or fetch (`--depth=N`) transfers only the commits within N steps of the fetched
tips. The client records the frontier in `.git/shallow`: a plain text file, one commit OID per
line. Each listed commit is a *graft point*: the commit object still physically contains its
`parent` lines (the bytes are hashed, they cannot be removed), but Git treats the commit as if
it had no parents. Every walk, every `git log`, every `git merge-base` sees a DAG that simply
ends there.

```text
.git/shallow
------------
93b1dabd5c2b2fb9b74a02a95b4ba1b96b09baa0
b8b4d5a291e055a496cbbc06553d17b779a99231
```

The full mechanics of how the boundary is negotiated (the `shallow`/`unshallow` lines of the
fetch protocol, `--deepen`, `--shallow-since`) are covered in
[shallow-and-partial-clone](./shallow-and-partial-clone.md). What matters here is the effect on
ancestry semantics: **reachability queries in a shallow repository are computed over a
different graph than the true one**, the true DAG with every edge out of the boundary erased.

### Failure shapes

Apply that to a merge base. Fetch the PR head at depth 64 and the base branch at depth 64, as
Quinjet's ladder does on its first rung, and consider where the true merge base M can be:

**1. M is inside both windows.** Both tips reach M within 64 steps in the truncated graph.
`git merge-base` finds M and it is correct: presence of deeper history could only add common
ancestors that M dominates. A merge base found in a shallow repository is never *wrong* in this
direction; shallowness can hide the answer but cannot fabricate a better-looking false one
above M, because everything above M that is common would also be common in the full graph.

**2. M is outside either window.** The walk from one tip (or both) hits graft points and stops.
The two truncated down-sets do not intersect at all, so `git merge-base` finds *no* common
ancestor, exits 1, and prints nothing. The histories look unrelated even though they fork from
the same line a few hundred commits deeper.

Shape 2 is the routine case for a long-lived pull request. The bun benchmark PR was opened on
May 9 against a repository that kept moving until Aug 11; its merge base sits thousands of
commits behind the base branch tip. Any fixed shallow window around the tips misses it, and the
only local remedies are to deepen (transfer more history and retry) or to unshallow entirely
(transfer everything).

There is a third, quieter shape worth naming because it produces a *possible* wrong answer
rather than a missing one:

**3. Multiple merge bases, one hidden.** If the pair has two best common ancestors (a
criss-cross) and the shallow window contains only one of them, `git merge-base` returns the
visible one. That answer is still *a* common ancestor and produces a usable diff, but it is not
the answer `--all` would have given on full history, and a merge strategy that wanted to
recurse over all bases would behave differently. For diff purposes, one consistent base is
sufficient, which is the property Quinjet actually relies on.

### Why deepening is expensive by construction

Deepening retries are not incremental in the way one might hope. `--depth=N` is measured from
the remote tips, so deepening from 64 to 256 renegotiates and transfers the commits between
those depths; deepening again to 1,024, then 4,096, then 16,384 repeats the negotiation with a
bigger window each time. Each round is a full fetch dialogue: refs advertisement, negotiation,
pack transfer, index-pack. With `--filter=blob:none` the packs contain only commits and trees,
which keeps them small relative to a full fetch, but tree count grows with history depth, and
the rounds are strictly serial: the ladder cannot know it needs depth 4,096 until depth 1,024
has been fetched and the merge-base probe has failed against it.

The ladder therefore has the worst possible cost profile for the common case it was originally
serving: a deeply diverged PR pays every rung, in order, before the one that succeeds, and a PR
whose divergence exceeds the final rung pays every rung and then fails anyway. The session
notes record what that looked like before the optimization stack: the pre-#47 ladder was
`[64, 256, 1024, 4096]`, and past it the whole PR load hard-failed with "Unable to find the PR
merge base within 4,096 commits" after up to 8 progressively deeper fetches were wasted (both
refs re-fetched per rung). The synthesizer that mapped Quinjet's big-PR failure modes ranked
this as failure mode 3, noting "Long-lived rewrite branches on active repos routinely exceed
4,096 commits of divergence."

### The escape hatch: someone else's complete graph

The structural insight behind PR #47 is that the merge base is a *metadata question about the
full graph*, and the client is the only party that lacks the full graph. The server has it. So
ask the server: GitHub's compare API returns `merge_base_commit.sha` in one round trip, and the
client then needs history for exactly one purpose, possessing the merge-base *commit object and
its tree* so `git diff` can run against it. That is a depth-1 fetch of one OID, a few objects,
regardless of divergence. The deepening ladder survives only as the fallback for when the API
answer is unavailable or fails the honesty check, and its ceiling was raised from 4,096 to
16,384 so that the fallback fails less often on exactly the branch shapes that need it.

The next section walks that pipeline through the real code, decision by decision.

## Quinjet's merge-base pipeline

Everything above converges in one function: `Repository::prepare_pull_request_diff`
(`src/git/github/mod.rs:767-822`), the single entry point that turns PR metadata into a
workspace holding a resolved `(merge_base, head)` pair and a changed-file index. This section
follows it top to bottom with the merged code.

### The decision tree

The function's first move decides everything else. From `src/git/github/mod.rs:775-802`:

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

Two branches, two philosophies:

- **Both commits already local**: resolve the merge base *in the opened repository* with plain
  `git merge-base`, no network at all. The head is pinned to the metadata's `head_oid`.
- **Anything missing**: gather the two API hints first (merge base and per-file counts, both of
  which are pure metadata and cost one `gh` invocation each at most), then build a disposable
  bare workspace and enter `fetch_pull_request`, which owns all fetching.

The ordering inside the second branch is deliberate: the hints are fetched *before* the first
`git fetch` runs, so by the time the workspace fetches its first ref, the code already knows
whether the ladder can be skipped. Note also `api_counts = None` on the local branch: with all
blobs present locally, a local `git diff --numstat` is cheap and exact, so the API counts are
not needed there (the trade-off is analyzed in [api-strategy](../github/api-strategy.md)).

### The local fast path

The gate is `Repository::has_commit` (`src/git/mod.rs:790-799`):

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

Dissecting the small choices:

- `is_full_oid` requires a full 40- or 64-character hex OID before any subprocess runs, so an
  abbreviated or malformed value from metadata can never reach a git argv, and the probe is
  only ever asked about exact commits.
- `git cat-file -e` is the cheapest possible existence probe: it resolves the object through
  the normal lookup path (loose, packs, alternates, promisor availability aside) and produces
  *no stdout at all*, only an exit code. See
  [plumbing-and-porcelain](./plumbing-and-porcelain.md) for the catalog this belongs to.
- The `^{commit}` peel suffix insists the object both exists and is a commit (peeling a tag to
  a commit if needed), so a blob that happens to share a prefix or a mislabeled OID fails the
  gate instead of failing later inside `merge-base`.

When both probes pass, the resolver is `Repository::merge_base`
(`src/git/github/mod.rs:852-863`):

```rust
fn merge_base(&self, base: &str, head: &str) -> Result<String> {
    let output = self.checked([
        OsString::from("merge-base"),
        OsString::from(base),
        OsString::from(head),
    ])?;
    let merge_base = text(trim_ascii(&output));
    if merge_base.is_empty() {
        bail!("Git did not return a pull-request merge base");
    }
    Ok(merge_base)
}
```

This runs in the opened repository with the standard read environment (`GIT_OPTIONAL_LOCKS=0`,
`LC_ALL=C`, argv direct, no shell; see [refs-index-and-worktrees](./refs-index-and-worktrees.md)
for why the lock flag matters). In a normal clone this is where commit-graph generation numbers
quietly do their work: the walk is bounded, the answer returns in milliseconds, and the entire
pull-request preparation is network-free. The test
`locally_available_pr_objects_avoid_disposable_fetches` (`src/git/github/mod.rs:2946-2986`)
pins the property adversarially: it points the PR's repository URL at an unreachable host and
asserts that preparation and diffing still complete, in under 2 seconds, proving no network
path is exercised.

This is the path that serves PRs for locally built branches and for merged PRs whose commits
came down with a regular `git fetch`. It is invariant 9's opening sentence in ARCHITECTURE.md:
"PR patches first use immutable base/head OIDs already present in the opened repository, which
makes local-branch PR previews network-free."

### The compare API hint

On the disposable path, the first thing computed is the hint. `Repository::merge_base_from_api`
(`src/git/github/mod.rs:1288-1325`) is small enough to read whole, and every line of it is a
guard. Its doc comment states the thesis of this entire page in three lines
(`src/git/github/mod.rs:1285-1287`):

```rust
/// Ask the GitHub compare API for the merge base of the two immutable PR
/// commits. One metadata request replaces the deepening fetch ladder, which
/// cannot reach a merge base thousands of commits behind either tip.
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

Walking the guards in order:

**1. Input validation before anything runs.** Both OIDs must pass `is_commit_oid`
(`src/git/github/mod.rs:1945-1947`: length exactly 40 or 64, every byte ASCII hex, so SHA-1
and SHA-256 object formats both pass), and the repository must have a `name_with_owner`. A
malformed OID cannot reach the URL, and a repository resolved without identity (offline
inference failures) never generates a request that would 404.

**2. The cache key names the question completely.**
`pr-merge-base-v1\n{repo url}\n{base}\n{head}` embeds the repository (trailing slash
normalized away so `.../repo` and `.../repo/` share an entry), and both OIDs. Because the
merge base of two fixed commits is a pure function of immutable history, the entry is read
with `CacheLife::Immutable`: it can be evicted by the 128 MiB / 2,048-entry pruner but it can
never be wrong, which is the exact sense of "immutable" defined at
`src/git/github/mod.rs:219-221` and documented as invariant 12. Newline as the field separator
is safe because none of the components can contain one, and the key never touches the
filesystem as text: the store hashes it into a fixed 128-bit file name
(see [caching](../github/caching.md)).

**3. Even a cache hit is validated.** The cached bytes must still parse as a commit OID before
they are trusted. A corrupted or truncated entry degrades to a miss, never to a malformed
value flowing into a refspec.

**4. The network answer is triple-checked.** The `gh` process must exit zero, the stdout must
not have been pipe-truncated (the bounded runner kills any child that exceeds its cap, see
[api-strategy](../github/api-strategy.md)), and the projected `sha` must itself pass
`is_commit_oid`. Only then is it cached and returned.

**5. Every failure is `None`, never an error.** The function's return type is the design: an
`Option`, not a `Result`. API down, rate-limited, offline, enterprise host without the
endpoint, truncated response, garbage output: all collapse to `None`, and the caller treats
`None` as "no hint, use the ladder". The hint is an accelerator with a fallback, so its
failure must never be able to fail the pull request load.

The `--jq .merge_base_commit.sha` projection is also a bandwidth decision: the compare
response carries commit listings and file listings Quinjet has no use for, and `gh` applies
the jq program to the response so only the 40-or-64-byte SHA crosses the final pipe into the
2 MiB-capped reader.

### The depth-1 hint fetch

Knowing the merge base's OID is necessary but not sufficient: `git diff <mb> <head>` needs the
merge-base *commit object and its trees* present in the workspace. `fetch_pull_request`
(`src/git/github/mod.rs:1781-1864`) turns the hint into objects. By the time the hint matters,
the function has already added `origin` (the base repository URL) and fetched the head at depth
64, preferring GitHub's synthetic `+refs/pull/{n}/head:refs/quinjet/head` ref with a fork
fallback (the ref choreography is detailed in [pr-workspace](../github/pr-workspace.md)). Then
comes the short circuit, from `src/git/github/mod.rs:1834-1844`:

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

Reading it mechanism by mechanism:

**The refspec names a raw OID.** `+{hint}:refs/quinjet/merge-base` asks the server for a commit
by SHA, not by ref name. Protocol v2 permits a `want` for an unadvertised object only when the
server opts in through its `allow-reachable-sha1-in-want` family of capabilities; GitHub does,
which is the same server behavior that makes force-push-orphaned commits fetchable by SHA. The
fetched commit is bound to the fixed local name `refs/quinjet/merge-base`, keeping the
workspace's ref namespace fully synthetic (`refs/quinjet/*`) so nothing can collide with a real
branch name.

**Depth 1 is the whole point.** `--depth=1` (the `1` argument to `fetch_ref`) transfers the
hinted commit and stops: no parents, no history. Combined with `--filter=blob:none` the pack
contains the commit object and its trees, with blobs deferred to lazy promisor fetches that
often resolve from the alternates link into the opened repository instead
(`borrow_local_objects`, `src/git/github/mod.rs:1732-1745`). The transfer is proportional to
the size of one tree, not to the tens of thousands of commits of divergence the ladder would
have plowed through. The workspace's `.git/shallow` file afterwards contains the merge-base
commit itself: it is a graft point with no parents, and that is fine, because nothing will ever
walk below it.

**No local merge-base runs at all.** On this path the workspace never executes
`git merge-base`. It holds two islands of history (a depth-64 head window and a depth-1 base
commit) that likely do not even connect in the local graft graph, and a local walk would find
nothing. Correctness rests entirely on GitHub's graph having computed the base, which is why
the guard on the next line exists.

**The honesty check.** `preferred_fetched_commit` resolves what the head actually is in the
workspace, preferring the metadata's `head_oid` when it verifies locally. The hint is accepted
only when that resolved head is *exactly* the `head_oid` the metadata advertised, because that
is the head the compare API was asked about. If the PR branch was force-pushed between the
metadata read and the fetch, the fetched `refs/pull/{n}/head` now points at some new commit,
the old `head_oid` fails to verify, `preferred_fetched_commit` returns the fallback ref name
(which cannot equal a 40-hex OID), the equality fails, and the code falls through to the
ladder. The review finding that forced this guard is dissected in
[The stale hint after a force-push](#the-stale-hint-after-a-force-push).

**Failure is silent and safe.** `fetch_ref(...).is_ok()` means a server that refuses OID wants,
a network blip, or a garbage-collected hint commit simply skips the short circuit. Nothing is
reported; the ladder is the report.

The evidence comment posted on PR #47 summarized the happy path from the outside: "merge base
now comes from one compare-api call plus two depth-1 fetches instead of the deepening ladder"
(the head fetch is depth 64 in code; the comment abbreviates). The base *branch* is never
fetched at all on this path: `refs/quinjet/base` stays unborn, `FetchingBase` progress is never
emitted, and the base branch's ten-thousand-commit history stays on GitHub's disks where it
belongs.

### The fallback ladder

When there is no hint, or the hint fetch fails, or the honesty check rejects it, control falls
to the classic strategy, from `src/git/github/mod.rs:1846-1864`:

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

The structure maps directly onto the shallow-history failure shapes from earlier:

- **Rung 1 (depth 64)** reuses the head fetch already done and adds the base branch at depth
  64. If the true merge base is within 64 commits of both tips (shape 1), `try_merge_base`
  finds it immediately, and most freshly opened PRs end here with one small extra fetch.
- **Deepening rungs (256, 1,024, 4,096, 16,384)** re-fetch *both* refs at the new depth before
  probing again. Both sides must deepen because the merge base must be inside both windows;
  deepening only the base would never help a head whose branch carried hundreds of its own
  commits. The roughly 4x geometric step keeps the number of wasted rungs logarithmic in the
  divergence while bounding overshoot: a merge base 300 commits deep pays the 64 and 256 rungs
  before 1,024 finds it.
- **The probe tolerates absence.** `try_merge_base` (`src/git/github/mod.rs:1967-1979`) maps a
  non-zero exit or empty output to `Ok(None)`, "deepen further", exactly matching the
  exit-code semantics of `git merge-base` in a shallow repository:

```rust
fn try_merge_base(temporary: &Path, base: &str, head: &str) -> Result<Option<String>> {
    let args = [
        OsString::from("merge-base"),
        OsString::from(base),
        OsString::from(head),
    ];
    let output = run_temp_git(temporary, &args, 128 * 1024, 128 * 1024)?;
    if !output.status.success() {
        return Ok(None);
    }
    let merge_base = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
    Ok((!merge_base.is_empty()).then_some(merge_base))
}
```

- **The ceiling is a policy statement.** The bail message says why the loop must end: "refusing
  an unbounded history fetch". A merge-base search with no ceiling is an accidental full clone
  for exactly the repositories where that is most expensive. The ceiling was 4,096 before
  PR #47 and the ladder was `[64, 256, 1024, 4096]`; the pre-stack behavior hard-failed the
  entire PR load past it, after up to 8 progressively deeper fetches were wasted. PR #47 kept
  the ladder as fallback, extended it with the 16,384 rung, and updated the bail text, on the
  reasoning recorded in the session notes: the API hint normally makes the ladder unnecessary,
  and when the fallback does run, long-lived rewrite branches "routinely exceed 4,096 commits
  of divergence", so the old ceiling turned the fallback into a failure mode of its own.
  ARCHITECTURE.md invariant 5 now records the raised bound: "adaptive selected-PR history
  fetches at 16,384 commits".

Worth noticing about the ceiling's units: 16,384 is a *depth per side*, not a total. The final
rung holds up to 16,384 commits of the base branch and up to 16,384 of the head branch, so the
ladder can bridge a fork point up to that far behind either tip. It intentionally matches the
scale of `MAX_PR_PATHS = 16_384` (`src/git/github/mod.rs:38`), the changed-file index cap:
the bounded-everything discipline of invariant 5 applies to history exactly as it applies to
file listings and pipe reads.

### The fetch anatomy

Every fetch in both paths goes through one function, `fetch_ref`
(`src/git/github/mod.rs:1876-1909`), whose argument list is the transfer policy of this whole
page in eight lines:

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
    ...
}
```

Per flag, what it prevents:

| Flag | What it prevents |
|---|---|
| `--depth={depth}` | History transfer beyond the rung; every fetch is shallow, even the ladder's deepest |
| `--filter=blob:none` | File-content transfer during history search; only commits and trees move |
| `--no-tags` | Tag auto-following, which would drag unrelated release history into the shallow window |
| `--force` | Non-fast-forward refusal on `refs/quinjet/*` updates as depths and tips change |
| `--quiet` | Progress chatter in the 128 KiB-capped stdout |

The remainder of the function (elided above) retries the identical command *without*
`--filter=blob:none` when the first attempt fails, because a server that has not enabled
`uploadpack.allowFilter` rejects filtered fetches outright; shallowness is preserved either
way, so the worst case on a filterless server is transferring blobs for the shallow window,
never unbounded history. Both attempts run under the bounded child runner with stdout capped
at 128 KiB and stderr at `MAX_GH_ERROR_BYTES` (256 KiB), so even a misbehaving fetch cannot
balloon memory (see [plumbing-and-porcelain](./plumbing-and-porcelain.md) for the runner).

The interaction of the two headline flags deserves one more sentence, because it is the
transfer-cost equation of the entire design: `--depth` bounds the *commit and tree count* and
`--filter=blob:none` removes the *blob bytes*, so the cost of a rung is roughly
(commits within depth) x (commit + changed-tree overhead), and the cost of the depth-1 hint
fetch is one commit plus one root tree's spine. [packfiles-and-deltas](./packfiles-and-deltas.md)
quantifies why such packs are small; [shallow-and-partial-clone](./shallow-and-partial-clone.md)
covers the promisor bookkeeping that makes the deferred blobs lazily fetchable later.

### Pinning the endpoints

Both the honesty check and the ladder resolve commits through `preferred_fetched_commit`
(`src/git/github/mod.rs:1949-1965`):

```rust
fn preferred_fetched_commit(temporary: &Path, oid: &str, fallback: &str) -> Result<String> {
    if is_commit_oid(oid) {
        let args = [
            OsString::from("rev-parse"),
            OsString::from("--verify"),
            OsString::from(format!("{oid}^{{commit}}")),
        ];
        let output = run_temp_git(temporary, &args, 128 * 1024, 128 * 1024)?;
        if output.status.success() {
            let resolved = String::from_utf8_lossy(trim_ascii(&output.stdout)).into_owned();
            if !resolved.is_empty() {
                return Ok(resolved);
            }
        }
    }
    Ok(fallback.to_owned())
}
```

The function encodes a preference order: *the exact commit the metadata described, if it
arrived; otherwise whatever the fetched ref points at now*. `git rev-parse --verify
{oid}^{commit}` in the workspace succeeds only when the metadata OID is actually present
locally (the fetch of `refs/pull/{n}/head` brought it, because the branch has not moved), and
in that case the returned OID *is* the metadata OID, so the eventual `PreparedPullRequest`
diffs precisely the snapshot the user was shown in the metadata pane. When the branch moved
between metadata read and fetch, the old OID is absent, verification fails, and the fallback
ref name (`refs/quinjet/base` or `refs/quinjet/head`) is used, meaning the *ladder* path
diffs the branch as it is now, a coherent pair either way because both endpoints then come
from the same fetch epoch.

This is also the mechanism behind the hint honesty check reading so simply as
`head == pull_request.head_oid`: after `preferred_fetched_commit`, `head` is either the
verified metadata OID (strings equal, hint accepted) or a `refs/quinjet/...` name (strings
unequal, hint rejected). One string comparison carries the whole force-push defense.

### End of the pipeline: the index

Whichever path resolved the pair, `prepare_pull_request_diff` finishes identically
(`src/git/github/mod.rs:803-822`): it emits `EnumeratingFiles` progress and calls
`changed_files_in_repository(repository.path(), &merge_base, &head, api_counts)`, which runs

```text
git diff --name-status -z --find-renames <merge_base> <head> --
```

in whichever repository was prepared, parses the NUL-separated records into up to
`MAX_PR_PATHS = 16_384` entries, and caches the raw bytes immutably under
`pr-files-v1\n{merge_base}\n{head}` (only when the 8 MiB read was not truncated). The complete
parse, the truncation repair at record boundaries, and the counts attachment are documented in
[pipeline](../diff/pipeline.md); the point here is the hand-off: from this line on, the merge
base is no longer a question, it is a string threaded through every command and every cache key
the pull request will ever generate.

The resolved pair also travels in the returned handle itself, `PreparedPullRequest`
(`src/git/github/mod.rs:385-391`), whose `merge_base` and `head` fields feed
`patch_cache_key` for every later per-file and batched diff. The workspace lives as long as
the reader keeps the pull request open (session ownership is invariant 14) and the pair never
changes within a workspace generation: a head force-push detected by the poll produces a *new*
prepare with a *new* pair rather than mutating the old one, which is what keeps every cached
artifact internally consistent.

### The progress vocabulary

One small design detail ties the pipeline to the UI: each phase emits a fixed
`PullRequestProgress` variant (`src/git/github/mod.rs:237-269`) with a constant percentage and
label: `LoadingMetadata` 10% "Fetching pull-request metadata", `PreparingRepository` 20%
"Preparing an isolated diff workspace", `FetchingBase` 35% "Fetching the destination commit",
`FetchingHead` 50% "Fetching the source commit", `FindingMergeBase` 65% "Finding the merge
base", `EnumeratingFiles` 90% "Enumerating changed files". The variants are observable
documentation of the strategy: on the happy hint path the sequence a user sees is
`PreparingRepository`, `FetchingHead`, `FindingMergeBase`, `EnumeratingFiles`, and
`FetchingBase` never appears, because the base branch is never fetched. When `FetchingBase`
does appear, the ladder is running. The percentages are fixed rather than measured because the
phases' real durations vary by orders of magnitude across paths, and a monotone staircase
communicates progress without pretending to predict it.

## The stale hint after a force-push

The optimization stack that produced this pipeline went through an adversarial review
(twelve reviewer agents over the five original PRs), and the single most instructive finding
for this page is the one filed against the merge-base hint. It is a time-of-check to
time-of-use race, textbook in shape, and worth reconstructing completely because the fix is
two lines and the reasoning is the whole lesson.

### The review finding

Finding 3 of the review, severity MAJOR, against the mid-stack state of
`fetch_pull_request`: "Stale merge-base hint paired with freshly fetched head after
force-push." The mechanism, replayed against the code as it stood before the fix:

**1.** The metadata snapshot reads `base_oid = B`, `head_oid = H_old`.

**2.** `merge_base_from_api` asks the compare API about `B...H_old` and receives `M_old`, the
merge base of that pair. So far, everything is consistent.

**3.** Between the metadata read and the workspace fetch, the PR author force-pushes the
branch. `refs/pull/{n}/head` on GitHub now points at `H_new`, a commit that may be based on a
freshly rebased line with a completely different fork point.

**4.** The workspace fetches `+refs/pull/{n}/head:refs/quinjet/head` and receives `H_new`.
`H_old` is not fetched: it is no longer the ref tip.

**5.** The pre-fix code fetched the hint `M_old` at depth 1 and returned the pair
`(M_old, H_new)`: the merge base of the *old* head paired with the *new* head.

**6.** Everything downstream then describes a diff no one asked for. `git diff M_old H_new`
mixes the rebased branch's changes with whatever moved between the two fork points, the file
list is wrong, and, worst of all, the wrongness is *cached immutably*: the index bytes land
under `pr-files-v1\nM_old\nH_new`, a key that genuinely names the wrong-but-fixed question, so
the wrong answer would be served again on every reopen of that pair. Immutable caching is only
sound when the key-to-content function is correct; this bug produced a correctly keyed cache
entry for an incorrectly *chosen* key pair.

The severity classification came from that last property: transient races that produce a wrong
frame heal on the next poll, but a wrong immutable cache entry is a persistent lie for as long
as it survives eviction.

### The honesty check as shipped

The fix, landed on the `perf/pr-prefetch` branch (#47) during the review-fix round and quoted
in full in [The depth-1 hint fetch](#the-depth-1-hint-fetch), is the guard:

```rust
let head =
    preferred_fetched_commit(temporary, &pull_request.head_oid, "refs/quinjet/head")?;
if head == pull_request.head_oid {
    return Ok((hint.to_owned(), head));
}
```

The rule it encodes: *the hint was computed for the metadata's head, so the hint may only be
paired with the metadata's head*. After the fetch, `preferred_fetched_commit` tries to verify
`H_old` inside the workspace. In the race scenario `H_old` never arrived, verification fails,
the function returns the ref name, the equality is false, and the code falls through to the
ladder, which fetches base and head afresh and computes a merge base for the commits *as they
exist now*, both endpoints from the same fetch epoch. Consistency is restored not by detecting
the force-push explicitly but by refusing to combine values from two different snapshots.

Three properties make this small guard a good piece of engineering to imitate:

**1. It fails toward the slow-but-correct path.** The worst the race can now cause is a ladder
run that the hint would have skipped: a latency cost, never a correctness cost.

**2. It needs no clock, no version counter, no lock.** The immutability of OIDs supplies the
consistency check for free: equality of two hashes is equality of two snapshots. This is the
same property the cache keys lean on, applied to control flow.

**3. It closes the *pairing*, not just this pair.** Any future source of a merge-base hint
(a different API, a cached value from a previous session, which is in fact where most hints
come from once `pr-merge-base-v1` is warm) flows through the same guard, because the guard
checks the only thing that matters: that the head about to be used is the head the hint was
derived for. A cached hint for `(B, H_old)` read while the PR now sits at `H_new` is
harmless: the lookup key itself contains `H_old`, but even if metadata and fetch race again,
the guard re-verifies at use time.

### The sibling finding: counts keyed without the base

The same review round caught the same category of bug one endpoint over, and the contrast is
instructive. Finding 4, also MAJOR: the per-file counts cache
(`pull_request_file_counts_from_api`) was keyed by repository URL, PR number, and *head* OID
only, as `Immutable`. But per-file additions and deletions are properties of the three-dot
diff, which depends on the merge base, which depends on the *base* OID. Retarget the PR to
another base branch, or reset the base branch, and the head-only key would keep serving counts
computed against the old base, forever, marked immutable.

The fix bumped the key to `pr-file-counts-v3\n{url}\n{number}\n{base}\n{head}`
(`src/git/github/mod.rs:1248-1252`): both endpoints in the key, so a retarget asks a new
question. The version bump in the key name (`-v3`) is the migration strategy: old entries
under the old shape simply become unreachable garbage for the pruner, no migration code, no
deserialization hazard.

Together the two findings state one rule from both sides: **an immutable cache entry must be
keyed by every input that determines its content, and a value derived for one snapshot must
never be combined with data from another**. The merge-base hint violated the second clause;
the counts key violated the first. Both fixes are now invariants of the codebase (invariant 12
in ARCHITECTURE.md), and both are tested: `api_file_counts_parse_and_skip_malformed_records`
and the workspace tests exercise the keys, and the honesty check is exercised by the ladder
fallback tests in `src/git/github/mod.rs`.

## Caching on immutable history

### Why OID-keyed entries never expire

The object model page ([object-model](./object-model.md)) establishes the root fact: an OID is
a cryptographic hash of content, so the mapping from OID to content is fixed at creation. This
page adds the graph-level corollary: *any pure function of a set of OIDs is itself immutable*.
The merge base of `(B, H)` is such a function (of the DAG below two OIDs). The name-status
listing of `(M, H)` is such a function (of two trees). The numstat of `(M, H)`, the patch of
`(M, H, path)`, all pure functions of immutable inputs.

Quinjet's cache formalizes this with a two-variant enum, `CacheLife`
(`src/git/github/mod.rs:222-235`), whose doc comment defines the taxonomy: "`Immutable` is for
content whose identity is already in its key: a finished run's log, or a patch between two
fixed commits. Such an entry can never become wrong, only evicted." `Immutable.accepts(age)`
returns true unconditionally; `Ttl(d).accepts(age)` checks the clock. There is no invalidation
API at all, and none is needed for the immutable class: invalidation is what you do when a key
maps to changing content, and these keys cannot.

ARCHITECTURE.md invariant 12 states the consequence as user-facing behavior: "A new head or a
new comment therefore asks a different question rather than aging an old answer, so a stale
read is impossible and only eviction applies." Even the `--refresh` flag respects the split:
the session notes record the answer given to the user directly, `--refresh` "bypasses the
five-minute metadata TTL but keeps commit-keyed immutable entries ('those can never go
stale')", and the cache-through wrapper implements it (`checked_cached_gh` at
`src/git/github/mod.rs:1076-1162`: refresh cannot bypass an `Immutable` life).

### The key inventory downstream of the merge base

Every immutable entry a pull request produces embeds the resolved history endpoints. The
merge-base-adjacent slice of the full inventory (the complete table lives in
[caching](../github/caching.md)):

| Key template | Content | Life |
|---|---|---|
| `pr-merge-base-v1\n{repo url}\n{base}\n{head}` | The compare-API merge-base SHA | Immutable |
| `pr-file-counts-v3\n{repo url}\n{number}\n{base}\n{head}` | API per-file counts TSV | Immutable |
| `pr-files-v1\n{merge_base}\n{head}` | Raw NUL-separated name-status bytes | Immutable |
| `pr-numstat-v1\n{merge_base}\n{head}` | Raw `--numstat -z` bytes | Immutable |
| `pr-patch-v1\n{merge_base}\n{head}\n{path}` | One file's patch bytes (1 MiB cap) | Immutable |

Reading the table as a dependency graph exposes the structure: the first two keys are indexed
by the *metadata* pair `(base_oid, head_oid)`, because they cache answers about the question as
GitHub frames it; the last three are indexed by the *resolved* pair `(merge_base, head)`,
because they cache answers Git produced from resolved endpoints. The merge base is the joint:
`pr-merge-base-v1` is the entry that translates one index space into the other, and it is why a
warm reopen of a pull request costs zero ancestry work of any kind: the hint comes from disk,
the depth-1 fetch usually finds the objects already in the workspace-to-be or borrowable
through alternates, and every downstream read hits its own immutable entry.

Concretely, on a warm reopen of an unchanged PR the sequence of merge-base-related work is:
one cache read for the hint (hit), one depth-1 fetch (a no-op negotiation when the commit is
already present), one string comparison for the honesty check. The session's measured numbers
for the benchmark PR reflect this collapse and are quoted with context in
[Measured behavior on the benchmark PR](#measured-behavior-on-the-benchmark-pr).

### What force-pushes and retargets do to the key space

Because every key embeds the endpoints it depends on, history rewrites map to clean key-space
transitions with no invalidation logic anywhere:

- **New commits pushed to the PR branch.** `head_oid` changes; every `{head}`-bearing key
  changes; the old entries become unreachable and age out through the oldest-first pruner. The
  merge base usually does not move (the fork point is unchanged), so `pr-merge-base-v1` for
  the new pair is a fresh API question whose answer may equal the old one; either way it is
  keyed separately.
- **Force-push (rebase).** `head_oid` changes *and* the merge base typically moves forward to
  the new fork point. Both index spaces shift together. The stale-hint guard covers the one
  window where the two spaces could have been mixed.
- **Retarget to a different base branch.** `base_oid` changes; `pr-merge-base-v1` and
  `pr-file-counts-v3` change immediately (this is exactly what the counts-key review fix
  ensured); after re-resolution the `(merge_base, head)` keys change too.
- **Base branch advances normally.** `base_oid` in metadata moves with the branch tip, so the
  hint key changes and a new compare call is made, but the *resolved merge base* is generally
  identical (mainline movement does not move the fork point), so all `(merge_base, head)`
  entries, the expensive ones, remain valid and warm. This asymmetry is the quiet payoff of
  keying git-derived artifacts by the resolved pair rather than by the metadata pair: routine
  mainline churn costs one metadata request, not a cache flush.

### What the merge base feeds

The resolved pair is not only a cache key; it is the input to every downstream computation
this documentation covers elsewhere, which makes correct resolution a prerequisite for
everything else:

- The changed-file index and the file tree (`pr-files-v1`, invariant 10), built once per pair,
  is what the Files pane renders and what prefetch walks.
- Per-file counts (API or numstat) attach to index entries so headers render real `+n -n`
  before any patch exists (invariant 8a); the estimates derived from those counts,
  `(additions + deletions) * 80 + 4_096` bytes with a 512 KiB fallback for countless files
  (`estimated_patch_bytes`, `src/app.rs:7052-7060`), size the background batches.
- Background prefetch walks that index under the resolved pair's workspace: batches of up to
  32 files under a 6 MiB estimated-byte budget, anchored at the first file visible in the
  Files tree and wrapping around, up to 4,096 files total
  (`request_pull_request_prefetch`, `src/app.rs:5930-5977`). The ordering itself is an
  evolution artifact worth noting precisely: PR #50 introduced smallest-first size tiers
  (files sorted by estimated patch bytes when a PR crossed 100,000 changed lines or 1,000
  files), and PR #55 later removed those tiers in favor of the current viewport-anchored
  wrap-around fill, raising the prefetch ceiling from 400 to 4,096 files. Both generations of
  the policy consumed the same merge-base-keyed index; the scheduling story is told in
  [prefetch](../github/prefetch.md) and [progressive-loading](../rendering/progressive-loading.md).
- Every patch, single or batched, runs `git diff ... <merge_base> <head> -- <paths>` and lands
  in `pr-patch-v1` under the pair ([pipeline](../diff/pipeline.md)).

A single wrong merge base would therefore poison the index, the counts, the estimates, the
batches, and every cached patch, coherently and silently, which is why the honesty check and
the both-endpoints keying rule earned MAJOR severity in review despite being small diffs.

## Topological order in the history pane

The history pane is the other consumer of DAG structure in Quinjet, and it exercises a
different slice of the same theory: not "find one ancestor" but "present many commits in an
order humans can follow".

`git log` offers several orderings. The default is commit-date order with a twist (commits are
shown as discovered by a date-ordered walk), `--date-order` strictly sorts by committer date
while keeping parents after children, and `--topo-order` guarantees two properties: parents
never appear before their children, and commits on distinct lines of history are not
interleaved. For a branch-viewing UI the third property is the valuable one: date order happily
alternates between two parallel branches as their commit dates interleave, shredding each
branch's narrative, while topological order keeps each line contiguous. The cost is that
`--topo-order` cannot stream from a simple date-ordered frontier; Git must buffer enough of the
walk to commit to an order (generation numbers again bound that work in commit-graph-enabled
repositories).

Quinjet's history read (`Repository::history`, `src/git/mod.rs:330-357`) asks for exactly
that ordering, paged:

```text
git log --topo-order --decorate=short --no-color --skip=<n> --max-count=<limit> --format=<LOG_FORMAT> <revision> --
```

with `LOG_FORMAT` defined in `src/git/history.rs:22-23`:

```rust
pub(crate) const LOG_FORMAT: &str =
    "%H%x1f%h%x1f%P%x1f%aN%x1f%aE%x1f%aI%x1f%cN%x1f%cE%x1f%cI%x1f%ar%x1f%s%x1f%D%x1e";
```

Two graph-relevant details in that format: `%P` emits the full parent OID list, so the parsed
`Commit` model (`src/git/history.rs`) carries the DAG's edges and a commit's merge-ness
(parent count > 1) without any extra command; and the 0x1f/0x1e unit/record separators make
the parse immune to any bytes a subject line could contain, part of the delimiter discipline
described in [plumbing-and-porcelain](./plumbing-and-porcelain.md). Pagination via
`--skip`/`--max-count` in pages of 300 (`DEFAULT_HISTORY_PAGE`, `src/git/mod.rs:25-29` region)
keeps even a million-commit history from ever being materialized at once, the same
bound-everything stance the merge-base ladder takes toward fetching. The `<revision>` argument
is validated to a fixed shape first (`HEAD`, `refs/heads/*`, `refs/remotes/*`, `refs/tags/*`,
or a full OID, guard at `src/git/mod.rs:331-338`), so the pane can never be steered into an
arbitrary rev-list expression.

## Design alternatives and why they lost

The shipped pipeline is one point in a large design space. The session that built it, and the
review that hardened it, considered or implicitly rejected each of the following. Recording the
losers is as useful as documenting the winner, because most of them look reasonable until one
specific property of the problem eliminates them.

### Unbounded deepening

The simplest correct fallback: keep deepening until `git merge-base` succeeds, no ceiling.
Rejected because the failure mode is invisible resource exhaustion: a PR against a
gigantic repository with an ancient fork point (or, worse, a base pair that genuinely shares
no history) would fetch the entire commit-and-tree history of both branches before concluding
anything, from inside a background worker, with the user watching a progress label. The
explicit bail, "refusing an unbounded history fetch", converts that into a bounded, explained
failure. The ceiling's *value* is a tuning judgment (4,096 proved too small in practice,
16,384 is the current judgment); the ceiling's *existence* is a principle (invariant 5 bounds
every read this codebase performs, and history fetches are reads).

### Full clone or unshallow into the workspace

Cloning the repository outright (or running `git fetch --unshallow` on first failure) makes
every ancestry question trivially answerable and every diff local. Rejected on transfer cost
and latency: the disposable workspace exists precisely because PR viewing must not cost a
repository download, and the benchmark repository makes the point concrete: the shallow
`blob:none` clone of bun used for testing already weighs 389 MB on disk *with* both filters
applied; a full clone is far beyond an acceptable price for opening one pull request. The
workspace's whole design (bare, shallow, blob-filtered, alternates-linked, deleted on drop) is
the negation of this alternative, and [pr-workspace](../github/pr-workspace.md) documents it.

### Always fetch the base branch and compute locally

The pre-#47 design: no API involvement, fetch both refs, deepen until the base is found.
Correct, self-contained, offline-friendly, and quadratically wasteful on exactly the PRs that
matter: the deepening ladder re-fetches both sides at each rung, cannot know the required
depth in advance, and for a May-to-August rewrite branch the required depth is thousands of
commits of mainline movement. It survives as the fallback because its virtues (no dependency
on GitHub's graph, no trust in a second system) are real, and a fallback is exactly where
those virtues belong: the expensive path runs only when the cheap path cannot.

### Trust the hint without the honesty check

Use `merge_base_commit.sha` unconditionally: one fewer subprocess, simpler code. Rejected by
the adversarial review's finding 3 (the force-push race documented above). The general
principle that killed it: a value computed from snapshot A must not be combined with data
fetched at snapshot B without proof the snapshots coincide. The proof here is one string
equality on an immutable OID, about as cheap as consistency ever gets, which is what made
shipping the unguarded version an error rather than a trade-off.

### An in-process graph library

Linking libgit2 or gitoxide would allow computing merge bases in-process, no subprocess, no
parsing, and even custom traversals (a bounded walk that reports "not within N commits"
directly). Quinjet's architecture rejects in-process Git wholesale: the project treats the
`git` binary as the single authority on repository semantics (the stance is documented across
[plumbing-and-porcelain](./plumbing-and-porcelain.md) and ARCHITECTURE.md), which buys exact
behavioral parity with the user's own Git, zero native-dependency surface, and freedom from
tracking upstream semantics in a second implementation. Merge-base computation is not close to
being the bottleneck that would justify breaking that rule: the expensive part was never the
walk, it was the transfer, and an in-process library still has to fetch the same history
before it can walk it.

### Ask GitHub for the whole answer

GitHub's compare response and the `pulls/{n}/files` endpoint carry file listings, patches, and
counts; the REST API can also serve a complete `.diff` for a pull request. Why fetch objects
and run local `git diff` at all? Three reasons, each load-bearing:

- **Bounded reads.** A single API-served mega-diff is one giant response against Quinjet's
  2 MiB metadata cap and pagination machinery; the local route reads patches in bounded,
  path-scoped, per-batch invocations that the mailbox architecture can schedule and cancel
  (see [prefetch](../github/prefetch.md)).
- **Fidelity.** Local `git diff` output feeds a parser that Quinjet fully controls, with
  `--find-renames`, configurable context, byte-exact paths via `-z` and `core.quotepath=false`,
  and identical behavior between the local-repository path and the workspace path. API-served
  patch text has its own truncation rules for large files and its own rename presentation.
- **The network-free fast path.** For locally present commits, the local route costs nothing;
  an API-first design would spend requests (and rate limit) on data already on disk. The
  hybrid keeps API usage where the API is uniquely cheap (graph metadata, per-file counts) and
  Git usage where Git is uniquely cheap (tree diffs over local objects). That split, "prefer
  API metadata, prefer local content", recurs across the stack and is cataloged in
  [techniques](../techniques.md).

### Merge-base by ref names instead of OIDs

Asking the compare API about `main...feature` and caching under the ref names would work until
either branch moved, and then the cache would be wrong with no way to notice: ref names are
mutable pointers, so a name-keyed entry needs a TTL and can still serve a stale answer inside
its window. Keying by OIDs makes every entry immutable and every branch movement a new key.
This is the same reasoning that keys the check list by `head_oid`
(`checks-v1\n{repo}\n{number}\n{head_oid}`), so a force-push instantly invalidates check state
by changing the question ([caching](../github/caching.md)).

### The fork-point shortcut

`git merge-base --fork-point`, discussed in the theory section, was never a candidate for the
workspace (no reflog exists there), but it is worth restating as a rejected alternative for
the *local* fast path too: it can return an answer that differs from the pure DAG merge base,
it depends on reflog expiry, and it would make the local path and the API path disagree about
the same pull request. Both paths implementing the same pure function is what lets the rest of
the system not care which one ran.

## Failure modes and edge cases

The pipeline's branches multiply into a considerable catalog of edge cases. Each entry below
names the trigger, the observed behavior in the merged code, and where the behavior comes
from.

### The base pair shares no history

Two genuinely unrelated histories (a PR from a repository bootstrapped independently, or an
orphan branch) have no common ancestor at any depth. The compare API cannot name a merge base
for them; locally, `git merge-base` exits 1 at every rung. The ladder runs to exhaustion and
fails with the 16,384-commit message. This is the correct outcome: a three-dot diff is
undefined without a base, and an error naming the bound is more honest than inventing an empty
tree as a base (which would present the entire repository as added). The same terminal state
covers the "merge base deeper than 16,384" case; the message does not distinguish them because
the local evidence cannot.

### The opened repository is itself shallow

The local fast path guards on *presence* (`has_commit` for both OIDs) but not on
*connectedness*. In a shallow clone that happens to contain both commits (for example the
benchmark clone at `/tmp/bun-test`, where the merged PR's head is reachable from shallow
`main`), local `git merge-base` succeeds only if a common ancestor lies inside the shallow
window. If it does not, `Repository::merge_base` receives a non-zero exit, `checked` bails,
and `prepare_pull_request_diff` propagates the error: the fast path does *not* fall back to
the disposable workspace, because its gate already concluded the repository could answer.
The session's benchmark notes record the benign version of this edge: the baseline build
"also succeeded on this PR because bun#30412 is merged, so its head is reachable in main's
shallow history", meaning presence and connectedness coincided there. The sharp version
(present but not connected) is a real limitation of the gate worth knowing when diagnosing a
failed load in a shallow clone: deepening the clone or fetching the PR ref locally resolves
it.

### Both commits local but via a squash merge: neither, actually

The most instructive real-world case from the session, because it looks like the previous one
and is the opposite. A user ran Quinjet against a *full* local bun clone and still saw
slow per-file loads, asking why anything was fetching when "everything is local". The
diagnosis, verified with hard data in the session: bun squash-merged the rewrite PR, so the
squash commit on `main` contains the PR's *content*, but the PR's actual head commit
`ed1a70f8` exists only under GitHub's `refs/pull/30412/head` and was never part of any local
ref. `has_commit(head_oid)` fails, the gate correctly routes to the disposable workspace, and
every expanded file becomes a lazy blob fetch. Squash and rebase merges *always* produce this
shape: the merged PR's head OID is unreachable from the target branch because merging rewrote
the changes into new commits.

Two mitigations exist, one manual and one automatic:

- The one-time manual fix given in the session:
  `git fetch origin +refs/pull/30412/head:refs/remotes/origin/pr-30412` run in the user's own
  clone, after which both OIDs verify locally and the network-free path applies (the session
  ran it; output: `* [new ref] refs/pull/30412/head -> origin/pr-30412`). Quinjet never runs
  this itself: fetching into the opened repository would violate the no-mutation guarantee of
  invariant 9.
- The automatic mitigation shipped in PR #55: `borrow_local_objects` writes the opened
  repository's objects directory into the workspace's `objects/info/alternates`, so even on
  the workspace path, blobs that the squash merge did bring into the local clone resolve from
  local disk instead of the network. The head *commit* still has to come from GitHub; its
  file *contents* mostly do not.

### Force-push between metadata and fetch

Covered in depth above: the hint honesty check rejects the pairing, the ladder recomputes both
endpoints from the same fetch epoch, and `preferred_fetched_commit` decides per endpoint
whether the metadata snapshot or the current ref tip is the operative commit. A related
polling-time behavior closes the loop after preparation: invariant 11 specifies that when a
poll observes a changed head OID, only the diff is reindexed, which produces a fresh prepare
and a fresh key space rather than mutating the existing workspace.

### The fork is gone

A cross-repository PR whose head fork was deleted can still be viewable when GitHub retains
`refs/pull/{n}/head` on the base repository, which is why that synthetic ref is always the
first fetch attempt. When that ref also fails and metadata says the fork is gone
(`head_repository` is `None`), the failure is contextualized precisely: "the base repository
no longer exposes the PR head and its fork was deleted" (`src/git/github/mod.rs:1806-1811`).
When the fork exists, the fallback fetches `+refs/heads/{head_ref}:refs/quinjet/head` from a
second remote named `head`, and the ladder's deepening rungs remember which remote and refspec
won so they deepen the right side.

### The server rejects filters or OID wants

Two independent server capabilities gate the optimized fetch shapes, and both degrade
gracefully. A server without `uploadpack.allowFilter` fails the `--filter=blob:none` fetch;
`fetch_ref` retries the identical command unfiltered, so the fetch stays shallow but carries
blobs. A server that refuses raw-OID wants fails the depth-1 hint fetch; the `is_ok()` guard
swallows it and the ladder proceeds by ref names only. GitHub itself exhibits neither
restriction, but the workspace also talks to GitHub Enterprise hosts, where server
configuration varies.

### The API lies, is down, or is rate-limited

`merge_base_from_api` funnels every network misfortune into `None`, so the ladder absorbs API
unavailability at the cost of latency. A subtler case: the API *returns* a SHA that is wrong
for the asked pair (a bug, a mid-flight repository swap). The workspace has no way to verify
an arbitrary claimed merge base cheaply (that verification would itself require the history
the design avoids fetching), so the depth-1 fetch trusts the answer's *identity* while the
`is_commit_oid` guards verify only its *shape*. The blast radius is bounded by the honesty
check (the pairing with head is still snapshot-consistent) and by key hygiene (a wrong answer
for `(B, H)` is cached only under `(B, H)`; asking about any other pair is unaffected). The
practical risk is accepted as negligible: GitHub's merge-base computation is the same one its
own UI and merge machinery depend on.

### Truncated subprocess output at the boundaries

Every read in the pipeline is capped, and each cap has a defined truncation behavior rather
than an implicit one: the compare-API read rejects a pipe-truncated SHA outright
(`output.stdout_truncated` returns `None`); `preferred_fetched_commit` and `try_merge_base`
read through 128 KiB caps that a 40-byte OID cannot approach; fetch stdout is diagnostics
only, capped at 128 KiB with stderr at 256 KiB; and the downstream name-status read repairs a
truncated stream to whole NUL-terminated records before parsing
(`src/git/github/mod.rs:2019-2030`). The bounded-pipe substrate itself (kill the child on cap
overflow) is shared with everything else in the codebase and documented in
[plumbing-and-porcelain](./plumbing-and-porcelain.md).

### Missing or malformed metadata refs

`fetch_pull_request` refuses to start when metadata lacks base or head ref names ("Pull
request metadata does not contain complete base/head refs",
`src/git/github/mod.rs:1787-1789`), because the refspecs are built from them. Malformed OIDs
never reach a refspec at all: `is_commit_oid` gates the hint, `is_full_oid` gates the local
probes, and `preferred_fetched_commit` only interpolates an OID it has validated. User text
cannot steer these argv values; the only strings that reach a fetch refspec are a validated
hex OID, a PR number formatted by the code itself, and ref names GitHub's own metadata
supplied.

### Criss-cross pairs across the two resolvers

When a pair has multiple merge bases, the local fast path and the API path each return one
representative, and nothing guarantees they would pick the *same* one. This never produces an
inconsistency in practice because the two resolvers never run for the same preparation: one
preparation resolves once, and every artifact of that preparation (index, counts, patches,
cache entries) hangs off the single OID that resolution produced. Two different sessions
resolving the same pair through different paths could cache artifacts under two different
merge-base OIDs; both sets are internally consistent, both diffs are legitimate three-dot
diffs by the definition, and the duplicate entries cost only cache space.

### Workspace death and rebirth

The merge base's lifetime is the workspace's lifetime. `PreparedPullRequest` is dropped when
the reader leaves the pull request, deleting the bare repository and its shallow, grafted
history; the resolved OID string survives only inside cache keys. Reopening the PR replays
preparation from the hints: cached merge base, cached counts, cached index bytes, and a
workspace whose fetches mostly no-op. Crashed sessions leak workspaces at most 24 hours;
`remove_stale_temporary_repositories` sweeps `pr-*.git` directories older than
`TEMPORARY_REPOSITORY_MAX_AGE` on the next workspace creation
(`src/git/github/mod.rs:1754-1779`). None of the sweeping touches cache entries: history
answers outlive the repositories that computed them, which is the entire point of keying them
by what they are rather than where they came from.

## Measured behavior on the benchmark PR

The optimization stack was benchmarked throughout against one deliberately hostile target:
oven-sh/bun#30412, "Rewrite Bun in Rust", 2,188 changed files, +1,009,257 added lines, a PR
opened May 9 and updated until Aug 11 against a repository that kept moving the whole time.
Every number below is quoted from the session records with its context; the full methodology
(the shallow `blob:none` clone at `/tmp/bun-test`, cold-cache isolation via
`QUINJET_CACHE_DIR=$(mktemp -d)`) lives in [benchmarking](../benchmarking.md).

The merge-base dimension of the benchmark is the divergence: with the PR branch forked in May
and mainline advancing until August, the fork point sat far behind the base tip, exactly the
shape that made the old 4,096-commit ladder both slow and terminal. The PR #47 evidence
comment describes the after state: "loading the 1M-line bun rewrite pr (oven-sh/bun#30412,
2188 files) from a shallow blob:none clone: merge base now comes from one compare-api call
plus two depth-1 fetches instead of the deepening ladder".

From the first verification round (top of the five-PR stack, cold cache):

- "Metadata in 1.7s" (`quinjet pr view 30412`, cold).
- "The rewrite PR enumerates all 2,188 files with real counts in 18.5s cold." (`pr files`,
  cold cache, includes workspace preparation and therefore the merge-base resolution).
- Warm re-run of the index: 0.04s.
- Single-file patches: 0.1s.

From the second verification round, after the review fixes (including the merge-base honesty
check and the counts-key fix) and the restack: "Final numbers on the bun PR: cold index 6.3s,
warm 0.04s, conversation 26s with the honest truncation notice." The PR #47 evidence comment
carries the same run as a terminal transcript: `time quinjet pr files 30412 | wc -l` printing
`2188`, `real 0m6.30s` cold, `real 0m0.04s` warm, and `time quinjet pr diff 30412
.buildkite/ci.mjs` at `real 0m0.10s`.

Reading the two cold numbers against this page's machinery: the 18.5 s and 6.3 s runs both
already used the compare-API merge base (that landed in #47, below both measurements); the
improvement between rounds came with the review-fix round, which among other things rebased
the stack and included the counts-cache key fix. The warm 0.04 s is the immutable key space
doing its job end to end: hint, counts, index, and numstat all served from OID-keyed entries,
no ancestry work, no fetch negotiation of consequence. And after the top-of-stack build was
installed locally, the session recorded the steady-state feel of the same command with real
caches: "Smoke-tested from the bun clone: `q pr files 30412` lists all 2,188 files of the
1M-line rewrite PR in 1.4s." (warm metadata, real cache).

The merge base is one commit out of the roughly one million lines this PR moves, and finding
it well is invisible when it works: no `FetchingBase` phase on screen, no deepening stalls, an
index that opens while the conversation is still loading. The rest of the machinery that turns
the resolved pair into pixels is the subject of [pipeline](../diff/pipeline.md),
[prefetch](../github/prefetch.md), and
[progressive-loading](../rendering/progressive-loading.md); the sibling pages in this group,
starting from [the group hub](./README.md), cover the object store, packfiles, and fetch
protocol underneath everything this page assumed.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Merge Bases and History in a small local repository | Record time to first useful rows |
| 2 | Check latency for Merge Bases and History in a small local repository | Record steady frame cost |
| 3 | Check latency for Merge Bases and History in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Merge Bases and History in a small local repository | Record Git and gh process count |
| 5 | Check latency for Merge Bases and History in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Merge Bases and History in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Merge Bases and History in a small local repository | Record stale reply rejection |
| 8 | Check latency for Merge Bases and History in a small local repository | Record visible state after failure |
| 9 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Merge Bases and History in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Merge Bases and History in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Merge Bases and History in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Merge Bases and History in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Merge Bases and History in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Merge Bases and History in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Merge Bases and History in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Merge Bases and History in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Merge Bases and History in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Merge Bases and History in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for Merge Bases and History in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for Merge Bases and History in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for Merge Bases and History in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for Merge Bases and History in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for Merge Bases and History in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for Merge Bases and History in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for Merge Bases and History in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for Merge Bases and History in an unavailable network | Record time to first useful rows |
| 34 | Check latency for Merge Bases and History in an unavailable network | Record steady frame cost |
| 35 | Check latency for Merge Bases and History in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for Merge Bases and History in an unavailable network | Record Git and gh process count |
| 37 | Check latency for Merge Bases and History in an unavailable network | Record maximum retained document bytes |
| 38 | Check latency for Merge Bases and History in an unavailable network | Record cache disposition and complete key |
| 39 | Check latency for Merge Bases and History in an unavailable network | Record stale reply rejection |
| 40 | Check latency for Merge Bases and History in an unavailable network | Record visible state after failure |
| 41 | Check latency for Merge Bases and History in rapid keyboard navigation | Record time to first useful rows |
| 42 | Check latency for Merge Bases and History in rapid keyboard navigation | Record steady frame cost |
| 43 | Check latency for Merge Bases and History in rapid keyboard navigation | Record bytes accepted from child output |
| 44 | Check latency for Merge Bases and History in rapid keyboard navigation | Record Git and gh process count |
| 45 | Check latency for Merge Bases and History in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Check latency for Merge Bases and History in rapid keyboard navigation | Record cache disposition and complete key |
| 47 | Check latency for Merge Bases and History in rapid keyboard navigation | Record stale reply rejection |
| 48 | Check latency for Merge Bases and History in rapid keyboard navigation | Record visible state after failure |
| 49 | Check latency for Merge Bases and History in a linked worktree | Record time to first useful rows |
| 50 | Check latency for Merge Bases and History in a linked worktree | Record steady frame cost |
| 51 | Check latency for Merge Bases and History in a linked worktree | Record bytes accepted from child output |
| 52 | Check latency for Merge Bases and History in a linked worktree | Record Git and gh process count |
| 53 | Check latency for Merge Bases and History in a linked worktree | Record maximum retained document bytes |
| 54 | Check latency for Merge Bases and History in a linked worktree | Record cache disposition and complete key |
| 55 | Check latency for Merge Bases and History in a linked worktree | Record stale reply rejection |
| 56 | Check latency for Merge Bases and History in a linked worktree | Record visible state after failure |
| 57 | Check latency for Merge Bases and History in cold and warm cache states | Record time to first useful rows |
| 58 | Check latency for Merge Bases and History in cold and warm cache states | Record steady frame cost |
| 59 | Check latency for Merge Bases and History in cold and warm cache states | Record bytes accepted from child output |
| 60 | Check latency for Merge Bases and History in cold and warm cache states | Record Git and gh process count |
| 61 | Check latency for Merge Bases and History in cold and warm cache states | Record maximum retained document bytes |
| 62 | Check latency for Merge Bases and History in cold and warm cache states | Record cache disposition and complete key |
| 63 | Check latency for Merge Bases and History in cold and warm cache states | Record stale reply rejection |
| 64 | Check latency for Merge Bases and History in cold and warm cache states | Record visible state after failure |
| 65 | Check peak memory for Merge Bases and History in a small local repository | Record time to first useful rows |
| 66 | Check peak memory for Merge Bases and History in a small local repository | Record steady frame cost |
| 67 | Check peak memory for Merge Bases and History in a small local repository | Record bytes accepted from child output |
| 68 | Check peak memory for Merge Bases and History in a small local repository | Record Git and gh process count |
| 69 | Check peak memory for Merge Bases and History in a small local repository | Record maximum retained document bytes |
| 70 | Check peak memory for Merge Bases and History in a small local repository | Record cache disposition and complete key |
| 71 | Check peak memory for Merge Bases and History in a small local repository | Record stale reply rejection |
| 72 | Check peak memory for Merge Bases and History in a small local repository | Record visible state after failure |
| 73 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record steady frame cost |
| 75 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record bytes accepted from child output |
| 76 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record Git and gh process count |
| 77 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record cache disposition and complete key |
| 79 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record stale reply rejection |
| 80 | Check peak memory for Merge Bases and History in a monorepo with many changed paths | Record visible state after failure |
| 81 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record time to first useful rows |
| 82 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record steady frame cost |
| 83 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record bytes accepted from child output |
| 84 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record Git and gh process count |
| 85 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record maximum retained document bytes |
| 86 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record cache disposition and complete key |
| 87 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record stale reply rejection |
| 88 | Check peak memory for Merge Bases and History in a pull request containing generated files | Record visible state after failure |
| 89 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record time to first useful rows |
| 90 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record steady frame cost |
| 91 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record bytes accepted from child output |
| 92 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record Git and gh process count |
| 93 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record cache disposition and complete key |
| 95 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record stale reply rejection |
| 96 | Check peak memory for Merge Bases and History in a deeply diverged branch | Record visible state after failure |
| 97 | Check peak memory for Merge Bases and History in an unavailable network | Record time to first useful rows |
| 98 | Check peak memory for Merge Bases and History in an unavailable network | Record steady frame cost |
| 99 | Check peak memory for Merge Bases and History in an unavailable network | Record bytes accepted from child output |
| 100 | Check peak memory for Merge Bases and History in an unavailable network | Record Git and gh process count |
| 101 | Check peak memory for Merge Bases and History in an unavailable network | Record maximum retained document bytes |
| 102 | Check peak memory for Merge Bases and History in an unavailable network | Record cache disposition and complete key |
| 103 | Check peak memory for Merge Bases and History in an unavailable network | Record stale reply rejection |
| 104 | Check peak memory for Merge Bases and History in an unavailable network | Record visible state after failure |
| 105 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record time to first useful rows |
| 106 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record steady frame cost |
| 107 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record bytes accepted from child output |
| 108 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record Git and gh process count |
| 109 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record cache disposition and complete key |
| 111 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record stale reply rejection |
| 112 | Check peak memory for Merge Bases and History in rapid keyboard navigation | Record visible state after failure |
| 113 | Check peak memory for Merge Bases and History in a linked worktree | Record time to first useful rows |
| 114 | Check peak memory for Merge Bases and History in a linked worktree | Record steady frame cost |
| 115 | Check peak memory for Merge Bases and History in a linked worktree | Record bytes accepted from child output |
| 116 | Check peak memory for Merge Bases and History in a linked worktree | Record Git and gh process count |
| 117 | Check peak memory for Merge Bases and History in a linked worktree | Record maximum retained document bytes |
| 118 | Check peak memory for Merge Bases and History in a linked worktree | Record cache disposition and complete key |
| 119 | Check peak memory for Merge Bases and History in a linked worktree | Record stale reply rejection |
| 120 | Check peak memory for Merge Bases and History in a linked worktree | Record visible state after failure |
| 121 | Check peak memory for Merge Bases and History in cold and warm cache states | Record time to first useful rows |
| 122 | Check peak memory for Merge Bases and History in cold and warm cache states | Record steady frame cost |
| 123 | Check peak memory for Merge Bases and History in cold and warm cache states | Record bytes accepted from child output |
| 124 | Check peak memory for Merge Bases and History in cold and warm cache states | Record Git and gh process count |
| 125 | Check peak memory for Merge Bases and History in cold and warm cache states | Record maximum retained document bytes |
| 126 | Check peak memory for Merge Bases and History in cold and warm cache states | Record cache disposition and complete key |
| 127 | Check peak memory for Merge Bases and History in cold and warm cache states | Record stale reply rejection |
| 128 | Check peak memory for Merge Bases and History in cold and warm cache states | Record visible state after failure |
