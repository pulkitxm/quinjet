# Diff Algorithms

This page is the theory half of the diff group: what it means to compute a difference between two
sequences, how the Myers algorithm and its refinements actually work, what patience and histogram
diff change, and where heuristics such as hunk sliding and the indent heuristic come from. It then
pairs every piece of that theory with the way Quinjet exploits it: Quinjet deliberately does not
reimplement any of these algorithms, it treats the `git` binary as the single authority for hunk
computation, and the one diff-shaped computation it performs itself, the intraline emphasis
pairing, is chosen precisely because it is not a general diff algorithm. The companion pages
[./pipeline.md](./pipeline.md) and
[./intraline-and-highlighting.md](./intraline-and-highlighting.md) cover the patch format, the
document model, and the highlighting budgets in depth.

## Contents

- [Diff as a shortest edit script](#diff-as-a-shortest-edit-script)
- [The edit graph](#the-edit-graph)
- [The Myers algorithm](#the-myers-algorithm)
- [Linear-space refinement](#linear-space-refinement)
- [Patience diff](#patience-diff)
- [Histogram diff](#histogram-diff)
- [Minimal diffs and the default heuristics](#minimal-diffs-and-the-default-heuristics)
- [The slider problem and the indent heuristic](#the-slider-problem-and-the-indent-heuristic)
- [Rename and copy detection](#rename-and-copy-detection)
- [Word-level diffing](#word-level-diffing)
- [Binary diffs and deltas](#binary-diffs-and-deltas)
- [Quinjet: Git as the hunk authority](#quinjet-git-as-the-hunk-authority)
- [The flag set on every patch read](#the-flag-set-on-every-patch-read)
- [Context radius as a rendering mode](#context-radius-as-a-rendering-mode)
- [Rename detection in the Quinjet pipeline](#rename-detection-in-the-quinjet-pipeline)
- [Quinjet's own diff-shaped computation](#quinjets-own-diff-shaped-computation)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Where to go next](#where-to-go-next)

## Diff as a shortest edit script

### The problem statement

A diff answers one question: given an old sequence `A` of length `N` and a new sequence `B` of
length `M`, what is the cheapest series of edits that turns `A` into `B`? For text diffs the
elements of the sequences are whole lines, and the allowed edits are exactly two:

- delete an element of `A`, and
- insert an element of `B`.

There is no substitution edit. A changed line is modeled as a deletion of the old line plus an
insertion of the new line. This restriction is not a simplification for its own sake; it is what
makes the output a *patch*. A patch consumes the old file top to bottom, copying unchanged lines
and interleaving deletions and insertions, and a two-operation edit model maps one-to-one onto
the `-` and `+` prefixed lines of the unified format. Tools that support substitution (such as
generic edit-distance computations with replacement cost 1) produce answers that cannot be
serialized into that format without first re-expanding each substitution into a delete plus an
insert.

The number of edits in a script is its length, and the shortest edit script (SES) is the one with
the fewest edits. Because every element of `A` that is not deleted must appear in `B`, and every
element of `B` that is not inserted must appear in `A`, the elements untouched by the script form
a common subsequence of `A` and `B`, in order. Minimizing the number of edits is therefore the
same problem as maximizing the number of untouched elements.

### Edit distance and the longest common subsequence

Let `L` be the length of the longest common subsequence (LCS) of `A` and `B`. Every edit script
must delete the `N - L` elements of `A` outside some common subsequence and insert the `M - L`
elements of `B` outside it, so the SES length `D` satisfies:

```text
D = N + M - 2L
```

The two views are interchangeable: an algorithm that finds the LCS finds the SES and vice versa.
The classic dynamic-programming solution fills an `(N + 1) x (M + 1)` table where cell `(x, y)`
holds the LCS length of the prefixes `A[1..x]` and `B[1..y]`:

```text
lcs(x, y) = 0                              if x == 0 or y == 0
lcs(x, y) = lcs(x - 1, y - 1) + 1          if A[x] == B[y]
lcs(x, y) = max(lcs(x - 1, y),
                lcs(x, y - 1))             otherwise
```

That table costs `O(N * M)` time and space. For two 10,000-line files that is one hundred million
cells, which is why no production diff tool uses the plain DP table. The insight behind the Myers
algorithm is that real diffs are usually small: `D` is tiny compared to `N + M`, and an algorithm
whose cost scales with `D` rather than with `N * M` is dramatically cheaper on the inputs that
actually occur. Two mostly identical files diff in near-linear time; only pathological inputs
(two unrelated files) approach the quadratic worst case.

### Why diffs operate on lines

Nothing in the theory requires the sequence elements to be lines. The same algorithms run over
characters, words, or tokens. Line granularity won for source code for three practical reasons:

1. Lines are the unit programmers edit and review. A hunk of changed lines corresponds to how
   the change was made and how it will be read.
2. Line granularity shrinks the input. A 100 KiB source file is a few thousand lines; running an
   `O((N + M) * D)` algorithm over a few thousand elements is far cheaper than over a hundred
   thousand characters.
3. Equality of whole lines can be reduced to equality of integers. Each distinct line is hashed
   once and the diff runs over sequences of hashes, so the inner comparison of the algorithm is
   an integer compare, not a string compare.

The hashing step matters more than it looks. Git's diff engine (the `xdiff` library) first
classifies every line of both files: it hashes each line, assigns each distinct line content a
small integer identity, and records occurrence counts per side. The algorithm proper then works
on arrays of integers. The occurrence counts also feed a preprocessing pass that can drop lines
which appear in only one of the two files from the search entirely, because such lines can never
be part of a common subsequence; they are re-inserted into the result afterward as guaranteed
deletions or insertions. Both ends of the file that are already equal (the common prefix and the
common suffix) are also stripped before the search begins, which alone disposes of the vast
majority of the content for a typical small change.

### What "best" means beyond shortest

Two different edit scripts can both be shortest and still not be equally good. Consider inserting
a new function between two existing functions that end with the same `}` line: the inserted block
can be attributed to several equally short scripts that differ in which `}` they treat as new.
Shortest-ness pins down the *number* of edited lines, not their *placement*. This gap between
"minimal" and "readable" is the reason the second half of the theory exists: patience and
histogram diff trade strict minimality for human-aligned anchoring, and the slider heuristics
reposition equal-cost hunks after the fact. All of them accept the same contract: the output must
still be a correct patch, it just does not have to be the mathematically minimal one.

## The edit graph

### Definition

The standard way to reason about edit scripts is the *edit graph*, introduced in Myers' 1986
paper "An O(ND) Difference Algorithm and Its Variations". Lay `A` out along the x axis and `B`
along the y axis. The graph's vertices are the grid points `(x, y)` with `0 <= x <= N` and
`0 <= y <= M`. The edges are:

- a horizontal edge from `(x - 1, y)` to `(x, y)`, meaning *delete* `A[x]`,
- a vertical edge from `(x, y - 1)` to `(x, y)`, meaning *insert* `B[y]`, and
- a diagonal edge from `(x - 1, y - 1)` to `(x, y)` exactly when `A[x] == B[y]`, meaning *keep*
  the matching element.

A path from `(0, 0)` to `(N, M)` is an edit script: it consumes all of `A` moving right, produces
all of `B` moving down, and rides diagonals through the parts the two sides share. If horizontal
and vertical edges cost 1 and diagonal edges cost 0, the cost of a path is the length of its edit
script, and the SES is a minimum-cost path through the graph. Finding a diff is literally a
shortest-path problem on a lattice whose free edges are the matches.

### Diagonals and the k index

Number the diagonals of the grid by `k = x - y`. Diagonal `k = 0` starts at the origin; positive
`k` diagonals sit below-right of it (more of `A` consumed than of `B` produced, i.e. net
deletions so far); negative `k` diagonals sit above-left (net insertions). Two facts make `k` the
natural coordinate:

1. A horizontal (delete) move goes from diagonal `k - 1` to `k`; a vertical (insert) move goes
   from `k + 1` to `k`; a diagonal (match) move stays on `k`.
2. After `d` cost-1 moves, the path must lie on a diagonal `k` with `-d <= k <= d` and with
   `k` congruent to `d` modulo 2, because each cost-1 move changes `k` by exactly one.

The end point `(N, M)` lies on diagonal `delta = N - M`, so the SES length `D` always has the
same parity as `delta`.

### Snakes

A *snake* is a maximal run of diagonal edges: from some point, follow matches as far as they go.
Since diagonal edges are free, any shortest path can be assumed to extend every snake it enters
to its end (stopping mid-snake never helps). This is what turns the search from
"explore every grid point" into "explore one frontier point per diagonal": all the intermediate
points of a snake are implied.

### A concrete graph

The worked example in the next section uses the two character sequences from Myers' paper:

```text
A = A B C A B B A        (N = 7)
B = C B A B A C          (M = 6)
```

The edit graph, with `\` marking the cells that carry a diagonal (match) edge:

```text
          A     B     C     A     B     B     A
       x=0   1     2     3     4     5     6     7
    y=0 +-----+-----+-----+-----+-----+-----+-----+
  C     |     |     |  \  |     |     |     |     |
    y=1 +-----+-----+-----+-----+-----+-----+-----+
  B     |     |  \  |     |     |  \  |  \  |     |
    y=2 +-----+-----+-----+-----+-----+-----+-----+
  A     |  \  |     |     |  \  |     |     |  \  |
    y=3 +-----+-----+-----+-----+-----+-----+-----+
  B     |     |  \  |     |     |  \  |  \  |     |
    y=4 +-----+-----+-----+-----+-----+-----+-----+
  A     |  \  |     |     |  \  |     |     |  \  |
    y=5 +-----+-----+-----+-----+-----+-----+-----+
  C     |     |     |  \  |     |     |     |     |
    y=6 +-----+-----+-----+-----+-----+-----+-----+
```

Every `\` is a free edge; every cell border crossed rightward is a deletion and downward an
insertion. The shortest path from the top-left corner to the bottom-right corner will turn out to
cost 5.

## The Myers algorithm

### Furthest-reaching paths

Call a path from `(0, 0)` that uses exactly `d` non-diagonal edges a *d-path*. The key lemma of
the algorithm is greedy: among all d-paths ending on diagonal `k`, only the one whose endpoint
has the largest `x` matters. Any solution reachable from a less-advanced endpoint on the same
diagonal is also reachable from the furthest one, because the furthest endpoint dominates it
coordinate-wise along the diagonal. So the algorithm keeps exactly one number per diagonal: the
`x` coordinate of the furthest-reaching d-path on that diagonal (the `y` coordinate is implied,
`y = x - k`).

The furthest-reaching d-path on diagonal `k` is built from (d-1)-paths on the two neighboring
diagonals:

- take the furthest (d-1)-path on `k + 1` and move down (an insertion), or
- take the furthest (d-1)-path on `k - 1` and move right (a deletion),

whichever yields the larger `x`, and then slide down the free diagonal as far as matches allow
(extend the snake). Ties prefer the down-move in the standard formulation, which biases scripts
toward emitting deletions before insertions inside a mixed hunk; the choice is arbitrary but must
be consistent so that path reconstruction agrees with the forward pass.

### The V array

The state of the whole search is one array `V`, indexed by `k` from `-D_max` to `D_max`, holding
the furthest-reaching `x` per diagonal. Because a d-pass only reads diagonals of the opposite
parity (written by the previous pass) and writes diagonals of its own parity, the algorithm can
update `V` in place. The full search:

```text
V[1] = 0
for d = 0 .. D_max:
    for k = -d .. d step 2:
        if k == -d or (k != d and V[k - 1] < V[k + 1]):
            x = V[k + 1]          # move down from k + 1: an insertion
        else:
            x = V[k - 1] + 1      # move right from k - 1: a deletion
        y = x - k
        while x < N and y < M and A[x + 1] == B[y + 1]:
            x = x + 1             # follow the snake
            y = y + 1
        V[k] = x
        if x >= N and y >= M:
            return d              # d is the SES length
```

The seed `V[1] = 0` exists so that the very first iteration (`d = 0`, `k = 0`) uniformly takes
the "move down from `k + 1`" branch and lands on `(0, 0)` before extending the initial snake;
it saves a special case rather than expressing anything deep.

The loop invariant to hold on to: after the `d`-th pass, `V[k]` is the largest `x` such that some
edit script with exactly `d` edits reaches `(x, x - k)`. The first `d` whose frontier touches
`(N, M)` is the SES length, because frontiers only ever grow.

### Cost analysis

Each pass touches `d + 1` diagonals and each snake extension consumes a match edge that no other
extension on the same diagonal in the same pass revisits, so pass `d` costs `O(d)` bookkeeping
plus the snake work; over all passes the total is `O((N + M) * D)` in the worst case, and the
expected cost under a plausible model of random inputs is `O(N + M + D^2)`. Space for the
length-only computation is a single array of `2 * (N + M) + 1` integers, i.e. `O(N + M)`.
Recovering the actual script from this version requires remembering the frontier of every pass
(`O(D^2)` space) or re-deriving it, which is what the linear-space refinement below fixes.

The practical shape of that bound is worth internalizing: the algorithm is an expanding wavefront
from the origin. Identical files cost one snake ride down the main diagonal, `O(N)` with `D = 0`.
A one-line change costs three passes. Two unrelated files force the wavefront to sweep most of
the grid, and the cost degrades toward the same order as the DP table. Diff cost tracks the size
of the *change*, not the size of the *files*, until the change stops being small.

### A full worked example

Run the algorithm on `A = ABCABBA`, `B = CBABAC`. The table below records every furthest-reaching
endpoint. Per row: the pass `d`, the diagonal `k`, which neighbor the step came from, the
position after the cost-1 move, the snake followed (if any), and the endpoint stored in `V[k]`.

| d | k | step taken | after move | snake | endpoint (x, y) |
| --- | --- | --- | --- | --- | --- |
| 0 | 0 | seed (down from `V[1] = 0`) | (0, 0) | none: `A[1]=A` vs `B[1]=C` | (0, 0) |
| 1 | -1 | down from `k=0` (`x=0`) | (0, 1) | none: `A[1]=A` vs `B[2]=B` | (0, 1) |
| 1 | +1 | right from `k=0` (`x=0+1`) | (1, 0) | none: `A[2]=B` vs `B[1]=C` | (1, 0) |
| 2 | -2 | down from `k=-1` (`x=0`) | (0, 2) | `(1,3)` then `(2,4)` | (2, 4) |
| 2 | 0 | down from `k=+1` (`x=1`) | (1, 1) | `(2,2)` | (2, 2) |
| 2 | +2 | right from `k=+1` (`x=1+1`) | (2, 0) | `(3,1)` | (3, 1) |
| 3 | -3 | down from `k=-2` (`x=2`) | (2, 5) | `(3,6)` | (3, 6) |
| 3 | -1 | right from `k=-2` (`x=2+1`) | (3, 4) | `(4,5)` | (4, 5) |
| 3 | +1 | down from `k=+2` (`x=3`) | (3, 2) | `(4,3)` then `(5,4)` | (5, 4) |
| 3 | +3 | right from `k=+2` (`x=3+1`) | (4, 1) | `(5,2)` | (5, 2) |
| 4 | -4 | down from `k=-3` (`x=3`) | (3, 7) | out of range | (3, 7) |
| 4 | -2 | down from `k=-1` (`x=4`) | (4, 6) | none | (4, 6) |
| 4 | 0 | down from `k=+1` (`x=5`) | (5, 5) | none: `A[6]=B` vs `B[6]=C` | (5, 5) |
| 4 | +2 | right from `k=+1` (`x=5+1`) | (6, 4) | `(7,5)` | (7, 5) |
| 4 | +4 | right from `k=+3` (`x=5+1`) | (6, 2) | `(7,3)` | (7, 3) |
| 5 | +1 | down from `k=+2` (`x=7`) | (7, 6) | at the corner | (7, 6) |

The choice rule plays out visibly in the table. At `d=3`, `k=-1`, the neighbors are
`V[-2] = 2` and `V[0] = 2`; since `V[k-1] < V[k+1]` is false, the step is a right-move from
`k = -2`. At `d=4`, `k=+2`, the neighbors are `V[+1] = 5` and `V[+3] = 5`; again the tie goes to
the right-move, and the snake through `(7, 5)` carries the frontier to the last column. One pass
later the down-move from that endpoint reaches `(7, 6) = (N, M)` and the algorithm reports
`D = 5`. Note `delta = N - M = 1` is odd, matching the odd script length, and the endpoint
`(3, 7)` at `d=4, k=-4` falls outside the grid: diagonals below `-M` or above `N` can never
contain the corner, and implementations commonly clip the `k` loop to skip them.

### Reconstructing the script

Walking the table backward from `(7, 6)`:

1. `d=5, k=+1` came down from the `d=4, k=+2` endpoint `(7, 5)`: **insert** `B[6] = C`.
2. `d=4, k=+2` came right from the `d=3, k=+1` endpoint `(5, 4)` to `(6, 4)`, then rode the
   snake to `(7, 5)`: **delete** `A[6] = B`, then **keep** `A[7] = A` (matches `B[5]`).
3. `d=3, k=+1` came down from the `d=2, k=+2` endpoint `(3, 1)` to `(3, 2)`, then rode the snake
   to `(5, 4)`: **insert** `B[2] = B`, then **keep** `A[4] = A` and `A[5] = B`.
4. `d=2, k=+2` came right from the `d=1, k=+1` endpoint `(1, 0)` to `(2, 0)`, then rode the
   snake to `(3, 1)`: **delete** `A[2] = B`, then **keep** `A[3] = C`.
5. `d=1, k=+1` came right from the `d=0, k=0` endpoint `(0, 0)`: **delete** `A[1] = A`.

Read forward, the script is: delete `A`, delete `B`, keep `C`, insert `B`, keep `A B`, delete
`B`, keep `A`, insert `C`. Applying it to `ABCABBA` yields `CBABAC`, which is `B`, using exactly
5 edits. Rendered the way a line diff would show it:

```diff
-A
-B
 C
+B
 A
 B
-B
 A
+C
```

Every property claimed earlier is visible in miniature: the kept lines (`C`, `A B`, `A`) form a
common subsequence of length 4, and `D = N + M - 2L = 7 + 6 - 8 = 5` checks out.

## Linear-space refinement

### The problem with remembering frontiers

The forward search finds the *length* of the SES in `O(N + M)` space, but reconstructing the
script needs the predecessor of every frontier endpoint, and storing a copy of `V` for each pass
costs `O(D^2)` space. For a large file with a large `D` (imports reordered, code reformatted)
that can dwarf the input. Myers' refinement recovers the script in `O(N + M)` space at roughly
twice the time, using divide and conquer around the *middle snake*.

### Bidirectional search and the middle snake

Run two frontier searches simultaneously:

- a forward search from `(0, 0)` exactly as above, and
- a reverse search from `(N, M)` that walks the graph backward (its "deletions" move left, its
  "insertions" move up, its snakes ride diagonals upward), keeping its own furthest-reaching
  array centered on the diagonal `delta = N - M` of the end corner.

If the SES has length `D`, the two searches meet after each has performed about `D / 2` passes:
an optimal D-path can be split into a leading part with `ceil(D / 2)` edits and a trailing part
with `floor(D / 2)` edits, and the split point sits on a snake (possibly empty) that both
searches discover. Parity decides who detects the overlap:

- When `delta` is odd, `D` is odd, and the forward search detects the meeting during its pass
  `d` on the diagonals the reverse search populated in pass `d - 1`.
- When `delta` is even, `D` is even, and the reverse search detects the meeting during its own
  pass `d` against the forward frontier of the same pass.

Concretely, after each frontier update the algorithm checks whether the opposing search's
furthest-reaching endpoint on the same diagonal has been passed; the first such crossing yields
both the total length `D = 2d - 1` or `2d` and the coordinates of the *middle snake*: the
matching run `(x1, y1) .. (x2, y2)` where the optimal path crosses the midline of the graph.

### Divide and conquer

The middle snake splits the problem in two: the region from `(0, 0)` to `(x1, y1)` and the
region from `(x2, y2)` to `(N, M)`. Each is a smaller diff problem, solved by the same
find-the-middle-snake procedure recursively. The recursion bottoms out when a region has zero
edits (pure snake) or when one side of the region is empty (pure insertions or deletions). The
concatenation of all the middle snakes found along the way, in order, is exactly the set of kept
lines, and everything between consecutive snakes is emitted as deletions and insertions.

The cost recurrence is the pleasant kind: finding the middle snake of a region with edit
distance `D` costs `O((N + M) * D / 2)` there, and the two subproblems have edit distances that
sum to `D`. Summing over the recursion tree gives a constant-factor overhead over the one-shot
search (the usual back-of-envelope says total work roughly doubles), while space drops to the
two frontier arrays plus the recursion stack, `O(N + M)` overall with recursion depth `O(log)`
in the balanced case and bounded by `D` in the worst one. This is the variant production diff
engines implement, including the `xdiff` library embedded in Git, where the function that finds
the middle snake and recurses is the heart of the default algorithm.

### Why this matters to a consumer like Quinjet

None of this machinery is visible in the output, and that is the point: hunk content does not
depend on which space strategy computed it. What a consumer does observe is the cost profile.
Because Git's engine is `O((N + M) * D)` with linear space, patch production cost tracks change
size, so a tool can treat "one `git diff` per small file" as cheap and "one `git diff` for a
massive rewrite" as bounded mainly by output size rather than by algorithmic blowup. Quinjet's
own defenses therefore target the *output*, not the algorithm: every patch read is capped at
8 MiB and the child process is killed on overflow (see
[the flag set on every patch read](#the-flag-set-on-every-patch-read)), because a diff engine
that is well-behaved in time can still emit more bytes than a TUI should buffer.

## Patience diff

### The idea

Patience diff, designed by Bram Cohen, starts from the observation that minimality is the wrong
objective for readability. The lines that should anchor an alignment are the *distinctive* ones:
function signatures, unique statements, section headers. Lines that occur many times (blank
lines, `}`, `end`) are exactly the ones a shortest-path search is happiest to match and exactly
the ones humans do not want matched across unrelated regions. The algorithm:

1. Trim the common prefix and suffix of the two files.
2. Collect the lines that occur *exactly once in each* file. Each such line gives a pair
   `(position in A, position in B)`.
3. Among those pairs, find the longest subsequence that is increasing in both coordinates: the
   longest common subsequence of the unique lines. These become the anchors.
4. Recurse into each gap between consecutive anchors (and before the first and after the last).
   A gap that still contains unique-on-both-sides lines repeats the process; a gap with none is
   handed to a conventional diff (Git's implementation falls back to its standard engine for
   those spans).

### Patience sorting and the LIS

Step 3 is a longest increasing subsequence (LIS) problem, and the algorithm's name comes from
solving it with patience sorting. Sort the pairs by their `A` position and read out their `B`
positions; the task is the LIS of that sequence of numbers. Deal the numbers onto piles left to
right: each number goes on the leftmost pile whose top is greater than it, or starts a new pile
to the right; each number also keeps a backpointer to the top of the pile to its left at the
moment it was placed. The number of piles equals the LIS length, and following backpointers from
the top of the last pile reads out one LIS in reverse.

Worked example: five lines `a b c d e` are unique in both files, appearing in `A` in that order
and in `B` at positions `b=1, c=2, e=3, a=4, d=5`. Reading `B` positions in `A` order gives the
sequence `4 1 2 5 3`:

```text
deal 4:  pile1[4]
deal 1:  pile1[1 over 4]                      (1 <= 4, leftmost pile)
deal 2:  pile1[1,4]  pile2[2]                 (2 > 1, new pile; backpointer 2 -> 1)
deal 5:  pile1[1,4]  pile2[2]  pile3[5]       (5 > 2, new pile; backpointer 5 -> 2)
deal 3:  pile1[1,4]  pile2[2]  pile3[3 over 5] (3 <= 5; backpointer 3 -> 2)
```

Three piles, so the LIS has length 3; walking back from the final top `3 -> 2 -> 1` yields
`1 2 3`, i.e. the lines `b`, `c`, `e` in both files' order. Those three lines are the anchors;
the diff recurses between them. The dealing pass is `O(n log n)` with binary search over pile
tops, negligible against the line count.

### What it buys and what it costs

The classic demonstration is inserting a complete function between two existing functions in a
C-like language. A shortest-path diff is free to match the inserted function's closing `}` and
trailing blank line against the first function's, producing a hunk that opens mid-function and
reads as if the old function acquired a new body. Patience never does this, because `}` occurs
many times and is therefore not an anchor; the unique signature lines pin each function to
itself and the insertion falls out as one clean contiguous `+` block.

The costs are symmetrical. Patience diffs can be *longer* than minimal (anchoring is a
constraint the optimum does not have), files dominated by repeated boilerplate offer few unique
lines and degrade to the fallback engine, and a line that was unique until an edit duplicated it
silently stops being an anchor, so output can change shape in response to edits far away. Git
exposes the algorithm as `--patience` or `diff.algorithm=patience`; see
[git-diff](https://git-scm.com/docs/git-diff).

## Histogram diff

### From unique lines to rare lines

Histogram diff generalizes patience. Patience refuses to anchor on any line that is not unique
on both sides, which throws information away: a line occurring twice is still a far better
anchor than a blank line occurring four hundred times. Histogram diff, which originated in JGit
and was later ported into Git's own `xdiff` as `--histogram`, keeps the anchoring idea but ranks
candidate anchors by occurrence count instead of demanding uniqueness:

1. Scan side `A` and build a histogram: for each distinct line content, how many times it occurs
   and where. This is the same hashing infrastructure the other algorithms use, extended with
   per-content occurrence chains.
2. Scan side `B` looking for regions that also exist in `A`, and among all common elements
   choose as the split point the one whose occurrence count in `A` is lowest, extending it to
   the longest common region around that element. A unique line (count 1) is the ideal anchor;
   failing that, the rarest available line wins.
3. Split both sequences around the chosen common region and recurse on the two remainders,
   exactly as patience recurses between its anchors.
4. If a region has no common element at all, everything in it is emitted as pure deletions plus
   insertions.

To keep the scan linear in practice, the implementation refuses to consider elements whose
occurrence chain is longer than a small fixed cap; a region dominated by such high-frequency
content is handled without anchoring rather than spending quadratic effort enumerating
positions. The result is an algorithm that behaves like patience on typical source code (where
good anchors are unique), degrades more gracefully than patience when anchors are merely rare
rather than unique, and in practice often runs faster than the default Myers engine because it
never builds a `D`-wide frontier: its work is dominated by hashing and by the recursive splits.

### Selection in Git

Git exposes all four line-diff strategies through one knob, per invocation
(`git diff --diff-algorithm=<name>`) or per user (`git config diff.algorithm <name>`):

| Name | Strategy |
| --- | --- |
| `myers` | The default: middle-snake Myers with the speed heuristics enabled |
| `minimal` | Myers with the heuristics disabled; guaranteed shortest script |
| `patience` | Unique-line anchoring with recursion, fallback for anchorless spans |
| `histogram` | Rare-line anchoring; the patience idea with occurrence-ranked anchors |

All four emit the same output format, honor the same context and rename flags, and differ only
in which of the many correct edit scripts they choose. That interchangeability is a fact
Quinjet's design leans on directly: since Quinjet parses whatever hunks `git` emits and never
re-derives them, a user's configured `diff.algorithm` flows straight through to the TUI. The
hunks on screen are the same hunks the user's own `git diff` would print at a shell, whichever
engine their configuration selects. Quinjet passes no `--diff-algorithm` override anywhere; the
catalog of its exact invocations appears in
[Quinjet: Git as the hunk authority](#quinjet-git-as-the-hunk-authority).

## Minimal diffs and the default heuristics

### What the default gives up

Git's default Myers mode is not the textbook algorithm; it is the textbook algorithm plus
cutoffs that bound the work spent hunting for the true middle snake. The two families of
shortcut:

- **A cost ceiling on the frontier search.** When the bidirectional search has run for more
  passes than a budget derived from the input size, the engine stops looking for the optimal
  meeting point and instead picks the best-looking frontier position it has: the endpoint that
  has consumed the most input. The split is still a valid division of the problem (both halves
  recurse normally), it just may not lie on an optimal path, so the final script can be slightly
  longer than minimal.
- **Early snake acceptance.** While expanding the frontier, the engine watches for long snakes
  far along a diagonal. A sufficiently long run of matches discovered by either direction is
  taken as the split point immediately, on the bet that a long common run almost certainly
  belongs to the optimal alignment. The bet is usually right, and it short-circuits a large
  amount of frontier expansion on big inputs.

`--minimal` disables these shortcuts and instructs the engine to "spend extra time to make sure
the smallest possible diff is produced" (the manpage's own phrasing; see
[git-diff](https://git-scm.com/docs/git-diff)). The guarantee costs real time precisely on the
inputs where the heuristics were saving it: large files with large edit distances.

### Why minimality is rarely worth buying

The scripts the heuristics produce differ from minimal ones by a handful of lines on
pathological inputs and not at all on typical ones. Meanwhile every consumer of a diff already
tolerates non-minimality: patience and histogram are non-minimal *by design*, and post-passes
like hunk sliding (next section) freely trade one equal-length script for another. A tool that
displays diffs, as Quinjet does, has no use for the guarantee: the patch applies identically
either way, per-file `+n -n` totals come from `--numstat` rather than from any particular
script's shape, and the extra CPU would be spent inside a subprocess the user is waiting on.
Quinjet accordingly never passes `--minimal`; the flags it does pass exist to stabilize the
*format* of the output, not to tune the algorithm (see
[the flag set on every patch read](#the-flag-set-on-every-patch-read)).

## The slider problem and the indent heuristic

### Ambiguity among equal-cost scripts

When a run of inserted (or deleted) lines is bordered by content identical to one of its own
edges, the run can "slide": several placements produce byte-identical results and identical edit
costs. The canonical case is inserting a new entry into a list or a new function into a file
where the block ends the same way its neighbor does. Both of these hunks describe the same
change:

```diff
 void alpha() {
     a();
 }
+
+void beta() {
+    b();
+}
```

```diff
 void alpha() {
     a();
+}
+
+void beta() {
+    b();
 }
```

The first attributes the inserted block cleanly: a blank line and a complete new function. The
second matches the *new* function's closing brace against `alpha`'s old closing brace, and the
hunk appears to tear `alpha` open. An SES-based engine has no reason to prefer either; both
scripts contain exactly four insertions. Which one falls out depends on incidental tie-breaking
inside the search, which is why "the diff put my closing brace in the wrong function" was for
years the most recognizable complaint about Myers output.

### Sliding and scoring

Git resolves the ambiguity in a post-pass over each change group. First it computes how far the
group can shift up and down while keeping the result identical (the *slider range*). Then it
chooses a position within that range:

- The historical default shifted groups as far as possible in one direction, which at least made
  placement deterministic, if not pretty.
- The *indent heuristic*, introduced behind `--indent-heuristic` and later made the default,
  scores every position in the slider range using the shape of the surrounding text: the
  indentation of the lines at the candidate boundaries and their neighbors, and the presence of
  blank lines immediately before or after the group. The scoring encodes preferences mined from
  large corpora of human-judged diffs, such as: a hunk boundary next to a blank line is good, a
  boundary that splits an indented block from its introducing line is bad, and boundaries at
  small indentation (function level rather than statement level) beat boundaries deep inside a
  block. The position with the best score wins.

The heuristic changes no semantics; every candidate placement was already a correct, equal-cost
patch. It exists purely because diffs are read by people, and it is the strongest single
example of the theme running through this half of the page: after correctness, diff quality is
a presentation problem. Quinjet inherits the heuristic's benefits for free through the
subprocess boundary, and applies the same philosophy at a smaller scale in its own intraline
range computation, where an analogous ambiguity (which repeated character to mark as changed)
is resolved by a deterministic greedy rule; see
[Quinjet's own diff-shaped computation](#quinjets-own-diff-shaped-computation).

## Rename and copy detection

### Renames are not in the data model

Git does not record renames. A commit stores complete trees; when a file moves, one path stops
existing and another starts existing with (possibly) similar content, and nothing in the object
model links them (see [../git-internals/object-model.md](../git-internals/object-model.md)).
Rename *detection* is a diff-time inference performed by the diffcore pipeline documented in
[gitdiffcore](https://git-scm.com/docs/gitdiffcore): after computing the raw file-pair list, a
`diffcore-rename` pass tries to match deleted paths with added paths and rewrites matched pairs
into rename records.

### Exact and inexact matching

Detection runs in two phases with very different costs:

1. **Exact matches.** If a deleted file's blob OID equals an added file's blob OID, the content
   is byte-identical and the pair is a certain rename. Because content addressing already hashed
   every blob, this phase is a hash-table join: near free, no content is read at all. This is
   the common case for pure `git mv` style moves and the reason rename detection is cheap in
   practice. The same content-addressing property is what makes Quinjet's OID-keyed caches sound
   (see [../github/caching.md](../github/caching.md)).
2. **Inexact matches.** Remaining candidates are compared by content similarity. Each file is
   reduced to a set of hashed chunks with sizes; the similarity of a (deleted, added) pair is
   the proportion of shared chunk content relative to the larger file, scaled to a percentage.
   Pairs are scored, the best assignments are chosen greedily from the top scores, and any pair
   at or above the threshold becomes a rename. The default threshold is 50 percent, adjustable
   as `--find-renames=<n>` (alias `-M<n>`); `git diff -M90%` demands near-identity.

Inexact matching is where the cost lives: with `d` deletions and `a` additions the candidate
matrix is `d * a`, quadratic in the worst case, and each cell needs the chunk signatures of both
files. Git bounds the work with a configurable ceiling on how many candidates it will consider
(`diff.renameLimit` and merge-time equivalents); past the ceiling it silently skips inexact
detection and reports the pairs as plain delete plus add. Copy detection (`--find-copies`, and
the expensive `--find-copies-harder` which considers unchanged files as copy sources) extends
the same scoring to files that still exist on both sides.

### What renames look like in output

A detected rename changes the *shape* of every diff output format, which is exactly why a parser
must be rename-aware end to end:

- In `--name-status` output the status letter becomes `R` followed by the score (`R100` for
  exact), and the record carries *two* paths, pre-image then post-image. With `-z` those are two
  separate NUL-terminated fields after the status field.
- In `--numstat -z` output a renamed file emits an *empty* path field followed by two extra
  NUL-terminated records, pre-image then post-image.
- In patch output the extended header lines `similarity index NN%`, `rename from <old>` and
  `rename to <new>` appear between the `diff --git` line and any hunks; a 100 percent rename has
  no hunks at all.

Every one of those shapes has a dedicated handler in Quinjet's parsers, covered in
[rename detection in the Quinjet pipeline](#rename-detection-in-the-quinjet-pipeline).

## Word-level diffing

### Tokens instead of lines

Line-level output is too coarse when a long line changes by one identifier. Git's answer is
`--word-diff`, which re-tokenizes changed regions and runs the same LCS machinery over words:
split each line into tokens (by default at whitespace; configurable per invocation with
`--word-diff-regex` or per file type through a driver's `wordRegex`), diff the token sequences,
and render the result inline. The modes differ only in serialization: `plain` brackets removals
as `[-old-]` and additions as `{+new+}`, `color` uses color alone, and `porcelain` emits a
machine-readable per-token format. `--color-words` is shorthand for the color mode.

Two properties of word diffing matter for a consumer deciding whether to use it:

1. It changes the *output shape*, not just the styling. A word-diff patch no longer round-trips
   through `git apply`; it is a presentation format, and a tool that also needs an applicable or
   line-addressable patch must run a second, plain diff anyway.
2. Tokenization quality decides output quality. Whitespace tokenization mishandles punctuation
   dense code (`foo(bar,baz)` is one token), so per-language regexes are effectively required
   for good results, and those regexes are user configuration, not something a tool can rely on
   being present.

### Intraline emphasis in diff viewers

Because of those two properties, interactive diff tools almost universally leave Git's word diff
alone and compute their own intraline emphasis on top of a plain line diff: pair up removed and
added lines, then compute a finer-grained difference within each pair and highlight it. The
per-pair computation ranges from full word-level LCS (as in editor diff views) down to the
cheapest useful form: the common-prefix and common-suffix trim, which marks the single
contiguous region where the two lines disagree. Quinjet takes the cheap end of that spectrum
deliberately, computes it only for row pairs actually on screen, and bounds the input size per
pair; the full analysis of that choice, including where it is weaker than a word LCS and why
that trade is right for a per-frame budget, is in
[Quinjet's own diff-shaped computation](#quinjets-own-diff-shaped-computation) and in
[./intraline-and-highlighting.md](./intraline-and-highlighting.md).

## Binary diffs and deltas

### Detection

Text diff algorithms assume line structure, so the first question about any file pair is whether
line structure exists. Git's heuristic is byte-level: a file whose opening block contains a NUL
byte is treated as binary (real text virtually never contains NUL; UTF-16 text famously fails
this test and needs a `.gitattributes` override). The determination is per file and can be
forced either way with attributes (`-diff` to mark binary, `diff` to force text).

In machine-readable listings, binary files surface as `-` in both numeric columns of
`--numstat`, a shape Quinjet's `parse_numstat` in `src/git/diff.rs` maps to
`DiffLineCounts { binary: true }` so headers can render a `· binary` marker instead of counts.
In patch output, a binary pair without `--binary` collapses to a single line:

```text
Binary files a/logo.png and b/logo.png differ
```

With `--binary`, Git instead emits an applicable payload under a `GIT binary patch` header:
base85-encoded, zlib-compressed data in one of two encodings, `literal` (the complete new
content) or `delta` (instructions against the old content), whichever is smaller, and usually
both directions so the patch can be applied in reverse.

### Delta encoding

The `delta` encoding is the same idea as the deltas inside packfiles: a program of two
instructions replayed against a source buffer. A *copy* instruction names an offset and length
in the source; an *insert* instruction carries fresh bytes. Finding a small delta is itself a
diff problem, solved not with Myers (byte sequences are too long and line structure is absent)
but with fingerprinting: index the source by hashing fixed-size windows, then scan the target
greedily, at each position looking up the window hash to find the longest copyable run,
emitting an insert when no run is found. This family (xdelta, and Git's own delta code) trades
optimality for a single linear scan, the same "good alignment fast" bargain the text-diff
heuristics make. The full byte-level format of pack deltas, chains, and their inflation cost is
covered in [../git-internals/packfiles-and-deltas.md](../git-internals/packfiles-and-deltas.md);
its headline consequence for Quinjet is that *transferring* a blob is cheap while
*materializing* one is not, which is why the PR pipeline avoids local blob reads whenever the
GitHub API already knows the answer (see
[../github/api-strategy.md](../github/api-strategy.md)).

### Quinjet and binary content

Quinjet never renders binary content, so its handling reduces to honest labeling at every layer:

- Index layer: `parse_numstat` (src/git/diff.rs:147-182) marks `binary` when either count field
  is `-`, and `DiffFileIndexEntry::label()` appends `· binary` to the header line.
- Patch layer: `parse_diff` (src/git/diff.rs:408-609) recognizes both the `Binary files `
  prefix and the exact `GIT binary patch` line, marks the file builder binary, and demotes every
  subsequent line of that file to a `Meta` row, so even a `--binary` payload that slipped
  through would render as inert text rather than being interpreted.
- Synthesis layer: for untracked files, which Git cannot diff at all, `untracked_patch`
  (src/git/mod.rs:1122-1165) reads the file directly and fabricates a patch; any NUL byte in
  the content flips the fabricated patch to the `Binary files /dev/null and b/<path> differ`
  form, mirroring Git's own detection heuristic byte for byte.

## Quinjet: Git as the hunk authority

### One authority, zero reimplementation

Quinjet links neither `libgit2` nor `gitoxide` and contains no diff engine. Every hunk on
screen was computed by a spawned `git` subprocess; the code in `src/git/diff.rs` is a *format
parser* for unified-diff bytes, not a difference algorithm. The decision follows directly from
the theory above:

1. **Hunk placement is underdetermined.** As the slider section showed, many correct outputs
   exist for one change, and which one appears depends on the algorithm, its heuristics, and
   its version. A reimplementation would produce hunks that disagree, subtly and permanently,
   with what `git diff`, `git add -p`, forge web UIs, and every other tool in the user's life
   shows for the same change. Delegating makes disagreement structurally impossible: the hunks
   in the TUI are the hunks the user's own Git produces, including their configured
   `diff.algorithm`, because Quinjet passes no algorithm override.
2. **The hard parts are not the search.** The middle-snake search is a few hundred lines; the
   surrounding machinery is not. Rename scoring, the diffcore pipeline, binary detection,
   attribute handling, path quoting, submodule boundaries, and the accumulated post-processing
   heuristics represent decades of casework. All of it arrives for free through one argv.
3. **The cost model tolerates a subprocess.** Diff computation cost tracks change size, and a
   process spawn is microseconds against a keypress. What does *not* tolerate carelessness is
   output volume, so every read is byte-capped with the child killed on overflow, and the
   expensive views are assembled from cheap metadata reads before any patch bytes are
   requested.

The architecture document states the boundary as a hard invariant: the render path never spawns
Git (`ARCHITECTURE.md`, design goals), the worker "performs no Git work of its own", and all
Git and GitHub CLI processes "receive argv directly, never via a shell" (invariant 7).

### The three-read design

The theory of this page describes what one `git diff` invocation computes. Quinjet's insight,
detailed in [./pipeline.md](./pipeline.md) and load-bearing for every large view, is that a
diff *view* should not start with a patch at all. The local diff workspace issues three
different kinds of read with three different cost profiles (src/git/mod.rs):

1. **The file list**: `git diff --name-status -z --find-renames <base> <head> --`. Status
   letters and paths only. Internally Git still runs rename detection (it must, to emit `R`
   records), but emits no hunks, so output is a few dozen bytes per file no matter how large
   each change is. This read builds the index of collapsed headers that renders immediately
   (invariant 8).
2. **Per-file totals**: the same command with `--name-status` swapped for `--numstat`. Adds
   and deletes per file, again without materializing patch text into the output. These totals
   let every header show its real `+n -n` before any patch exists (invariant 8a).
3. **Patch bodies**: `git diff --patch` for one path (or one batch of paths), issued only when
   a file is actually displayed or prefetched.

The first two reads are the ones the DP-table intuition says should be expensive and are not:
Git computes the line counts with the same engine but streams only the summary. The exact
argv for the index read, from `src/git/mod.rs`:

```rust
fn diff_index_args(base: &str, head: &str) -> Vec<OsString> {
    vec![
        OsString::from("diff"),
        OsString::from("--name-status"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(base),
        OsString::from(head),
        OsString::from("--"),
    ]
}
```

The numstat variant is not written out a second time; it is *derived*, guaranteeing the two
reads can never drift apart in revision range or rename settings. From `src/git/mod.rs`:

```rust
/// Reuse an index command's own revision range for its totals by swapping the
/// listing option. This keeps the two reads describing exactly the same diff.
fn numstat_args(args: &[OsString]) -> Option<Vec<OsString>> {
    let name_status = OsStr::new("--name-status");
    args.iter().any(|arg| arg == name_status).then(|| {
        args.iter()
            .map(|arg| {
                if arg == name_status {
                    OsString::from("--numstat")
                } else {
                    arg.clone()
                }
            })
            .collect()
    })
}
```

This is a small function carrying a real correctness property. `--find-renames` changes which
records exist (a rename is one `R` record, not a `D` plus an `A`), so if the counts read ever
ran without it while the listing ran with it, renamed files would key their counts under the
wrong shape and headers would silently show placeholders. Deriving one argv from the other
makes that class of bug unrepresentable. The counts read is also explicitly non-load-bearing:
`numstat_counts` (src/git/mod.rs:511-517) documents that "Counts are a rendering enhancement,
never a correctness requirement, so a failed or bounded read simply leaves the affected headers
unresolved", and headers fall back to the `+·· -··` placeholder pair.

For working-tree changes the file list is already known from the status snapshot, so only the
totals are fetched: `apply_worktree_counts` (src/git/mod.rs:469-509) runs
`git diff --numstat [--cached] -z --find-renames --` at most twice, once per populated area
(staged, unstaged), regardless of how many files changed. Two subprocesses is the ceiling.

### The same design, PR-shaped

The pull-request pipeline replays the identical structure against a merge-base/head OID pair
(the pair itself is resolved through the compare API and a fetch ladder documented in
[../git-internals/merge-bases-and-history.md](../git-internals/merge-bases-and-history.md)):

- The file list is `git diff --name-status -z --find-renames <merge_base> <head> --` run in
  whichever repository holds the objects, capped at 8 MiB of output and 16,384 parsed files
  (`changed_files_in_repository`, src/git/github/mod.rs:1981-2089), cached forever under the
  key `pr-files-v1\n{merge_base}\n{head}` because an OID pair can never produce different
  bytes.
- Per-file counts prefer the GitHub pulls files endpoint over a local `--numstat` when the
  workspace is a blob-less partial clone, because "a local `--numstat` would download every
  changed blob just to count lines; GitHub already knows the totals" (doc comment,
  src/git/github/mod.rs:1235-1237). When the objects are already local, the same
  `git diff --numstat -z --find-renames` read runs instead, cached under
  `pr-numstat-v1\n{merge_base}\n{head}`.
- Patch bodies arrive per selected file and in background batches, each batch one Git
  invocation split back into per-file documents (next section).

Counts do double duty in the PR view. Beyond header rendering, they drive the prefetch
scheduler's byte estimates: `estimated_patch_bytes` (src/app.rs:7052-7059) prices a file at
`(additions + deletions) * 80 + 4096` bytes, falling back to 512 KiB when a file has no
counts, and batches accumulate files until a 6 MiB estimated budget or 32 files, whichever
comes first. Accurate counts up front are what let the batcher pack near the 8 MiB pipe cap
without tripping it. This is also where the stack's history shows an evolution step: PR #50
introduced a size-tiered ordering that prefetched the smallest files first on very large pull
requests, so many files finished early; PR #55 replaced that ordering with the current
viewport-anchored walk, which starts at the first file visible in the Files tree and wraps
around the whole index (up to 4,096 files), so the bytes land where the reader is actually
looking. The scheduling story in full lives in [../github/prefetch.md](../github/prefetch.md)
and [../rendering/progressive-loading.md](../rendering/progressive-loading.md).

### Parsing without recomputing

`parse_diff` (src/git/diff.rs:408-609) consumes patch bytes line by line and rebuilds document
rows, and its relationship to the algorithms above is strictly *trusting*. It never checks
that hunks are minimal, never re-diffs content, and never second-guesses line numbers beyond
arithmetic: `parse_hunk_starts` reads the `-a,b +c,d` fields of each `@@` header and the
parser then counts forward, assigning `old_line`/`new_line` to each `-`, `+`, and context row.
From `src/git/diff.rs`:

```rust
fn parse_hunk_starts(line: &str) -> (Option<usize>, Option<usize>) {
    let mut fields = line.split_ascii_whitespace();
    let _marker = fields.next();
    let old = fields
        .next()
        .and_then(|field| parse_range_start(field, '-'));
    let new = fields
        .next()
        .and_then(|field| parse_range_start(field, '+'));
    (old, new)
}
```

Everything the diff engine decided (hunk boundaries, slider placement, rename pairing) passes
through unchanged. The parser's own work is the part Git cannot do: splitting per-file
sections, tracking two independent syntax-highlighting states so added lines cannot corrupt
the old side's grammar state, expanding tabs to display columns, and enforcing the render-side
budgets (512 KiB of grammar parsing per patch, 32 KiB per row). Those mechanics belong to
[./pipeline.md](./pipeline.md) and
[./intraline-and-highlighting.md](./intraline-and-highlighting.md).

## The flag set on every patch read

### The canonical invocation

Every patch body Quinjet requests, local or PR, uses one shape. The single-file revision diff,
from `src/git/mod.rs`:

```rust
fn revision_diff_file(
    &self,
    base: &str,
    head: &str,
    file: &DiffFileIndexEntry,
    expanded: bool,
    title: &str,
) -> Result<DiffDocument> {
    let mut args = vec![
        OsString::from("diff"),
        OsString::from("--no-color"),
        OsString::from("--no-ext-diff"),
        OsString::from("--find-renames"),
        OsString::from("--patch"),
        OsString::from(if expanded {
            "--unified=1000000"
        } else {
            "--unified=3"
        }),
        OsString::from(base),
        OsString::from(head),
        OsString::from("--"),
    ];
    append_diff_file_paths(&mut args, file);
    self.diff_document_from_args(args, title, &file.path)
}
```

The PR batch variant is the same skeleton with a fixed context radius and many trailing paths,
from `src/git/github/mod.rs`:

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

### What each flag buys

Each flag exists to remove a failure mode, and each maps back to a mechanism from the theory
half or from Git's configuration surface:

- **`--no-ext-diff`.** Git allows an *external diff driver* per file type: a user-configured
  program that replaces the internal engine and may print anything at all. A parser expecting
  unified-diff bytes cannot survive that, and a TUI must never let repository-supplied
  configuration (`.gitattributes` plus a driver definition) choose an arbitrary program to
  execute on the user's behalf. This flag pins output to the built-in engine: exactly the
  algorithms this page describes, in whatever `diff.algorithm` mode the user configured, and
  nothing else.
- **`--no-color`.** ANSI escapes would otherwise interleave with content depending on the
  user's `color.diff` settings. The parser sees plain bytes; color is applied at render time
  from parsed structure, which is also what allows the theme to restyle a cached document
  without re-running Git.
- **`--find-renames`.** Keeps the patch read's diffcore pipeline consistent with the index
  read's: a file the index listed as `renamed` produces `rename from`/`rename to` headers here
  rather than a delete-plus-add pair, so the per-file document and its header agree. Rename
  scoring theory is in [rename and copy detection](#rename-and-copy-detection); its pipeline
  consequences in
  [rename detection in the Quinjet pipeline](#rename-detection-in-the-quinjet-pipeline).
- **`--patch`.** Explicit, because some of the surrounding commands default to other output
  and the argv is assembled programmatically; stating the mode makes the command
  self-describing regardless of configuration such as `diff.noprefix` era defaults.
- **`--unified=3` versus `--unified=1000000`.** The context radius doubles as the view mode;
  the next section is devoted to it.
- **`--` then paths.** The separator makes it syntactically impossible for a path to be parsed
  as a flag, one instance of the repo-wide rule that user text never reaches an argv position
  where it could be an option (invariant 7). For renames, `append_diff_file_paths`
  (src/git/mod.rs:1368-1373) pushes the pre-image path before the post-image path, so the
  pathspec covers both sides and the engine can still pair them.

Equally telling is what is absent. No `--diff-algorithm`: user configuration governs. No
`--minimal`: minimality buys nothing at display time. No `-U0`: zero-context patches save
bytes but destroy the reader's orientation and the slider heuristic's room to work. No
`--word-diff`: it would change the output shape away from an applicable patch, and intraline
emphasis is computed locally instead. No `--color-moved`: moved-line analysis is a display
transform Git applies to its own output; Quinjet's document model does not consume it, and it
would reintroduce the ANSI parsing problem `--no-color` removes.

### The environment around the argv

The flags travel with an environment that stabilizes the substrate (`checked_bounded`,
src/git/mod.rs:1258-1278): `-C <root>` so worker threads never depend on process cwd,
`-c core.quotepath=false` so non-ASCII paths arrive as raw bytes rather than octal-escaped
quoted strings, `LC_ALL=C` so no output is ever localized, `GIT_OPTIONAL_LOCKS=0` so read
commands never take `index.lock` opportunistically, and `GIT_TERMINAL_PROMPT=0` so a fetch
against an authenticated remote fails instead of freezing a worker on a credential prompt.
The full catalog of invocations and their parsers is in
[../git-internals/plumbing-and-porcelain.md](../git-internals/plumbing-and-porcelain.md).

### The byte caps as an algorithmic backstop

The cost analysis earlier ended with a caveat: a well-behaved engine can still emit unbounded
bytes, because output size is a property of the *change*, not of the algorithm. Quinjet's
caps close that hole mechanically rather than statistically:

| Cap | Value | Applies to |
| --- | --- | --- |
| `MAX_DIFF_BYTES` | 8 MiB | any single patch read, local or PR |
| `MAX_DIFF_INDEX_BYTES` | 8 MiB | `--name-status` and `--numstat` listings |
| `MAX_DIFF_INDEX_FILES` | 16,384 | files parsed into a local diff index |
| `MAX_PR_PATH_BYTES` | 8 MiB | the PR file listing and its cache entry |
| `MAX_PR_PATHS` | 16,384 | files parsed into a PR index |
| `MAX_CACHED_PATCH_BYTES` | 1 MiB | one file's patch in the on-disk cache |

The enforcement is the important part: `run_bounded_command`
(src/git/github/mod.rs:2222-2274) reads the child's stdout in 64 KiB chunks and kills the
child the moment the running total would cross the limit, rather than buffering everything
and truncating afterward (invariant 6). A truncated patch is then popped back to its last
complete line (`truncate_to_complete_line`, src/git/mod.rs:1554-1558), so the parser never
sees a half line, and the document is flagged truncated so the view can say so
("… diff truncated to keep Quinjet responsive …"). Listing reads are cut back to the last
complete NUL record instead, so a truncated index still parses as whole records.

### Batching and splitting at diff boundaries

One subprocess per file is the wrong shape for a pull request with thousands of files; process
spawn overhead would dominate. The doc comment on `PreparedPullRequest::diff_files`
(src/git/github/mod.rs:440-517) states the design plainly: "Spawning one Git process per file
dominates the cost of a wide pull request, so batching is what lets the whole diff arrive
while the reader is still reading the first file." The batch runs `diff_selected_paths` once
for up to 32 paths, then `split_patch_by_file` cuts the combined output back into per-file
sections. The splitter, from `src/git/diff.rs`, is a single allocation-free scan for the three
header forms a combined patch can contain:

```rust
pub(crate) fn split_patch_by_file(patch: &[u8]) -> Vec<PatchSection<'_>> {
    let mut starts = Vec::new();
    let mut offset = 0;
    while offset < patch.len() {
        let end = patch
            .get(offset..)
            .unwrap_or_default()
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(patch.len(), |index| offset + index + 1);
        let line = patch.get(offset..end).unwrap_or_default();
        if line.starts_with(b"diff --git ")
            || line.starts_with(b"diff --cc ")
            || line.starts_with(b"diff --combined ")
        {
            starts.push(offset);
        }
        offset = end;
    }
    ...
}
```

Each section is keyed by the paths parsed from its own header (`PatchSection { old_path,
new_path, body }`), and `PatchSection::matches(path)` accepts either side, so a renamed file
is found whether the caller asked under the pre-image or the post-image name. Splitting on
`diff --git` line starts is sound because the diff format guarantees those strings begin a
line only at file boundaries; content lines are always prefixed with a marker character
(` `, `+`, `-`, `@`, or a header keyword), so a source file that itself contains the text
`diff --git` can never forge a boundary at column zero of a content line: the marker prefix
shifts it to column one or later.

Truncation interacts with batching carefully (src/git/github/mod.rs:487-515): when the 8 MiB
cap cuts a batch, only the final section can be incomplete, so complete sections are still
emitted and cached (under `pr-patch-v1\n{merge_base}\n{head}\n{path}`, 1 MiB per-entry
ceiling, never cached when truncated), and the cut section is held back as a fallback so a
later retry can fetch it alone with the full 8 MiB budget to itself. A batch therefore
degrades by shrinking, never by corrupting.

## Context radius as a rendering mode

### What context is for

A unified hunk carries `U` lines of unchanged context on each side of every run of changes,
three by default. Context serves two masters. For a human it is orientation: enough
surrounding code to recognize where the change lives. For `git apply` and `patch` it is an
addressing mechanism: a patch is applied by *searching* for the context, which is what lets a
patch apply against a file that has drifted since the diff was taken. More context means more
robust application but bigger patches; `-U0` means positional application only.

Context also determines hunk *fusion*. Two runs of changes separated by no more than
`2 * U` unchanged lines cannot each keep their own full context without overlapping, so the
diff merges them into one hunk. Growing `U` therefore monotonically decreases the hunk count
until, at a radius at least as large as the file, every change in the file fuses into exactly
one hunk that happens to contain every line of the file.

### `--unified=1000000` as the expanded view

Quinjet's diff pane has two display modes: the normal patch view and an "expanded" view that
shows the entire file with the changes highlighted in place. A tool with its own diff engine
would implement the second mode by reading both blobs and merging them against the computed
edit script. Quinjet gets it from the flag it already has: `expanded` selects
`--unified=1000000` instead of `--unified=3` in `revision_diff_file` (src/git/mod.rs:618-643),
and one million lines of context exceeds any file it will meet, so the patch that comes back
*is* the whole file, one fused hunk, with `-` and `+` rows interleaved at the right places and
every other row a context row.

The elegance is that nothing downstream changes. The same `parse_diff` walks the same
prefixes; context rows get both line numbers from the same counters seeded by the single
`@@ -a,b +c,d @@` header; the same document model, row layout cache, and viewport renderer
apply. Expanded view is not a feature of the renderer at all; it is a parameter of the
subprocess. The trade is paid in bytes, exactly where the caps already look: an expanded patch
is the size of the file plus the change, so large files hit the 8 MiB read cap and the 512 KiB
syntax-highlighting budget sooner in expanded mode, and both degrade gracefully (truncation
notice, plain spans) rather than failing.

Three other corners of the codebase reuse the same trick, so the two context radii are the
only patch shapes in the system: root commits via `git show --format= ... --unified=N`
(`root_commit_diff_file`, src/git/mod.rs:645-669), stashes via a two-part read of the tracked
diff plus the untracked stash commit within one shared byte budget (`stash_diff_file`,
src/git/mod.rs:671-729), and working-tree changes with optional `--cached` and `--cc`
(`raw_diff_for_change`, src/git/mod.rs:759-788). The PR path deliberately has no expanded
mode: batched PR reads always use `--unified=3` (`diff_selected_paths`), because expanded
bodies would blow the byte estimates the prefetch scheduler prices batches with.

### Why not always fetch expanded

Fetching `--unified=1000000` unconditionally and folding context away at render time would
halve the command shapes and make mode switches free. It loses on every axis Quinjet
prioritizes:

- Patch bytes scale with file size instead of change size, so the 8 MiB cap starts truncating
  routine diffs of large files even for one-line changes, and each PR batch would carry whole
  files for all 32 paths.
- The parsed-document memory budget (32 MiB across prefetched PR documents, evicted oldest
  first, src/app.rs) would hold entire files per document instead of hunks, cutting how many
  files stay warm by orders of magnitude.
- Syntax highlighting cost is per parsed line, and the 512 KiB per-patch grammar budget would
  be exhausted by context that is mostly never scrolled to.

So the mode is a flag, the flag is chosen per request, and switching modes re-runs one
path-scoped subprocess whose result is cached in the workspace like any other document
(invariant 8).

## Rename detection in the Quinjet pipeline

### Where `--find-renames` runs

Rename detection has to run wherever records are produced, or shapes stop lining up. Quinjet
passes `--find-renames` in every one of the reads that feed a diff view:

- the index read (`diff_index_args`, above), so renamed files arrive as single `R` records
  with two paths;
- the numstat read (derived by `numstat_args`, so automatically consistent);
- every patch read, single-file and batched, so the patch presents `rename from`/`rename to`
  headers instead of a delete-plus-add pair;
- the PR file listing and PR numstat (`changed_files_in_repository`, `numstat_counts` in
  src/git/github/mod.rs), so the PR index agrees with the patches that later stream in.

The default 50 percent similarity threshold is used everywhere; no invocation tightens or
loosens it. That means Quinjet's notion of "renamed" is exactly Git's default notion, and a
pair Git scores at 49 percent shows as a delete and an add in the tree, the header list, and
the patch alike, consistently.

### Parsing the three rename shapes

Each output format encodes a rename differently, and each parser handles its shape
explicitly:

**1. Name-status records.** With `-z`, a rename or copy is *three* NUL-terminated records:
the status (`R` or `C` with a score), then the pre-image path, then the post-image path.
`diff_index_files` (src/git/mod.rs:519-577) consumes one path record normally and two when
the status byte is `R` or `C`, producing a `DiffFileIndexEntry` whose `path` is the
post-image and `old_path` the pre-image. The status byte maps through `diff_status_label`
(src/git/mod.rs:1355-1366) to the lowercase label (`renamed`, `copied`) the header renders.

**2. Numstat records.** With `-z`, a renamed file's counts record carries an *empty* path
field, and the two real paths follow as separate records. The doc comment on `parse_numstat`
in `src/git/diff.rs` names the trap, and the scanner consumes accordingly:

```rust
/// Parse `git diff --numstat -z` output into per-path totals. Renames emit an
/// empty path field followed by the pre-image and post-image records, so the
/// scanner has to consume those two extra records instead of assuming one.
pub(crate) fn parse_numstat(output: &[u8]) -> HashMap<PathBuf, DiffLineCounts> {
```

When the path field is empty, the scanner reads the next two records, keys the entry by the
*new* path (matching how the index stores renames), and advances its cursor past both. A
subtle byte-level detail rides along: each record is split by `splitn(3, b'\t')`, at most two
tab splits, so a path that itself contains tab characters survives intact; the test
`reads_numstat_totals_for_plain_renamed_and_binary_paths` (src/git/diff.rs:1062-1095) pins
both behaviors.

**3. Patch headers.** `parse_diff` recognizes `rename from ` and `rename to ` lines, decoding
each path with `decode_git_path` (quoted C-style escapes with octal sequences, for the cases
where quoting appears despite `core.quotepath=false`) and setting the file builder's status
to `renamed`. The `similarity index` line, by contrast, is deliberately *dropped* along with
`index` lines as transport noise: the score is an artifact of the detector's threshold
arithmetic, useful to `git apply`, meaningless to a reader. A rename with no content changes
produces zero rows, and `flush_file` (src/git/diff.rs:729-757) injects the meta row
"File renamed without content changes" so the file still renders as something rather than as
an empty section.

### Keeping both names addressable

A rename gives every file two names, and different layers naturally hold different ones. Three
mechanisms keep the pipeline coherent:

- **The index carries both.** `DiffFileIndexEntry::label()` (src/git/diff.rs:116-129) renders
  the post-image path plus a `· renamed from <old>` annotation, so the header communicates
  the pairing without the user opening the patch.
- **Path-scoped diffs pass both.** A single-file diff for a renamed file must let the engine
  see the pre-image path inside its pathspec, or detection cannot pair the sides and the
  "rename" degenerates into a delete of an invisible file plus an add. From
  `src/git/mod.rs`:

```rust
fn append_diff_file_paths(args: &mut Vec<OsString>, file: &DiffFileIndexEntry) {
    if let Some(old_path) = &file.old_path {
        args.push(old_path.as_os_str().to_owned());
    }
    args.push(file.path.as_os_str().to_owned());
}
```

- **Batch sections match under either name.** When a combined patch is split,
  `PatchSection::matches` (src/git/diff.rs:672-676) compares the requested path against both
  header paths, so a batch requested by post-image paths still claims sections whose
  `diff --git` header leads with the pre-image side. The test
  `splits_a_batched_patch_into_one_section_per_file` (src/git/diff.rs:1097-1120) covers the
  rename case, including a path containing spaces.

One more parser subtlety earns its complexity budget here: `diff_header_paths`
(src/git/diff.rs:759-766) splits the `diff --git a/... b/...` remainder at the *last*
occurrence of `" b/"` rather than the first, because the old path may itself contain the byte
sequence ` b/`. Rename headers with exotic paths are exactly where such casework pays off,
and it is casework a from-scratch implementation would have to rediscover one bug report at a
time.

### Renames and the API-count path

The PR pipeline's API-sourced counts (the pulls files endpoint) report status strings rather
than letters, including `renamed`, with `previous_filename` semantics mirrored into the same
`old_path` field of the index entry. One filtering rule differs from the local path:
`parse_api_file_counts` (src/git/github/mod.rs:1918-1943) drops records reporting 0 additions
and 0 deletions *unless* the status is `renamed`, because a pure rename legitimately has zero
line changes while a zero/zero record for any other status is a pure mode change that would
otherwise fake exact counts. The local numstat path needs no such rule; Git's own records
already distinguish the cases.

## Quinjet's own diff-shaped computation

### The one place Quinjet diffs anything

Intraline emphasis is the VS Code style highlight inside a changed line pair: when a removed
line and an added line are versions of each other, the region where they disagree gets a
tinted background so the eye lands on the actual edit. This is the single computation in the
codebase that resembles a diff algorithm, it lives entirely in the render layer
(src/ui/mod.rs), and every design decision in it inverts the trade-offs of the general
algorithms above, because its constraints are inverted too:

| Constraint | `git diff` | Intraline emphasis |
| --- | --- | --- |
| Runs | once per patch, on a worker thread | every frame, on the UI thread |
| Input | whole files | one pair of lines |
| Output | must be a correct, applicable patch | purely decorative |
| Wrong answer costs | corrupted patch | slightly off highlight |
| Budget | seconds are tolerable | a frame is 16 ms for everything |

A decorative computation on the frame path buys accuracy with latency, which is the wrong
currency. So Quinjet does not run Myers here, or a word LCS, or patience over tokens. It runs
the cheapest analysis that is right in the common case and provably bounded in every case.

### Positional pairing, not LCS pairing

The first sub-problem is deciding *which* removed line pairs with which added line. The
unified format groups a replacement as a run of `-` lines followed by a run of `+` lines, so
the pairing question is: within one such block, which old line is which new line's ancestor?
A general answer would diff the two runs against each other (a nested line-level LCS over
line similarity scores, which is roughly what side-by-side views in some editors compute).
Quinjet's answer is positional: the i-th removed line pairs with the i-th added line, and
surplus lines on the longer side pair with nothing. The block structure is captured by
`EmphasisBlock`, from `src/ui/mod.rs`:

```rust
struct EmphasisBlock {
    removed_start: usize,
    added_start: usize,
    added_end: usize,
}

impl EmphasisBlock {
    const fn contains(&self, index: usize) -> bool {
        self.removed_start <= index && index < self.added_end
    }
}
```

`emphasis_block` (src/ui/mod.rs:4554-4579) builds one from any removed or added row by
walking outward over runs of the same kind: from a removed row it finds the start of the
removed run, the end of that run (which is where the added run starts), and the end of the
following added run; from an added row it walks back through the added run and then the
removed run that precedes it. The walk is linear in the block size and needs no allocation.

Positional pairing is exactly right whenever the block is a stack of line edits, which is what
editing produces: change three consecutive lines and the diff emits three `-` then three `+`,
i-th with i-th. It is wrong when a block mixes an edit with an insertion above it, e.g. one
removed line whose true partner is the *second* added line; the emphasis then compares the
wrong pair, finds them mostly different, and highlights a wide range or, on the surplus line,
nothing. The failure is cosmetic and self-limiting: the fewer lines pair sensibly, the less
emphasis appears, and the plain added/removed row coloring still communicates the change.
Buying the mixed case would cost a similarity matrix per block per frame; the common case
costs two index subtractions.

The side-by-side layout makes the same positional choice structural:
`side_by_side_rows` (src/ui/mod.rs:4446-4522) measures a removed run and its following added
run and emits `max(removed_len, added_len)` split rows pairing i-th with i-th, `None` filling
the exhausted side. In that layout the pair *is* the row, so the split renderer computes
emphasis directly per visible row with no block cache at all.

### The range computation: common prefix, common suffix

The second sub-problem is finding the changed region inside a pair. Quinjet computes the
longest common prefix and the longest common suffix and marks whatever lies between them.
The whole algorithm, verbatim from `src/ui/mod.rs`:

```rust
fn changed_ranges(old: &str, new: &str) -> (Option<Range<usize>>, Option<Range<usize>>) {
    let mut prefix = 0;
    for ((old_index, old_character), (new_index, new_character)) in
        old.char_indices().zip(new.char_indices())
    {
        if old_character != new_character || old_index != new_index {
            break;
        }
        prefix = old_index + old_character.len_utf8();
    }

    let mut old_end = old.len();
    let mut new_end = new.len();
    for ((old_index, old_character), (new_index, new_character)) in
        old.char_indices().rev().zip(new.char_indices().rev())
    {
        if old_character != new_character || old_index < prefix || new_index < prefix {
            break;
        }
        old_end = old_index;
        new_end = new_index;
    }

    (
        (prefix < old_end).then_some(prefix..old_end),
        (prefix < new_end).then_some(prefix..new_end),
    )
}
```

Reading it against the theory: this is the "strip common prefix and suffix" preprocessing step
that every real diff implementation performs before its search, promoted to being the *entire*
algorithm. What a full diff would do next (find common content *inside* the changed region) is
skipped, so the result is always zero or one contiguous range per side. In edit-graph terms,
it rides the initial snake from `(0, 0)` and the final snake into `(N, M)` and declares
everything between them changed, an edit script that is correct but generally not shortest.

Four details in those twenty lines carry real weight:

- **Char-wise, byte-addressed.** The loops iterate characters (so a range can never split a
  UTF-8 sequence) but accumulate byte offsets (so the renderer can slice spans without
  re-measuring). `prefix` advances by `len_utf8()` past each matched character.
- **The index-equality guard.** The forward loop breaks when `old_index != new_index`, not
  only on character mismatch. `zip` over two `char_indices()` streams can drift after a
  mismatch of differently sized characters; requiring equal byte positions makes "common
  prefix" mean *byte-identical* prefix, which is the only sound meaning for downstream byte
  slicing.
- **The crossing guard.** The suffix loop refuses to walk past the prefix boundary
  (`old_index < prefix || new_index < prefix` breaks), which is what prevents prefix and
  suffix from claiming overlapping bytes when one line is a substring-like variant of the
  other.
- **Empty means none.** A side whose range would be empty reports `None`, so a pure
  in-line insertion emphasizes only the added side, and identical lines emphasize nothing.

The complexity is `O(len)` per pair with no allocation beyond the two line strings themselves
(materialized by `DiffLine::text()` from the highlight spans), against `O((N + M) * D)` plus
tokenization and allocation for a word-level LCS. On the worked example from the test suite
(src/ui/mod.rs:8745-8755): `"const oldValue = 1;"` versus `"const newValue = 2;"` yields
`(Some(6..18), Some(6..18))`, the region spanning `oldValue = 1` and `newValue = 2`, because
the common prefix `const ` is 6 bytes and the common suffix `;` starts at byte 18 on both
sides. A word LCS would additionally recover `Value = ` as common and emphasize only `old`
versus `new` and `1` versus `2`; the prefix/suffix answer is coarser, one span instead of
two, and in a terminal cell grid that coarser answer reads perfectly well.

### The char-level slider, resolved greedily

The slider ambiguity from the line-level theory recurs at character level. Take
`old = "aaa"` and `new = "aaaa"`: the inserted `a` could be marked at any of four positions,
all producing the same string. `changed_ranges` resolves it deterministically: the forward
loop greedily consumes all three matching `a`s (`prefix = 3`), the suffix loop immediately
hits the crossing guard (its first candidate byte on the old side is index 2, already inside
the prefix), and the result is `(None, Some(3..4))`: the *last* `a` is marked as inserted.
Prefix greed always anchors ambiguous repeats to the rightmost placement. Git's indent
heuristic solves the analogous line-level problem with corpus-derived scoring because hunk
placement changes how a whole patch reads; a one-cell highlight placement does not justify a
scoring model, but it does justify determinism, which the greedy rule provides.

### Viewport scoping and the pair budget

The remaining question is when this runs. The answer, introduced with the optimization stack's
first PR (#46) and pinned by tests since, is: only for rows on screen this frame.
`draw_unified_diff` passes exactly the visible window of row indices into
`visible_intraline_emphasis` (src/ui/mod.rs:4581-4626), which walks those indices, reuses the
current `EmphasisBlock` while indices stay inside it, computes `pair_count` as the shorter
run's length, skips surplus rows, and calls the pair function only for rows that pair. Cost
scales with the viewport height, never with the document: a million-row document pays for
the ~50 visible rows. One subtlety keeps correctness at the window's edges: the block is
computed over the full line array, not the visible slice, so a visible added line finds its
removed partner even when the partner is scrolled off screen (the test
`visible_intraline_emphasis_matches_block_pairing`, src/ui/mod.rs:8645-8670, covers exactly
this).

Above the range computation sits a hard input bound, from `paired_intraline_emphasis`
(src/ui/mod.rs:4628-4650): the spans' byte lengths are summed first, and if either side
exceeds `MAX_INTRALINE_SOURCE_BYTES = 32 * 1024` the function returns `(None, None)` without
materializing text at all. A minified one-line JavaScript bundle diffed against its next
version would otherwise dominate the frame; under the cap it costs two integer sums and a
comparison per frame. The guard test at src/ui/mod.rs:8631 feeds two strings one byte over
the cap and asserts the bail-out.

Rendering closes the loop without further computation: `highlight_spans`
(src/ui/mod.rs:4751-4838) intersects the byte range with each stored highlight span,
splitting a span into before/changed/after pieces and giving the changed piece the
`added_emphasis_background` or `removed_emphasis_background` (a 27 percent blend of the
add/remove hue over the base background, src/theme.rs:179-181). The emphasis is a range over
bytes the spans already contain, so syntax coloring and emphasis compose without either
recomputing the other; the full render mechanics live in
[./intraline-and-highlighting.md](./intraline-and-highlighting.md) and
[../rendering/viewport.md](../rendering/viewport.md).

### Reading the whole design at once

Stacked up, the intraline path is a pipeline of refusals: refuse non-adjacent pairings
(positional blocks), refuse oversized inputs (32 KiB cap), refuse offscreen work (viewport
scoping), refuse sub-region recovery (prefix/suffix only), refuse allocation where a borrow
serves. Each refusal is individually small; together they turn "run a diff per changed line
pair per frame" from an obvious performance hazard into a rounding error, while the answers
stay exact in the case that dominates real diffs (one edited region per line). It is the
page's thesis in miniature: Quinjet's performance work is rarely a faster algorithm and
almost always a smaller, better-bounded problem.

## Design alternatives and why they lost

**1. An in-process diff engine.** Rust has mature diff crates implementing Myers, patience,
and histogram over arbitrary sequences, and an in-process engine would eliminate subprocess
spawns and patch parsing entirely. It lost on authority, not on speed. The hunks would be
computed by a second implementation with its own tie-breaking, its own heuristic set, and no
knowledge of the user's `diff.algorithm`, so the TUI would routinely disagree with
`git diff` at the same prompt; rename detection, binary detection, attribute handling, and
combined-diff support would all need reimplementation to reach feature parity; and the
subprocess cost it saves is already amortized by batching (one spawn per 32 files) and by the
three-read design that avoids patch computation for everything not displayed. The general
project preference for established implementations points the same way: the most established
diff implementation available is Git's own.

**2. Reading blobs and diffing on demand for counts.** Per-file `+n -n` totals could come
from materializing both blobs and counting locally, avoiding the second `--numstat`
subprocess. In the PR workspace this is precisely wrong: the workspace is a `blob:none`
partial clone, so touching blob content triggers lazy network fetches per file, and "GitHub
already knows the totals" (src/git/github/mod.rs:1235-1237). Locally it would mean paying
blob inflation for every listed file when the fused `--numstat` read answers for all files in
one bounded subprocess. Counts are metadata; both pipelines treat them as such.

**3. Word-level LCS for intraline emphasis.** A tokenizing LCS recovers multiple changed
spans per line and is what editor diff views compute. It lost to the frame budget: it
allocates token vectors and a search structure per pair, its cost grows with `D` per pair,
and its extra fidelity (splitting one highlight into two) is marginal at terminal-cell
resolution. The prefix/suffix computation is allocation-light, strictly linear, and exact for
single-region edits. The block cache plus 32 KiB cap plus viewport scoping were designed
around that cost model; a heavier per-pair algorithm would need its own memoization layer to
survive scrolling, adding state where the current design has none.

**4. Similarity-scored pairing inside replacement blocks.** Pairing removed to added lines by
content similarity (as some side-by-side tools do) handles blocks that mix edits with
insertions. It lost for the same reason at block scale: a `d * a` scoring matrix per visible
block per frame, to improve a decorative signal in the minority case. The positional rule is
constant-time per row and structurally shared with the side-by-side layout, so both layouts
agree about which lines are partners.

**5. `git diff --word-diff` as the emphasis source.** Git already computes word diffs, so the
render layer could parse them instead of computing ranges. It lost three times over: word
diff output is not an applicable patch, so the normal patch read would still be needed and
every file would cost two subprocesses; the output shape depends on per-repository
`wordRegex` configuration, reintroducing variability the parser cannot control; and the
emphasis would be frozen at parse time rather than computed from whatever rows are actually
paired on screen, which is what keeps the collapsed/expanded and unified/side-by-side views
consistent with each other.

**6. Precomputing emphasis at parse time.** Computing ranges once per document instead of per
frame trades CPU for memory and staleness. It lost because the pairing is a *layout* property
(it depends on which rows are adjacent after hunk-header removal and fold state), because
documents are cached and re-rendered across theme and layout changes, and because the
per-frame cost after viewport scoping is already negligible: recomputing ~50 bounded pairs
per frame is cheaper than keeping per-document range tables coherent through every fold
toggle and eviction.

**7. Shipping a custom patch format.** Since Quinjet controls both the producer flags and the
parser, it could ask Git for a more machine-friendly format (or use `--porcelain`-style
plumbing per file). It lost because unified diff *is* the machine-friendly format: it is the
one shape every Git version emits identically, it batches naturally (`diff --git` boundaries
are the split points), it carries rename and binary metadata inline, and its pathologies
(quoting, tabs, `\ No newline`) are finite, documented, and already handled. A private format
would buy parser simplicity that the 1,394-line `src/git/diff.rs` demonstrably does not
need, at the price of a second code path for every producer.

## Failure modes and edge cases

The algorithms above are clean; the byte streams that carry their results are not. This
section catalogs the ways real diff data misbehaves and the specific mechanism that absorbs
each one. Most of these behaviors are pinned by tests in `src/git/diff.rs` and
`src/ui/mod.rs`, which makes the section double as a map of the parsing contract.

### Truncation at every boundary

Every read can be cut by its byte cap, and each layer repairs the cut at the boundary its
format requires:

- **Patch reads** are popped back byte by byte until the buffer ends in `\n`
  (`truncate_to_complete_line`, src/git/mod.rs:1554-1558; the same loop appears inline in
  `diff_selected_paths`). A parser fed half a line could misread a marker byte; a parser fed
  whole lines merely misses the tail. The resulting document carries `truncated: true` and
  appends the meta row "… diff truncated to keep Quinjet responsive …" so the reader knows
  the view is a prefix.
- **Listing reads** are cut back to the last complete NUL record (`truncate_diff_index`,
  src/git/mod.rs:1341-1353), because a name-status record split mid-path would otherwise
  fabricate a bogus file. Index parsing additionally stops with `truncated = true` at
  `MAX_DIFF_INDEX_FILES = 16,384` entries or on a short record run, so a listing that lies
  about its own shape degrades to a shorter honest one.
- **Batched patch reads** localize the damage: after `split_patch_by_file`, only the final
  section of a capped batch can be incomplete, so every earlier file still parses, renders,
  and enters the per-file cache. The incomplete section is retried alone later, with the
  entire 8 MiB budget to itself (src/git/github/mod.rs:487-515).
- **Stash reads** split one budget across two commands: the tracked diff runs first under
  `MAX_DIFF_BYTES`, and the untracked part (`git show` of the `^3` stash commit) may only
  spend `MAX_DIFF_BYTES.saturating_sub(output.len())`, the remainder
  (`stash_diff_file`, src/git/mod.rs:671-729). Two independently capped reads could sum to
  double the cap; a shared budget cannot.

The counts layer makes truncation of its own read a non-event by construction: a failed or
bounded `--numstat` leaves `counts: None` on the affected entries, headers show the
`+·· -··` placeholder, and the document is otherwise unaffected, because counts are "a
rendering enhancement, never a correctness requirement" (src/git/mod.rs:511-517).

### The missing final newline

Unified diff marks a file whose last line lacks a trailing newline with a dedicated marker
line immediately after the affected `+` or `-` line:

```text
\ No newline at end of file
```

The marker starts with a backslash, which is none of the content prefixes, so `parse_diff`'s
fall-through arm turns it into a `Meta` row: displayed, inert, never counted as an addition
or deletion. Quinjet also *produces* the marker: `untracked_patch` (src/git/mod.rs:1122-1165)
fabricates a valid unified diff for files Git does not yet track, and when the file's content
does not end in `\n` it appends the marker exactly as Git would, so the synthesized patch is
indistinguishable in shape from a real one and flows through the same parser with no special
case.

### Paths that fight the format

Paths are the format's soft underbelly, because they are user-controlled bytes embedded in a
line-oriented text stream:

- **Quoting.** Quinjet runs every Git command with `-c core.quotepath=false`, so non-ASCII
  paths arrive as raw bytes rather than C-style quoted strings. Quoted paths can still
  appear in `diff --git` headers, so `decode_git_path` (src/git/diff.rs:778-813) handles
  them anyway: double-quoted values are unescaped byte-wise, including `\n`, `\r`, `\t`,
  `\"`, up-to-three-digit octal escapes accumulated with saturating arithmetic, and a
  tolerated trailing lone backslash. Belt and suspenders: configuration removes the common
  case, the decoder survives the rest.
- **Spaces and the header split.** A `diff --git a/old b/new` header has no unambiguous
  separator when paths contain spaces. `diff_header_paths` (src/git/diff.rs:759-766) splits
  at the *last* occurrence of the `b/`-prefixed field marker rather than the first, so an
  old path containing that byte sequence still parses; the batched-split test includes a
  renamed path with spaces to hold the line.
- **Tabs.** `--numstat` is tab-separated, so a path containing tabs would shear a naive
  split. `parse_numstat` splits each record with `splitn(3, b'\t')`, at most two cuts, so
  everything after the second tab is the path, tabs and all. NUL-terminated formats dodge
  the whole class, which is why every listing Quinjet requests passes `-z`.
- **Non-UTF-8 bytes.** Both parsers convert path bytes with `String::from_utf8_lossy`,
  trading exactness for totality: an invalid sequence renders as the replacement character
  instead of failing the parse. The architecture document lists true non-UTF-8 path
  preservation among deliberate next steps; today's contract is "never crash, always
  display something addressable".
- **CRLF content.** Patch lines are split with Rust's `str::lines`, which treats `\r\n` as
  a line ending and strips the `\r`, so Windows-encoded content does not render with
  phantom trailing characters, and byte offsets used downstream (highlighting, emphasis)
  are computed over the stripped text consistently.

### Hunk arithmetic and combined diffs

Line numbers in the gutter are not stored anywhere in the patch except the hunk headers;
everything else is counting. `parse_hunk_starts` reads only the two start fields from
`@@ -a,b +c,d @@` and tolerates the format's abbreviations: a one-line range may omit its
count (`-12` instead of `-12,1`), which `parse_range_start` handles by taking the digits
before the first comma, and a malformed field simply yields `None`, turning the numbers off
for that hunk rather than guessing. Content rows that arrive before any hunk header (which a
malformed patch could contain) carry `None` numbers for the same reason: the counters are
`Option<usize>` precisely so that "unknown" is representable and renders as blank rather
than as a wrong number.

Combined diffs (`diff --cc`, produced for conflicts and merges) bend the format further:
one file against multiple parents, multi-column markers, `@@@` hunk headers. Quinjet meets
them where it needs them: the working-tree conflict view requests `--cc` explicitly
(`raw_diff_for_change`), and `parse_diff` recognizes `diff --cc ` and `diff --combined `
headers, decoding the single path that serves as both sides of the section so the file
groups, labels, and sorts correctly. The `@@` prefix match covers `@@@` headers too, so
combined hunks still break sections at the right places; the parser's single-column line
model then renders combined marker columns conservatively as content rather than
attempting per-parent attribution, which is the honest floor for a display whose row model
is two-sided.

### When there is nothing to show

Absence is a first-class output, and each empty case gets a distinct, deliberate message
instead of a blank pane:

| Case | Produced by | Row shown |
| --- | --- | --- |
| Empty patch bytes | `parse_diff` on empty input | "No textual diff to display" |
| Headers parsed, no rows at all | `parse_diff` after flushing | "No file changes to display" |
| Rename with no content change | `flush_file` on a zero-row renamed file | "File renamed without content changes" |
| Non-rename file with no rows | `flush_file` | "No textual changes to display" |
| Empty index | `document_with_visibility` | "No file changes to display" |
| Patch not yet loaded, file visible | skeleton assembly | "Loading diff…" |
| Patch not loaded, file collapsed | skeleton assembly | "Expand this file to load its diff" |

The last two rows are where this page hands off to the loading architecture: the skeleton
document is assembled from the bounded index with per-file placeholders, and patches replace
placeholders as they arrive, which is the mechanism behind progressive loading of huge PR
views ([../rendering/progressive-loading.md](../rendering/progressive-loading.md)).

### Unknown lines and forward compatibility

`parse_diff` is a first-match-wins ladder over line prefixes with two default arms: lines
before the first `diff` header are dropped entirely (this is what silently absorbs
`commit`/`Author:` preambles when a `git show` style patch is fed through), and any
unrecognized non-empty line inside a file becomes a `Meta` row. The second arm is the
forward-compatibility valve: when a future Git emits an extended header the parser has never
heard of, the line renders as visible metadata instead of breaking the parse or vanishing.
The lines deliberately routed to that arm today (`old mode` / `new mode` pairs, binary
notices) confirm the pattern: unknown means shown, not fatal. The lines deliberately
*dropped* (`index`, `similarity index`) are the ones whose information is either transport
detail or already surfaced through the index entry's status and label.

### Intraline edge cases

The emphasis path has its own corner catalog, all resolved toward "less emphasis" rather
than "wrong emphasis":

- Identical paired lines yield `(None, None)`: no range, no tint, even though the pair was
  positionally eligible.
- A pure insertion inside a line yields `None` on the old side and a range on the new side
  only, so nothing is highlighted that did not change.
- The crossing guard prevents prefix and suffix from overlapping when one line extends the
  other (`"aaa"` to `"aaaa"` marks exactly one trailing byte, deterministically the last).
- Either side over 32 KiB kills the computation for that pair before any text is
  materialized; surplus unpaired lines in a lopsided block are skipped by the
  `pair_index >= pair_count` check.
- Ranges respect UTF-8 scalar boundaries by construction but not grapheme clusters: a pair
  differing only in a combining mark emphasizes the mark's bytes, not the whole visual
  glyph. Rendering absorbs this benignly because display slicing is width-aware and
  zero-width scalars occupy no cells.
- Hunk headers never interfere with pairing because they are removed from the row stream
  before layout (`unified_row_indices` skips them), so a removed run and its added run are
  adjacent in the layout even when the raw patch interleaved a header between hunks.

### File ordering

A multi-file patch renders in a stable order that is chosen, not inherited: `parse_diff`
sorts file sections by full repository path, case-sensitively, byte-wise, with backslashes
normalized to forward slashes (`FileBuilder::sort_path`, sorted at src/git/diff.rs:588). The
pinned expectation (test `sorts_files_by_case_sensitive_full_repository_path`) places
`.github/ISSUE_TEMPLATE/bug.yml` before `.github/labeler.yml`, and `CODE_OF_CONDUCT.md`
before `Cargo.toml` before `README.md` before `src/app.rs`: uppercase before lowercase,
directories interleaved purely by their byte sequence. Byte order was chosen over
human-friendly collation because it is total, locale-independent, and identical on every
platform, so a cached document and a freshly parsed one can never disagree about order, and
because it matches the index ordering Git itself uses for trees
([../git-internals/object-model.md](../git-internals/object-model.md)). The batched PR path
does not rely on this sort; it assembles documents in index order via
`document_with_visibility`, so both assembly routes present files in a deterministic order
of their own layer.

## Where to go next

- [./README.md](./README.md): the diff group hub and reading order.
- [./pipeline.md](./pipeline.md): the unified-diff format byte by byte, the document model,
  collapsed-headers-first indexing, and batching mechanics.
- [./intraline-and-highlighting.md](./intraline-and-highlighting.md): the emphasis render
  path in full, syntect grammars, and the highlighting budgets.
- [../git-internals/plumbing-and-porcelain.md](../git-internals/plumbing-and-porcelain.md):
  the complete catalog of Git invocations and the parsers behind them.
- [../git-internals/merge-bases-and-history.md](../git-internals/merge-bases-and-history.md):
  why a PR diff is a merge-base comparison and how the base is resolved.
- [../git-internals/packfiles-and-deltas.md](../git-internals/packfiles-and-deltas.md):
  binary deltas at rest and on the wire.
- [../github/prefetch.md](../github/prefetch.md): the batch scheduler that consumes the
  count estimates, including the #50 to #55 ordering evolution.
- [../rendering/progressive-loading.md](../rendering/progressive-loading.md): how skeleton
  documents and streaming patches compose into the huge-PR loading experience.
- [../techniques.md](../techniques.md): the cross-cutting technique catalog, several entries
  of which (byte-budgeted batching, capped pipes, viewport-scoped computation) this page
  derived from first principles.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Diff Algorithms in a small local repository | Record time to first useful rows |
| 2 | Check latency for Diff Algorithms in a small local repository | Record steady frame cost |
| 3 | Check latency for Diff Algorithms in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Diff Algorithms in a small local repository | Record Git and gh process count |
| 5 | Check latency for Diff Algorithms in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Diff Algorithms in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Diff Algorithms in a small local repository | Record stale reply rejection |
| 8 | Check latency for Diff Algorithms in a small local repository | Record visible state after failure |
| 9 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Diff Algorithms in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Diff Algorithms in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Diff Algorithms in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Diff Algorithms in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Diff Algorithms in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Diff Algorithms in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Diff Algorithms in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Diff Algorithms in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Diff Algorithms in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Diff Algorithms in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for Diff Algorithms in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for Diff Algorithms in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for Diff Algorithms in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for Diff Algorithms in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for Diff Algorithms in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for Diff Algorithms in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for Diff Algorithms in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for Diff Algorithms in an unavailable network | Record time to first useful rows |
| 34 | Check latency for Diff Algorithms in an unavailable network | Record steady frame cost |
| 35 | Check latency for Diff Algorithms in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for Diff Algorithms in an unavailable network | Record Git and gh process count |
| 37 | Check latency for Diff Algorithms in an unavailable network | Record maximum retained document bytes |
| 38 | Check latency for Diff Algorithms in an unavailable network | Record cache disposition and complete key |
| 39 | Check latency for Diff Algorithms in an unavailable network | Record stale reply rejection |
| 40 | Check latency for Diff Algorithms in an unavailable network | Record visible state after failure |
| 41 | Check latency for Diff Algorithms in rapid keyboard navigation | Record time to first useful rows |
| 42 | Check latency for Diff Algorithms in rapid keyboard navigation | Record steady frame cost |
| 43 | Check latency for Diff Algorithms in rapid keyboard navigation | Record bytes accepted from child output |
| 44 | Check latency for Diff Algorithms in rapid keyboard navigation | Record Git and gh process count |
| 45 | Check latency for Diff Algorithms in rapid keyboard navigation | Record maximum retained document bytes |
| 46 | Check latency for Diff Algorithms in rapid keyboard navigation | Record cache disposition and complete key |
| 47 | Check latency for Diff Algorithms in rapid keyboard navigation | Record stale reply rejection |
| 48 | Check latency for Diff Algorithms in rapid keyboard navigation | Record visible state after failure |
| 49 | Check latency for Diff Algorithms in a linked worktree | Record time to first useful rows |
| 50 | Check latency for Diff Algorithms in a linked worktree | Record steady frame cost |
| 51 | Check latency for Diff Algorithms in a linked worktree | Record bytes accepted from child output |
| 52 | Check latency for Diff Algorithms in a linked worktree | Record Git and gh process count |
| 53 | Check latency for Diff Algorithms in a linked worktree | Record maximum retained document bytes |
| 54 | Check latency for Diff Algorithms in a linked worktree | Record cache disposition and complete key |
| 55 | Check latency for Diff Algorithms in a linked worktree | Record stale reply rejection |
| 56 | Check latency for Diff Algorithms in a linked worktree | Record visible state after failure |
| 57 | Check latency for Diff Algorithms in cold and warm cache states | Record time to first useful rows |
| 58 | Check latency for Diff Algorithms in cold and warm cache states | Record steady frame cost |
| 59 | Check latency for Diff Algorithms in cold and warm cache states | Record bytes accepted from child output |
| 60 | Check latency for Diff Algorithms in cold and warm cache states | Record Git and gh process count |
| 61 | Check latency for Diff Algorithms in cold and warm cache states | Record maximum retained document bytes |
| 62 | Check latency for Diff Algorithms in cold and warm cache states | Record cache disposition and complete key |
| 63 | Check latency for Diff Algorithms in cold and warm cache states | Record stale reply rejection |
| 64 | Check latency for Diff Algorithms in cold and warm cache states | Record visible state after failure |
| 65 | Check peak memory for Diff Algorithms in a small local repository | Record time to first useful rows |
| 66 | Check peak memory for Diff Algorithms in a small local repository | Record steady frame cost |
| 67 | Check peak memory for Diff Algorithms in a small local repository | Record bytes accepted from child output |
| 68 | Check peak memory for Diff Algorithms in a small local repository | Record Git and gh process count |
| 69 | Check peak memory for Diff Algorithms in a small local repository | Record maximum retained document bytes |
| 70 | Check peak memory for Diff Algorithms in a small local repository | Record cache disposition and complete key |
| 71 | Check peak memory for Diff Algorithms in a small local repository | Record stale reply rejection |
| 72 | Check peak memory for Diff Algorithms in a small local repository | Record visible state after failure |
| 73 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record time to first useful rows |
| 74 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record steady frame cost |
| 75 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record bytes accepted from child output |
| 76 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record Git and gh process count |
| 77 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record maximum retained document bytes |
| 78 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record cache disposition and complete key |
| 79 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record stale reply rejection |
| 80 | Check peak memory for Diff Algorithms in a monorepo with many changed paths | Record visible state after failure |
| 81 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record time to first useful rows |
| 82 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record steady frame cost |
| 83 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record bytes accepted from child output |
| 84 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record Git and gh process count |
| 85 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record maximum retained document bytes |
| 86 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record cache disposition and complete key |
| 87 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record stale reply rejection |
| 88 | Check peak memory for Diff Algorithms in a pull request containing generated files | Record visible state after failure |
| 89 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record time to first useful rows |
| 90 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record steady frame cost |
| 91 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record bytes accepted from child output |
| 92 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record Git and gh process count |
| 93 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record maximum retained document bytes |
| 94 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record cache disposition and complete key |
| 95 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record stale reply rejection |
| 96 | Check peak memory for Diff Algorithms in a deeply diverged branch | Record visible state after failure |
| 97 | Check peak memory for Diff Algorithms in an unavailable network | Record time to first useful rows |
| 98 | Check peak memory for Diff Algorithms in an unavailable network | Record steady frame cost |
| 99 | Check peak memory for Diff Algorithms in an unavailable network | Record bytes accepted from child output |
| 100 | Check peak memory for Diff Algorithms in an unavailable network | Record Git and gh process count |
| 101 | Check peak memory for Diff Algorithms in an unavailable network | Record maximum retained document bytes |
| 102 | Check peak memory for Diff Algorithms in an unavailable network | Record cache disposition and complete key |
| 103 | Check peak memory for Diff Algorithms in an unavailable network | Record stale reply rejection |
| 104 | Check peak memory for Diff Algorithms in an unavailable network | Record visible state after failure |
| 105 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record time to first useful rows |
| 106 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record steady frame cost |
| 107 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record bytes accepted from child output |
| 108 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record Git and gh process count |
| 109 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record maximum retained document bytes |
| 110 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record cache disposition and complete key |
| 111 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record stale reply rejection |
| 112 | Check peak memory for Diff Algorithms in rapid keyboard navigation | Record visible state after failure |
| 113 | Check peak memory for Diff Algorithms in a linked worktree | Record time to first useful rows |
| 114 | Check peak memory for Diff Algorithms in a linked worktree | Record steady frame cost |
| 115 | Check peak memory for Diff Algorithms in a linked worktree | Record bytes accepted from child output |
| 116 | Check peak memory for Diff Algorithms in a linked worktree | Record Git and gh process count |
| 117 | Check peak memory for Diff Algorithms in a linked worktree | Record maximum retained document bytes |
| 118 | Check peak memory for Diff Algorithms in a linked worktree | Record cache disposition and complete key |
| 119 | Check peak memory for Diff Algorithms in a linked worktree | Record stale reply rejection |
| 120 | Check peak memory for Diff Algorithms in a linked worktree | Record visible state after failure |
| 121 | Check peak memory for Diff Algorithms in cold and warm cache states | Record time to first useful rows |
| 122 | Check peak memory for Diff Algorithms in cold and warm cache states | Record steady frame cost |
| 123 | Check peak memory for Diff Algorithms in cold and warm cache states | Record bytes accepted from child output |
| 124 | Check peak memory for Diff Algorithms in cold and warm cache states | Record Git and gh process count |
| 125 | Check peak memory for Diff Algorithms in cold and warm cache states | Record maximum retained document bytes |
| 126 | Check peak memory for Diff Algorithms in cold and warm cache states | Record cache disposition and complete key |
| 127 | Check peak memory for Diff Algorithms in cold and warm cache states | Record stale reply rejection |
| 128 | Check peak memory for Diff Algorithms in cold and warm cache states | Record visible state after failure |
| 129 | Check network transfer for Diff Algorithms in a small local repository | Record time to first useful rows |
| 130 | Check network transfer for Diff Algorithms in a small local repository | Record steady frame cost |
| 131 | Check network transfer for Diff Algorithms in a small local repository | Record bytes accepted from child output |
| 132 | Check network transfer for Diff Algorithms in a small local repository | Record Git and gh process count |
| 133 | Check network transfer for Diff Algorithms in a small local repository | Record maximum retained document bytes |
| 134 | Check network transfer for Diff Algorithms in a small local repository | Record cache disposition and complete key |
| 135 | Check network transfer for Diff Algorithms in a small local repository | Record stale reply rejection |
| 136 | Check network transfer for Diff Algorithms in a small local repository | Record visible state after failure |
| 137 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record time to first useful rows |
| 138 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record steady frame cost |
| 139 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record bytes accepted from child output |
| 140 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record Git and gh process count |
| 141 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record maximum retained document bytes |
| 142 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record cache disposition and complete key |
| 143 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record stale reply rejection |
| 144 | Check network transfer for Diff Algorithms in a monorepo with many changed paths | Record visible state after failure |
| 145 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record time to first useful rows |
| 146 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record steady frame cost |
| 147 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record bytes accepted from child output |
| 148 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record Git and gh process count |
| 149 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record maximum retained document bytes |
| 150 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record cache disposition and complete key |
| 151 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record stale reply rejection |
| 152 | Check network transfer for Diff Algorithms in a pull request containing generated files | Record visible state after failure |
| 153 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record time to first useful rows |
| 154 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record steady frame cost |
| 155 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record bytes accepted from child output |
| 156 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record Git and gh process count |
| 157 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record maximum retained document bytes |
| 158 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record cache disposition and complete key |
| 159 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record stale reply rejection |
| 160 | Check network transfer for Diff Algorithms in a deeply diverged branch | Record visible state after failure |
| 161 | Check network transfer for Diff Algorithms in an unavailable network | Record time to first useful rows |
| 162 | Check network transfer for Diff Algorithms in an unavailable network | Record steady frame cost |
| 163 | Check network transfer for Diff Algorithms in an unavailable network | Record bytes accepted from child output |
| 164 | Check network transfer for Diff Algorithms in an unavailable network | Record Git and gh process count |
| 165 | Check network transfer for Diff Algorithms in an unavailable network | Record maximum retained document bytes |
| 166 | Check network transfer for Diff Algorithms in an unavailable network | Record cache disposition and complete key |
| 167 | Check network transfer for Diff Algorithms in an unavailable network | Record stale reply rejection |
| 168 | Check network transfer for Diff Algorithms in an unavailable network | Record visible state after failure |
| 169 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record time to first useful rows |
| 170 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record steady frame cost |
| 171 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record bytes accepted from child output |
| 172 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record Git and gh process count |
| 173 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record maximum retained document bytes |
| 174 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record cache disposition and complete key |
| 175 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record stale reply rejection |
| 176 | Check network transfer for Diff Algorithms in rapid keyboard navigation | Record visible state after failure |
| 177 | Check network transfer for Diff Algorithms in a linked worktree | Record time to first useful rows |
| 178 | Check network transfer for Diff Algorithms in a linked worktree | Record steady frame cost |
| 179 | Check network transfer for Diff Algorithms in a linked worktree | Record bytes accepted from child output |
| 180 | Check network transfer for Diff Algorithms in a linked worktree | Record Git and gh process count |
| 181 | Check network transfer for Diff Algorithms in a linked worktree | Record maximum retained document bytes |
| 182 | Check network transfer for Diff Algorithms in a linked worktree | Record cache disposition and complete key |
| 183 | Check network transfer for Diff Algorithms in a linked worktree | Record stale reply rejection |
| 184 | Check network transfer for Diff Algorithms in a linked worktree | Record visible state after failure |
| 185 | Check network transfer for Diff Algorithms in cold and warm cache states | Record time to first useful rows |
| 186 | Check network transfer for Diff Algorithms in cold and warm cache states | Record steady frame cost |
| 187 | Check network transfer for Diff Algorithms in cold and warm cache states | Record bytes accepted from child output |
| 188 | Check network transfer for Diff Algorithms in cold and warm cache states | Record Git and gh process count |
| 189 | Check network transfer for Diff Algorithms in cold and warm cache states | Record maximum retained document bytes |
| 190 | Check network transfer for Diff Algorithms in cold and warm cache states | Record cache disposition and complete key |
| 191 | Check network transfer for Diff Algorithms in cold and warm cache states | Record stale reply rejection |
| 192 | Check network transfer for Diff Algorithms in cold and warm cache states | Record visible state after failure |
| 193 | Check subprocess count for Diff Algorithms in a small local repository | Record time to first useful rows |
| 194 | Check subprocess count for Diff Algorithms in a small local repository | Record steady frame cost |
| 195 | Check subprocess count for Diff Algorithms in a small local repository | Record bytes accepted from child output |
| 196 | Check subprocess count for Diff Algorithms in a small local repository | Record Git and gh process count |
| 197 | Check subprocess count for Diff Algorithms in a small local repository | Record maximum retained document bytes |
| 198 | Check subprocess count for Diff Algorithms in a small local repository | Record cache disposition and complete key |
| 199 | Check subprocess count for Diff Algorithms in a small local repository | Record stale reply rejection |
| 200 | Check subprocess count for Diff Algorithms in a small local repository | Record visible state after failure |
| 201 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record time to first useful rows |
| 202 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record steady frame cost |
| 203 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record bytes accepted from child output |
| 204 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record Git and gh process count |
| 205 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record maximum retained document bytes |
| 206 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record cache disposition and complete key |
| 207 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record stale reply rejection |
| 208 | Check subprocess count for Diff Algorithms in a monorepo with many changed paths | Record visible state after failure |
| 209 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record time to first useful rows |
| 210 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record steady frame cost |
| 211 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record bytes accepted from child output |
| 212 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record Git and gh process count |
| 213 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record maximum retained document bytes |
| 214 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record cache disposition and complete key |
| 215 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record stale reply rejection |
| 216 | Check subprocess count for Diff Algorithms in a pull request containing generated files | Record visible state after failure |
| 217 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record time to first useful rows |
| 218 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record steady frame cost |
| 219 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record bytes accepted from child output |
| 220 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record Git and gh process count |
| 221 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record maximum retained document bytes |
| 222 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record cache disposition and complete key |
| 223 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record stale reply rejection |
| 224 | Check subprocess count for Diff Algorithms in a deeply diverged branch | Record visible state after failure |
| 225 | Check subprocess count for Diff Algorithms in an unavailable network | Record time to first useful rows |
| 226 | Check subprocess count for Diff Algorithms in an unavailable network | Record steady frame cost |
| 227 | Check subprocess count for Diff Algorithms in an unavailable network | Record bytes accepted from child output |
| 228 | Check subprocess count for Diff Algorithms in an unavailable network | Record Git and gh process count |
