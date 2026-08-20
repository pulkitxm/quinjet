# Intraline Emphasis and Syntax Highlighting

A diff pane earns its keep with two layers of color: syntax highlighting, which paints code the way
an editor would, and intraline emphasis, which marks the exact changed region inside a paired
removed and added line. Quinjet computes these two layers at opposite ends of its pipeline: syntax
spans are produced once, at parse time, and stored inside the document; intraline emphasis is
recomputed every frame, but only for the rows on screen. This page walks both systems from theory
to the exact code: the `EmphasisBlock` scan and positional pairing, the `changed_ranges` prefix and
suffix computation, the 32 KiB per-pair source cap, the 27 percent blend backgrounds, and the story
of PR #46 turning per-frame emphasis from O(document) into O(viewport). It then covers syntect:
how grammar stacks and incremental line parsing work, why `parse_diff` keeps two `HighlightLines`
states, the two-face `extra_newlines` syntax set, the mapping from `base16-ocean.dark` to the
theme-independent `SyntaxColor` enum, the 512 KiB per-patch and 32 KiB per-line budgets, and
semantic color resolution through `src/theme.rs`. The surrounding parse pipeline is documented in
./pipeline.md and the hunk-level diff theory in ./algorithms.md.

## Contents

- [The two color systems at a glance](#the-two-color-systems-at-a-glance)
- [The shared span model](#the-shared-span-model)
- [Intraline emphasis: the problem and the algorithm family](#intraline-emphasis-the-problem-and-the-algorithm-family)
- [Replacement blocks and positional pairing](#replacement-blocks-and-positional-pairing)
- [changed_ranges: prefix and suffix over char boundaries](#changed_ranges-prefix-and-suffix-over-char-boundaries)
- [The 32 KiB intraline source cap](#the-32-kib-intraline-source-cap)
- [Painting the emphasis: span splitting and the 27 percent blend](#painting-the-emphasis-span-splitting-and-the-27-percent-blend)
- [PR #46: per-frame work from O(document) to O(viewport)](#pr-46-per-frame-work-from-odocument-to-oviewport)
- [How syntect parses and highlights](#how-syntect-parses-and-highlights)
- [Two parser states per file section](#two-parser-states-per-file-section)
- [two-face and the extra_newlines syntax set](#two-face-and-the-extra_newlines-syntax-set)
- [From base16-ocean.dark to semantic SyntaxColor](#from-base16-oceandark-to-semantic-syntaxcolor)
- [Semantic resolution in src/theme.rs](#semantic-resolution-in-srcthemers)
- [The syntax budgets: 512 KiB per patch, 32 KiB per line](#the-syntax-budgets-512-kib-per-patch-32-kib-per-line)
- [Upstream feeders and the 32 MiB document budget](#upstream-feeders-and-the-32-mib-document-budget)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Edge cases and failure modes](#edge-cases-and-failure-modes)
- [The tests that pin the contract](#the-tests-that-pin-the-contract)

## The two color systems at a glance

The two systems answer different questions and therefore live at different pipeline stages.

Syntax highlighting answers "what language construct is this token". The answer depends only on
the file's content and grammar, never on the viewport, the theme, or the frame. It is therefore
computed exactly once, inside `parse_diff` in `src/git/diff.rs`, while raw patch bytes are turned
into a `DiffDocument`. The result is stored as `HighlightSpan` runs on every `DiffLine`, and those
spans survive for as long as the document is cached.

Intraline emphasis answers "which part of this line changed relative to its partner". The answer
depends on pairing two rows, and which rows are worth pairing depends on what is on screen. It is
therefore computed at render time, in `src/ui/mod.rs`, once per frame, from exactly the visible
row indices. Nothing about it is stored in the document; the computed byte ranges live only for
the duration of one draw call.

```text
raw patch bytes (git diff output, capped at 8 MiB per read)
        │
        ▼
parse_diff (src/git/diff.rs)          ── once per patch ──
  ├─ unified-diff structure: files, hunks, rows
  ├─ syntect highlighting: two HighlightLines states
  │    budget: 512 KiB per patch, 32 KiB per line
  └─ output: DiffDocument { lines: Vec<DiffLine> }
        │
        ▼  (cached: per-file documents, 32 MiB in-memory budget)
        │
draw (src/ui/mod.rs)                  ── once per frame ──
  ├─ row layout cache: unified_diff_rows / side_by_side_diff_rows
  ├─ visible_intraline_emphasis: only on-screen rows
  │    budget: 32 KiB per line pair
  └─ highlight_spans: foreground from SyntaxColor + theme,
       background from line kind + emphasis range
```

The division of labor is deliberate and each half exploits a different invariance:

- Syntax spans are invariant under scrolling, resizing, folding, and theme switching, so they are
  computed once and stored. The theme-switch invariance is the subtle one: spans store a semantic
  `SyntaxColor`, not an RGB value, so changing the theme re-colors every document without
  re-parsing anything (see
  [From base16-ocean.dark to semantic SyntaxColor](#from-base16-oceandark-to-semantic-syntaxcolor)).
- Emphasis ranges are cheap to compute for a screenful of rows but expensive for a whole document,
  and they are pure functions of two lines' text. Recomputing them per frame costs O(viewport) and
  removes any need for invalidation bookkeeping (see
  [PR #46: per-frame work from O(document) to O(viewport)](#pr-46-per-frame-work-from-odocument-to-oviewport)).

The budgets that bound each stage:

| Constant | Value | Where | Bounds |
| --- | --- | --- | --- |
| `MAX_DIFF_BYTES` | 8 MiB | `src/git/mod.rs` | any single patch read from Git |
| `MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES` | 512 KiB | `src/git/diff.rs:15` | grammar work per patch |
| `MAX_SYNTAX_HIGHLIGHT_LINE_BYTES` | 32 KiB | `src/git/diff.rs:16` | grammar work per content line |
| `MAX_INTRALINE_SOURCE_BYTES` | 32 KiB | `src/ui/mod.rs:38` | emphasis work per line pair |
| `MAX_PULL_REQUEST_DOCUMENT_BYTES` | 32 MiB | `src/app.rs` | parsed PR documents held in memory |
| `MAX_CACHED_PATCH_BYTES` | 1 MiB | `src/git/github/mod.rs` | one cached per-file PR patch on disk |

ARCHITECTURE.md states the contract as invariant 6: "Syntax grammar parsing stops at 512 KiB per
patch or 32 KiB per row, parsed PR patches use a 32 MiB in-memory budget, and collapsed cached
patches are not cloned into the combined document, so post-processing remains bounded after Git
returns." Everything in this page is machinery behind that sentence.

## The shared span model

Both systems operate over the same three types in `src/git/diff.rs`, so their interaction is worth
pinning down before either algorithm. The row kind enum, from `src/git/diff.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DiffLineKind {
    FileHeader,
    FileFooter,
    HunkHeader,
    Context,
    Added,
    Removed,
    Meta,
}
```

Only `Removed` and `Added` rows ever receive intraline emphasis; only `Removed`, `Added`, and
`Context` rows carry syntax-highlighted content. `FileHeader`, `FileFooter`, `HunkHeader`, and
`Meta` rows hold plain spans. The span itself, also from `src/git/diff.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HighlightSpan {
    pub text: String,
    pub foreground: Option<SyntaxColor>,
    pub bold: bool,
    pub italic: bool,
}
```

Two properties of this struct shape everything downstream:

**1. `foreground` is optional and semantic.** `None` means "no syntax opinion": the renderer falls
back to the per-kind default color (`line_foreground` in `src/ui/mod.rs`), which is what paints an
un-highlighted added line green and an un-highlighted removed line red. `Some(SyntaxColor)` names
a semantic role (`Comment`, `Green`, `Purple`, ...) that the active theme resolves to an RGB value
at draw time. The stored document never contains an absolute color.

**2. There is no background field.** Backgrounds are entirely a render-time decision: the row kind
picks the base background (`added_background`, `removed_background`, panel), and the intraline
emphasis range upgrades the changed piece to the stronger emphasis background. Because emphasis is
not stored, a document cached for hours renders correctly under any theme and any viewport.

A row is a list of spans plus line numbers:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub spans: Vec<HighlightSpan>,
}
```

`old_line` and `new_line` are the 1-based file line numbers: a `Removed` row has only `old_line`,
an `Added` row only `new_line`, a `Context` row both. The intraline machinery reassembles a row's
full text with `DiffLine::text()`, from `src/git/diff.rs`:

```rust
pub(crate) fn text(&self) -> String {
    if self.kind == DiffLineKind::FileHeader {
        self.spans
            .iter()
            .map(|span| span.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}
```

The branch matters for correctness: file headers hold three separate spans (label, `+n`, `-n`)
that display with spacing between them, so `text()` joins them with single spaces. Content rows
concatenate spans with no separator, which makes the concatenated string byte-for-byte identical
to the original content line. That identity is what lets `changed_ranges` return byte offsets over
`text()` and `highlight_spans` apply those offsets against the span list using a running byte
cursor: both walk the same byte sequence. Since emphasis is only computed for `Removed` and
`Added` rows, the space-joined header branch never participates.

One more upstream detail keeps the two byte views aligned: `parse_diff` expands tabs before
highlighting. `expand_tabs` in `src/git/diff.rs` rewrites each tab to spaces up to the next
multiple of `TAB_WIDTH = 4` display columns, so the text stored in spans, the text reassembled by
`text()`, and the text measured for display width all agree. Neither the highlighter nor the
emphasis scanner ever sees a raw tab.

All three types derive `Serialize` (`kebab-case` kinds, `camelCase` fields), because the same
documents back the `--json` output of the diff subcommands. A machine consumer therefore receives
semantic colors (`"green"`, `"comment"`), not terminal RGB, and no emphasis ranges at all, since
emphasis is a property of a rendered viewport rather than of the diff.

## Intraline emphasis: the problem and the algorithm family

When a line is modified, Git's line-oriented diff reports it as one removed line plus one added
line. For a reader, that is too coarse: in a 120-column line where one identifier changed, the
interesting three characters are visually indistinguishable from the 117 unchanged ones. Editors
solve this with a second, finer diff inside the paired lines, rendered as a stronger background on
just the changed region. VS Code does this in its diff editor, and Git itself offers a variant with
`--word-diff` (see [git-diff](https://git-scm.com/docs/git-diff)).

The general problem: given an old string and a new string, produce a set of ranges on each side
covering the differences. There is a spectrum of algorithms, trading fidelity for cost:

| Approach | Output | Cost | Failure mode |
| --- | --- | --- | --- |
| Character-level LCS (Myers on chars) | minimal per-char ranges | O(ND), worst O(n²) | noise: highlights scattered single chars |
| Word-level diff (tokenize, then LCS) | per-word ranges | tokenizer + O(ND) | language-dependent token rules |
| Common prefix/suffix | one contiguous range per side | O(n), no allocation | over-highlights multi-edit lines |

**Character-level LCS** produces the minimal edit ranges but is quadratic in the worst case and,
more damningly for readability, tends to find spurious micro-matches: diffing `oldValue` against
`newValue` character-by-character matches the `l` inside `old` with the `l` in `Value` and can
fragment the highlight into confetti. Editors that use it add post-processing to merge nearby
ranges, which is more code and more tuning.

**Word-level diff** is what `git diff --word-diff` does: split each line into tokens with a regex,
run the line diff over tokens, and report changed tokens. Quality is good, but it drags in a
tokenizer, a per-language notion of what a word is, and an O(ND) diff per pair. Run per visible
row per frame in a TUI, that is real work.

**Common prefix and suffix** scans forward while characters match, scans backward while characters
match, and declares everything in between changed, as one contiguous range per side. It is a
single O(n) pass with zero allocations, and it is exact whenever the line contains one contiguous
edit, which covers the overwhelmingly common cases: a renamed identifier, a changed literal, an
inserted argument, a changed operator. Quinjet uses this approach.

The known cost is over-highlighting when a line contains two separated edits. Consider:

```text
old:  foo(a, b, c)
new:  foo(x, b, z)
```

The common prefix is `foo(` (4 bytes) and the common suffix is `)` (1 byte), so both ranges come
out as bytes 4..11: `a, b, c` and `x, b, z`, including the unchanged `, b, ` in the middle. A
word-level diff would have marked only `a`/`x` and `c`/`z`. Quinjet accepts the wider range: the
highlight still lands the eye on the right region of the line, the cost stays linear, and the one
contiguous range per side is exactly the shape the span-splitting renderer consumes (a single
range intersects each `HighlightSpan` at most once, so every span splits into at most three
pieces; see
[Painting the emphasis: span splitting and the 27 percent blend](#painting-the-emphasis-span-splitting-and-the-27-percent-blend)).

The choice also interlocks with where the computation runs. Because emphasis is recomputed every
frame for visible rows (post-#46), the per-pair cost is paid tens of times per second during a
scroll. An O(n) affix scan over two 100-byte lines is a few hundred comparisons; an O(ND) word
diff with tokenization would multiply that by an order of magnitude for marginal visual gain. The
section on [design alternatives](#design-alternatives-and-why-they-lost) returns to this tradeoff.

## Replacement blocks and positional pairing

Before any ranges can be computed, the renderer must decide which removed line pairs with which
added line. Unified diff output makes this tractable through a structural property: within a hunk,
a contiguous run of edits prints all its `-` lines first, then all its `+` lines. Git never
interleaves them inside one edit region; an interleaving would imply a context line between,
which by definition starts a new region. So the visual shape of a modification is always:

```text
context
- removed line 0
- removed line 1
- removed line 2
+ added line 0
+ added line 1
context
```

Quinjet calls this shape a replacement block and represents it with three indices into
`document.lines`, from `src/ui/mod.rs`:

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

The removed run spans `removed_start..added_start` and the added run spans
`added_start..added_end`. There is no `removed_end` field because the removed run ends exactly
where the added run starts; the adjacency is the definition of the block.

### Discovering the block from any member row

The scan is seeded from a single row index, which is the crucial design point: after PR #46, the
caller knows only which rows are visible, so the block around a visible row must be discoverable
locally, without walking the whole document. Two helpers expand outward over runs of one kind,
from `src/ui/mod.rs`:

```rust
fn emphasis_run_start(lines: &[DiffLine], mut start: usize, kind: DiffLineKind) -> usize {
    while start > 0
        && lines
            .get(start.saturating_sub(1))
            .is_some_and(|line| line.kind == kind)
    {
        start = start.saturating_sub(1);
    }
    start
}

fn emphasis_run_end(lines: &[DiffLine], mut end: usize, kind: DiffLineKind) -> usize {
    while lines.get(end).is_some_and(|line| line.kind == kind) {
        end = end.saturating_add(1);
    }
    end
}
```

`emphasis_block` composes them differently depending on which side the seed row is on, from
`src/ui/mod.rs`:

```rust
fn emphasis_block(lines: &[DiffLine], index: usize) -> Option<EmphasisBlock> {
    match lines.get(index)?.kind {
        DiffLineKind::Removed => {
            let removed_start = emphasis_run_start(lines, index, DiffLineKind::Removed);
            let added_start =
                emphasis_run_end(lines, index.saturating_add(1), DiffLineKind::Removed);
            let added_end = emphasis_run_end(lines, added_start, DiffLineKind::Added);
            Some(EmphasisBlock {
                removed_start,
                added_start,
                added_end,
            })
        }
        DiffLineKind::Added => {
            let added_start = emphasis_run_start(lines, index, DiffLineKind::Added);
            let removed_start = emphasis_run_start(lines, added_start, DiffLineKind::Removed);
            let added_end = emphasis_run_end(lines, index.saturating_add(1), DiffLineKind::Added);
            Some(EmphasisBlock {
                removed_start,
                added_start,
                added_end,
            })
        }
        _ => None,
    }
}
```

From a `Removed` seed: walk back to the start of the removed run, walk forward past the rest of
the removed run to find where the added run starts, then walk forward past the added run. From an
`Added` seed: walk back to the start of the added run, then keep walking back through the removed
run that must immediately precede it, then forward to the end of the added run. Any other kind
returns `None`, which is how context rows, headers, and meta rows opt out. Note the asymmetric
subtlety on the `Added` arm: `emphasis_run_start(lines, added_start, DiffLineKind::Removed)`
starts the backward walk at the first added row, so if no removed run precedes the added run (a
pure insertion), the walk moves nowhere and `removed_start == added_start`, giving a removed run
of length zero. The pairing arithmetic below then produces `pair_count = 0` and the insertion
gets no emphasis, which is correct: there is nothing to compare it against.

### A worked example

Take this seven-row slice of a document (indices are positions in `document.lines`):

```text
index  kind     text
0      Context  "    let total = 0;"
1      Removed  "    for item in list {"
2      Removed  "        total += item.value;"
3      Removed  "    }"
4      Added    "    for item in &list {"
5      Added    "        total += item.value();"
6      Context  "    total"
```

Seeding `emphasis_block` at index 2 (a `Removed` row):

1. `removed_start = emphasis_run_start(lines, 2, Removed)`: index 1 is `Removed`, index 0 is
   `Context`, so `removed_start = 1`.
1. `added_start = emphasis_run_end(lines, 3, Removed)`: indices 3 is `Removed`, 4 is not, so
   `added_start = 4`.
1. `added_end = emphasis_run_end(lines, 4, Added)`: indices 4 and 5 are `Added`, 6 is not, so
   `added_end = 6`.

Seeding at index 5 (an `Added` row) reaches the same block by the other arm: `added_start = 4`
(walk back over the added run), `removed_start = 1` (walk back over the removed run from 4), and
`added_end = 6`. Every member row of a block resolves to the identical block, which is what makes
the per-frame block cache in `visible_intraline_emphasis` sound.

### Positional pairing and pair_count

With the block known, pairing is strictly positional: the i-th removed line pairs with the i-th
added line. The number of pairs is the length of the shorter run:

```text
removed_run_len = added_start - removed_start   = 4 - 1 = 3
added_run_len   = added_end   - added_start     = 6 - 4 = 2
pair_count      = min(removed_run_len, added_run_len) = 2
```

Rows 1 and 4 pair, rows 2 and 5 pair, and row 3, the surplus removed line `    }`, has
`pair_index = 2 >= pair_count` and receives no emphasis. It renders as a plainly deleted line,
which is the honest presentation: nothing replaced it.

Positional pairing is an assumption, not an analysis: it presumes the author edited line-for-line
in order. When the assumption is wrong (say the block deletes a comment at the top of the run and
modifies the line below, shifting positions by one), the pairing compares the wrong lines, the
common prefix and suffix collapse to little or nothing, and the emphasis quietly covers most of
both lines. The failure is cosmetic and conservative: emphasis degrades toward "the whole line
changed", never toward highlighting an unchanged region as changed on its own. The alternative,
similarity-based pairing (score every removed x added combination and match greedily), costs
O(removed_run x added_run) text comparisons per block and is examined in
[Design alternatives and why they lost](#design-alternatives-and-why-they-lost).

The same positional rule drives the side-by-side layout, so the two views can never disagree about
which lines face each other. `side_by_side_rows` in `src/ui/mod.rs` measures the removed run and
the following added run with the same two-scan shape and then emits one `Split` row per pair
position:

```rust
let removed_len = added_start - removed_start;
let added_len = index - added_start;
for pair_index in 0..removed_len.max(added_len) {
    rows.push(SideBySideRow::Split(
        (pair_index < removed_len).then(|| removed_start + pair_index),
        (pair_index < added_len).then(|| added_start + pair_index),
    ));
}
```

Note the `max` here versus the `min` in the emphasis path: the split view must display every line,
so surplus rows appear as `Split(Some(_), None)` or `Split(None, Some(_))` with a blank filler
cell on the unmatched side, while the emphasis path only processes the `min`-bounded pairs. The
two constants describe the same block from two needs: layout shows everything, emphasis compares
only what has a partner.

## changed_ranges: prefix and suffix over char boundaries

With a pair selected, the byte ranges come from `changed_ranges`, quoted in full from
`src/ui/mod.rs`:

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

The function returns byte ranges but iterates characters. That distinction carries all the
correctness weight, so it deserves a careful reading.

### The prefix loop

`char_indices()` yields `(byte_offset, char)` pairs, so the loop compares whole characters, never
bytes. Comparing raw bytes would risk declaring a common prefix that ends in the middle of a
multi-byte UTF-8 sequence; slicing a `String` at such an offset panics in Rust, and rendering a
range that splits a code point would corrupt the emphasis boundary. By advancing
`prefix = old_index + old_character.len_utf8()`, the prefix always lands exactly on a character
boundary of both strings.

The loop's second condition, `old_index != new_index`, is an invariant guard rather than a
reachable branch: two equal characters occupy the same number of bytes, and both iterations start
at offset zero, so as long as every compared character has matched, the byte offsets are
necessarily equal. Writing the check anyway makes the assumption explicit and keeps the function
safe under future edits: if the iteration logic ever changed so the two sides could drift, the
loop stops at the first desynchronized position instead of producing a prefix that is valid on one
string and not the other.

### The suffix loop

The backward pass uses `char_indices().rev()`, which is a `DoubleEndedIterator` walking characters
from the end, again yielding character-boundary byte offsets. Here the two sides' offsets genuinely
differ whenever the strings have different lengths, so there is no index-equality condition.
Instead the guard is `old_index < prefix || new_index < prefix`: the suffix must not claim bytes
the prefix already claimed. Without it, a pair like `"aa"` and `"aaa"` would go wrong:

- Prefix pass: both characters of `"aa"` match, `prefix = 2` (the zip ends when the shorter side
  is exhausted).
- Suffix pass without the guard: `(1, 'a')` vs `(2, 'a')` match, then `(0, 'a')` vs `(1, 'a')`
  match, driving `old_end` to 0 while `prefix` is 2, an inverted range.

With the guard, the very first backward comparison `(1, 'a')` vs `(2, 'a')` fails `old_index <
prefix` (1 < 2) and breaks immediately, leaving `old_end = 2` and `new_end = 3`. The result is
`(None, Some(2..3))`: nothing changed on the old side, and the appended `a` is highlighted on the
new side. This overlap rule also encodes a policy: when a repeated character makes the boundary
ambiguous (did the author append an `a` at the end or insert one in the middle?), the prefix wins
and the change is attributed as late in the line as possible.

### Return semantics

The final expressions convert empty ranges to `None`:

- Both `Some`: a replacement; each side has a non-empty changed region.
- Old `None`, new `Some`: a pure insertion within the line; only the new side gets emphasis.
- Old `Some`, new `None`: a pure deletion within the line.
- Both `None`: the lines are identical; nothing to emphasize.

The identical-lines case is not hypothetical: positional pairing can align a removed and an added
line that happen to have equal text (for example, a block that moves a line while editing its
neighbors). `changed_ranges` then correctly reports no intraline difference and the rows render
with only their line-level backgrounds.

### Worked example: the pinned test case

The behavior is pinned by the test `computes_vscode_style_intraline_changed_ranges` in
`src/ui/mod.rs`, whose first assertion is:

```rust
assert_eq!(
    changed_ranges("const oldValue = 1;", "const newValue = 2;"),
    (Some(6..18), Some(6..18))
);
```

Byte-by-byte, both strings are 19 bytes of ASCII:

```text
offset:  0    1    2    3    4    5    6    7    8    9    10   11   12   13   14   15   16   17   18
old:     c    o    n    s    t    ' '  o    l    d    V    a    l    u    e    ' '  =    ' '  1    ;
new:     c    o    n    s    t    ' '  n    e    w    V    a    l    u    e    ' '  =    ' '  2    ;
```

The prefix pass matches `const ` and stops at offset 6 (`o` vs `n`), so `prefix = 6`. The suffix
pass matches `;` at offset 18 and stops at offset 17 (`1` vs `2`), so `old_end = new_end = 18`.
Both ranges are `6..18`, covering `oldValue = 1` and `newValue = 2`.

Notice what the algorithm did not do: `Value = ` (offsets 9 through 16) is identical on both
sides, but it sits between two changed regions, and a contiguous suffix scan from the end cannot
reach it. The single range therefore swallows it. This is the same over-highlighting tradeoff
shown earlier with `foo(a, b, c)`, and it is the accepted price of the O(n) scan.

### Worked example: multi-byte characters

Consider `"café_old = 1"` against `"café_new = 1"`. The `é` occupies two bytes (0xC3 0xA9), so
byte offsets and character positions diverge after it:

```text
byte:    0    1    2    3    4    5    6    7    8    9    10   11   12
old:     c    a    f    é━━━━━    _    o    l    d    ' '  =    ' '  1
new:     c    a    f    é━━━━━    _    n    e    w    ' '  =    ' '  1
```

The prefix pass matches `c`, `a`, `f`, `é`, `_`; after `_` at byte 5, `prefix = 6`. The characters
`o` vs `n` at byte 6 differ. The suffix pass matches `1`, the space, `=`, the other space, and
stops at `d` vs `w` (byte 8), so both ends settle at 9. The ranges are `6..9` on both sides,
exactly the bytes of `old` and `new`, and both endpoints are character boundaries even though a
two-byte character sits inside the common prefix. An implementation iterating bytes with the same
structure would have produced the same answer here, but only because the changed region contains
no multi-byte characters; place the edit inside `café` itself and byte-wise scanning could split
the `é`. Character iteration removes the entire class of bug.

### Cost profile

Each pass is a single linear scan that stops at the first mismatch, so the total work is
proportional to the length of the common affixes plus one character, bounded by the shorter line.
There are no allocations inside `changed_ranges`; the only allocations in the whole emphasis path
are the two `String`s materialized by `DiffLine::text()` in the caller, and those are bounded by
the 32 KiB cap described next. For typical source lines of 40 to 120 bytes, a pair costs on the
order of a hundred character comparisons, which is why recomputing emphasis for a screenful of
rows every frame is affordable.

## The 32 KiB intraline source cap

`changed_ranges` is cheap for source code but an attacker-shaped input exists in every real
repository: the single-line minified bundle, the one-line lockfile, the generated JSON blob. A
pair of such lines can be megabytes each. Emphasizing them would allocate two multi-megabyte
strings and scan them, per visible row, per frame. The guard sits in
`paired_intraline_emphasis`, quoted in full from `src/ui/mod.rs`:

```rust
fn paired_intraline_emphasis(
    old_line: Option<&DiffLine>,
    new_line: Option<&DiffLine>,
) -> (Option<Range<usize>>, Option<Range<usize>>) {
    let (Some(old_line), Some(new_line)) = (old_line, new_line) else {
        return (None, None);
    };
    if old_line.kind != DiffLineKind::Removed || new_line.kind != DiffLineKind::Added {
        return (None, None);
    }
    let old_bytes = old_line
        .spans
        .iter()
        .fold(0_usize, |total, span| total.saturating_add(span.text.len()));
    let new_bytes = new_line
        .spans
        .iter()
        .fold(0_usize, |total, span| total.saturating_add(span.text.len()));
    if old_bytes.max(new_bytes) > MAX_INTRALINE_SOURCE_BYTES {
        return (None, None);
    }
    changed_ranges(&old_line.text(), &new_line.text())
}
```

with the constant declared at `src/ui/mod.rs:38`:

```rust
const MAX_INTRALINE_SOURCE_BYTES: usize = 32 * 1024;
```

The ordering of the checks is the point of the function:

**1. Presence and kind first.** Both lines must exist and be exactly a `Removed` and an `Added`
row, in that order. This makes the function safe to call from the side-by-side renderer, which
feeds it whatever pair of lines a `Split` row references, including `Context` rows (a
`Split(Some(i), Some(i))` context row passes the same line as both sides and is rejected by the
kind check) and half-empty rows (`Split(Some(_), None)` fails the presence check).

**2. Size before allocation.** The byte totals are computed by summing `span.text.len()` over the
stored spans, which reads lengths that already exist in memory and allocates nothing. Only after
both sums pass the cap does the function call `DiffLine::text()`, which allocates the two
concatenated strings. A 10 MiB minified line is therefore rejected for the cost of iterating its
span list, not for the cost of copying 10 MiB, and the rejection repeats harmlessly every frame
the row stays visible. The sums use `saturating_add`, so even a pathological document whose span
lengths overflow `usize` degrades to "too big" rather than wrapping around to "small enough".

**3. If either side is over, both sides are skipped.** The condition is on
`old_bytes.max(new_bytes)`: emphasis is a comparison, and comparing a normal line against a
monster line would cost the monster's length in the prefix scan. Returning `(None, None)` renders
both rows with plain line-level backgrounds, which is also the honest presentation: a claim about
"the changed region" of a 10 MiB line has no visual value in an 80-column pane anyway.

The value 32 KiB is not arbitrary: it equals `MAX_SYNTAX_HIGHLIGHT_LINE_BYTES` in
`src/git/diff.rs`, so the two systems share one definition of "this line is too pathological to
analyze". A line that was too long to syntax-highlight at parse time is also too long to emphasize
at render time; both degrade in the same place, and a reader sees consistent plainness rather than
one system straining where the other gave up.

The guard has its own test, `skips_intraline_work_for_very_long_rows` in `src/ui/mod.rs`, which
builds a removed and an added line of `MAX_INTRALINE_SOURCE_BYTES + 1` repeated characters and
asserts the `(None, None)` bail-out:

```rust
let old = test_line(
    DiffLineKind::Removed,
    &"a".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
);
let new = test_line(
    DiffLineKind::Added,
    &"b".repeat(MAX_INTRALINE_SOURCE_BYTES + 1),
);

assert_eq!(
    paired_intraline_emphasis(Some(&old), Some(&new)),
    (None, None)
);
```

## Painting the emphasis: span splitting and the 27 percent blend

A computed range is a pair of byte offsets over a line's concatenated text; the renderer must turn
it into styled terminal cells without losing the syntax colors already attached to the spans. The
work happens in `highlight_spans` and `push_highlight_piece` in `src/ui/mod.rs`, one pass that
simultaneously applies syntax foregrounds, the emphasis background, horizontal scrolling, and
width clipping.

### Mapping a byte range onto the span list

`highlight_spans` walks the stored `HighlightSpan`s with a running byte cursor and intersects the
emphasis range with each span's byte interval, from `src/ui/mod.rs`:

```rust
let span_start = source_offset;
let span_end = span_start + span.text.len();
let intersection = emphasis.and_then(|range| {
    let start = range.start.max(span_start);
    let end = range.end.min(span_end);
    (start < end).then_some(start..end)
});
```

`source_offset` accumulates `span.text.len()` across the loop, so `span_start..span_end` is the
span's interval in the same byte coordinate system `changed_ranges` used, because
`DiffLine::text()` concatenates content spans with no separator. When the intersection is
non-empty, the span's text splits into up to three pieces: the bytes before the changed region,
the changed bytes, and the bytes after. Each piece goes through `push_highlight_piece` with an
`emphasized` flag, and only the middle piece carries `true`. A span entirely inside the range
becomes one emphasized piece plus two empty pieces that are dropped immediately; a span the range
merely clips at one end splits into two.

Because the emphasis range is contiguous, each span intersects it at most once, so the output span
count is at most the input count plus two. A fragmented multi-range design would multiply this
bookkeeping; the single-range shape of the affix algorithm keeps the renderer's inner loop simple.

### push_highlight_piece: columns, not bytes

The piece writer applies horizontal scroll and clipping in display columns, quoted from
`src/ui/mod.rs`:

```rust
fn push_highlight_piece(
    output: &mut Vec<Span<'_>>,
    text: &str,
    mut style: Style,
    emphasized: bool,
    kind: DiffLineKind,
    theme: &Theme,
    skip: &mut usize,
    remaining: &mut usize,
) {
    if text.is_empty() || *remaining == 0 {
        return;
    }
    let text_width = text.width();
    if *skip >= text_width {
        *skip -= text_width;
        return;
    }
    if emphasized {
        style = match kind {
            DiffLineKind::Added => style.bg(theme.added_emphasis_background),
            DiffLineKind::Removed => style.bg(theme.removed_emphasis_background),
            _ => style,
        };
    }
    let sliced = slice_width(text, *skip, *remaining);
    *skip = 0;
    *remaining = remaining.saturating_sub(sliced.width());
    output.push(Span::styled(sliced, style));
}
```

`skip` starts as the pane's horizontal scroll offset and `remaining` as the available content
width; both count display columns measured with the `unicode-width` rules (a CJK character is two
columns, a zero-width joiner is zero). The emphasis decision, by contrast, was made in bytes. The
two coordinate systems never need reconciling because the split into pieces happens first, in byte
space, and each piece is then independently skipped and clipped in column space by `slice_width`.
Scrolling a line sideways therefore slides the emphasized background along with its text: a piece
scrolled fully off consumes its width from `skip` and emits nothing, a piece half visible is
sliced mid-piece, and the background attaches to whatever part of the changed piece survives.

The style layering is also visible here: the piece keeps its syntax `Style` (foreground color,
bold, italic) and the emphasis only sets the background. A changed keyword stays purple while
gaining the stronger background; emphasis and syntax highlighting compose rather than compete.
The non-emphasized pieces get their background from the row level: `draw_unified_line` renders the
whole row's `Paragraph` with `line_background(line.kind, theme)`, which is `added_background` or
`removed_background` for diff rows, so the changed piece's stronger background reads as a
highlight within the already-tinted line.

### The 27 percent blend

The emphasis backgrounds are not hand-picked hex values; they are derived from each theme's
palette by integer interpolation, so all 13 themes in both light and dark appearances get a
consistent two-level tint hierarchy for free. `Theme::new` in `src/theme.rs` constructs four diff
backgrounds from two palette slots:

```rust
let added_background = blend(palette[11], palette[0], 14);
let added_emphasis_background = blend(palette[11], palette[0], 27);
let removed_background = blend(palette[8], palette[0], 14);
let removed_emphasis_background = blend(palette[8], palette[0], 27);
```

`palette[11]` is the theme's green (additions), `palette[8]` its red (removals), and `palette[0]`
its base background. The blend itself is a const integer interpolation, from `src/theme.rs`:

```rust
const fn blend(foreground: u32, background: u32, amount: u32) -> Color {
    let inverse = 100 - amount;
    let red = (((foreground >> 16) & 0xff) * amount + ((background >> 16) & 0xff) * inverse) / 100;
    let green = (((foreground >> 8) & 0xff) * amount + ((background >> 8) & 0xff) * inverse) / 100;
    let blue = ((foreground & 0xff) * amount + (background & 0xff) * inverse) / 100;
    Color::Rgb(red as u8, green as u8, blue as u8)
}
```

So a whole added or removed line sits on a 14 percent wash of its hue over the background, and the
changed region inside it sits on a 27 percent wash: roughly double the chroma, enough to pop
without inverting the text. Working the arithmetic for the default Quinjet dark palette, where
`palette[0] = 0x0d1117` (13, 17, 23), `palette[11] = 0x3fb950` (63, 185, 80), and
`palette[8] = 0xf85149` (248, 81, 73):

| Background | Formula per channel | Result |
| --- | --- | --- |
| `added_background` | `(63*14 + 13*86)/100, (185*14 + 17*86)/100, (80*14 + 23*86)/100` | `(20, 40, 30)` = `#14281e` |
| `added_emphasis_background` | `(63*27 + 13*73)/100, (185*27 + 17*73)/100, (80*27 + 23*73)/100` | `(26, 62, 38)` = `#1a3e26` |
| `removed_background` | `(248*14 + 13*86)/100, (81*14 + 17*86)/100, (73*14 + 23*86)/100` | `(45, 25, 30)` = `#2d191e` |
| `removed_emphasis_background` | `(248*27 + 13*73)/100, (81*27 + 17*73)/100, (73*27 + 23*73)/100` | `(76, 34, 36)` = `#4c2224` |

The division rounds down per channel, which the code acknowledges with an explicit lint
expectation ("bounded integer RGB interpolation intentionally rounds down to a byte").

The two-level hierarchy alone would be worthless if syntax-colored text became unreadable on the
stronger background, so `Theme::new` treats both emphasis backgrounds as first-class surfaces:
they are members of the `surfaces` array against which every foreground is contrast-corrected by
`readable`, which iteratively blends a color toward white (dark appearance) or black (light
appearance) until it clears a minimum WCAG-style contrast ratio on every surface. The theme test
`every_theme_keeps_text_and_graphics_readable_on_every_surface` in `src/theme.rs` then asserts a
4.5 contrast ratio for the text, semantic, and all ten syntax colors against
`added_emphasis_background` and `removed_emphasis_background` for all 13 themes in both
appearances. A purple keyword inside an emphasized region is therefore guaranteed legible in every
palette, not just the default one.

Reserved gutter widths complete the geometry: the unified layout spends 12 columns on two 4-wide
line numbers plus markers before content begins (`content_area.width.saturating_sub(12)` in
`draw_unified_line`), each side of the split layout spends 7, and full-width meta rows spend 2.
The `width` passed into `highlight_spans` is what remains, so emphasis never bleeds into the
gutters.

## PR #46: per-frame work from O(document) to O(viewport)

Everything above describes the current, viewport-scoped design. It is worth documenting what it
replaced, because the change is the clearest illustration in the codebase of the difference
between work proportional to data and work proportional to screen.

PR #46 ("perf: viewport-scoped diff rendering and cached PR layouts", squash-merged as commit
`521ffee`, 2 files changed, 435 insertions and 183 deletions) carried four mechanisms, described
by its commit message bullets:

```text
perf: viewport-scoped diff rendering and cached PR layouts (#46)
* perf: scope intraline diff emphasis to visible rows
* perf: cache diff row layouts across frames
* perf: rebuild PR overview rows only when their content changes
* fix: close cache invalidation gaps in the overview rows
```

This page covers the first bullet in depth; the row-layout cache and the PR overview caches are
rendering-wide concerns documented in ../rendering/viewport.md.

### The before state

Before #46, the unified diff drawer called a function with this signature on every frame:

```text
fn intraline_emphasis(lines: &[DiffLine]) -> Vec<Option<Range<usize>>>
```

It allocated a `Vec` sized to the entire document and paired every removed run with its following
added run across all of `document.lines`, computing changed ranges for every pair whether or not
any of those rows could possibly be drawn. On a working-tree diff of a few hundred lines this was
invisible. On the stress benchmark that drove the whole optimization stack, the Bun rewrite pull
request oven-sh/bun#30412 with 2,188 changed files and +1,009,257 added lines (see
../benchmarking.md), the combined all-files document can hold on the order of a million rows, and
a ratatui TUI redraws on every input event, worker reply, and poll tick. Each frame re-derived
emphasis for the whole million-row document to draw a few dozen rows of it.

The scale is easy to bound from the codebase's own constants: the prefetch estimator in
`src/app.rs` budgets 80 bytes per changed line, so a million-line document is on the order of
80 MB of line text. The old function walked and compared against that volume once per frame, plus
a million-entry `Vec<Option<Range<usize>>>` allocation per frame, before a single cell was drawn.

An immediate-mode UI cannot amortize this by "only redrawing what changed" at the widget level;
ratatui rebuilds the visible widget tree each frame and diffs terminal cells afterward. The only
lever is to make the per-frame computation itself proportional to the viewport. That is exactly
what the replacement does.

### The after state: visible_intraline_emphasis

The current entry point takes the visible row indices as an iterator and returns a map keyed by
absolute line index, quoted in full from `src/ui/mod.rs`:

```rust
fn visible_intraline_emphasis(
    lines: &[DiffLine],
    visible: impl Iterator<Item = usize>,
) -> HashMap<usize, Range<usize>> {
    let mut emphasis = HashMap::new();
    let mut block: Option<EmphasisBlock> = None;
    for index in visible {
        let Some(kind) = lines.get(index).map(|line| line.kind) else {
            continue;
        };
        if kind != DiffLineKind::Removed && kind != DiffLineKind::Added {
            continue;
        }
        if !block.as_ref().is_some_and(|block| block.contains(index)) {
            block = emphasis_block(lines, index);
        }
        let Some(current) = block.as_ref() else {
            continue;
        };
        let pair_count = current
            .added_start
            .saturating_sub(current.removed_start)
            .min(current.added_end.saturating_sub(current.added_start));
        let pair_index = if kind == DiffLineKind::Removed {
            index.saturating_sub(current.removed_start)
        } else {
            index.saturating_sub(current.added_start)
        };
        if pair_index >= pair_count {
            continue;
        }
        let (old_range, new_range) = paired_intraline_emphasis(
            lines.get(current.removed_start.saturating_add(pair_index)),
            lines.get(current.added_start.saturating_add(pair_index)),
        );
        let range = if kind == DiffLineKind::Removed {
            old_range
        } else {
            new_range
        };
        if let Some(range) = range {
            let _ = emphasis.insert(index, range);
        }
    }
    emphasis
}
```

The unified drawer feeds it exactly the on-screen window, from `draw_unified_diff` in
`src/ui/mod.rs`:

```rust
let emphasis = visible_intraline_emphasis(
    &app.document.lines,
    rows.iter()
        .copied()
        .skip(diff_scroll)
        .take(area.height as usize),
);
```

`rows` here is the cached unified row list (line indices with hunk headers and collapsed file
bodies already excluded, see ../rendering/viewport.md), `diff_scroll` the vertical offset, and
`area.height` the pane height. On an 80x24 terminal the iterator yields at most a couple dozen
indices regardless of whether the document has 40 rows or a million. Each visible content row then
does its `HashMap` lookup with `emphasis.get(&line_index)` during drawing.

Four properties of the loop deserve attention:

**1. The block cache amortizes run discovery.** `block` persists across loop iterations, and
consecutive visible rows usually belong to the same replacement block, so the
`contains(index)` check skips re-deriving the block for every row of a run. A block of k paired
rows visible on screen costs one `emphasis_block` scan plus k pair computations, not k scans.
Because visible indices arrive in ascending order, the cache can only miss when the iteration
leaves the current block, so the number of `emphasis_block` calls per frame is the number of
distinct blocks intersecting the viewport.

**2. The partner does not need to be visible.** `emphasis_block` scans outward from the seed row,
so a visible added row whose removed partner is scrolled just above the viewport still finds it:
the block extends beyond the screen and the pairing indexes into `lines` directly. The test
`visible_intraline_emphasis_matches_block_pairing` in `src/ui/mod.rs` pins this with a six-line
document, passing `[3_usize]` as the only visible index and asserting the added row at index 3
still gets its range `8..9`, with the assertion message "partner is found outside the viewport".
Without this property, scrolling a pair's boundary across the viewport edge would flicker the
emphasis on and off.

**3. Only the visible side's range is stored.** `paired_intraline_emphasis` computes both ranges,
but the map keeps the old range for a `Removed` row and the new range for an `Added` row, keyed by
that row's own index. When both halves of a pair are visible, the pair is computed twice, once per
row. This is deliberate simplicity: memoizing per pair would save at most one affix scan per
visible pair per frame, at the cost of a second map and its invalidation, and the scan is already
bounded by the 32 KiB cap.

**4. The map is sparse.** Rows without emphasis (context, unpaired surplus, identical pairs,
over-cap pairs) simply have no entry, so the `HashMap` holds at most `area.height` entries. The
old design's document-sized `Vec<Option<...>>` spent memory on every row to answer questions about
a handful.

### Complexity accounting

Let h be the viewport height, b the number of replacement blocks intersecting the viewport, r the
total length of those blocks' runs, and L the byte length of the longest visible paired line under
the 32 KiB cap. Per frame:

- run discovery: O(r) across the b `emphasis_block` calls (each line of each intersecting block is
  visited once per scan direction);
- pairing arithmetic: O(h);
- range computation: O(h x L) worst case, since each visible paired row triggers one affix scan;
- map operations: O(h).

Every term is bounded by screen-shaped quantities plus the caps. The one input that can exceed the
viewport is r: a single replacement block can be arbitrarily tall (delete ten thousand lines, add
ten thousand), and the block scan walks all of it even when two rows are visible. That walk is a
`kind` comparison per row with no text access, so it is cheap in absolute terms, and it is bounded
by the size of the one block under the cursor rather than by the document. The pathological case
of one million-row replacement block costs a linear index walk per frame, still orders of
magnitude below the old design's per-pair text comparisons over the same rows.

### Why the side-by-side path needed no map

The split layout was already viewport-scoped before #46, structurally: a `Split` row carries both
sides of a pair by construction, so the renderer computes emphasis lazily per visible row, from
`src/ui/mod.rs`:

```rust
SideBySideRow::Split(old_index, new_index) => {
    let old_line = old_index.and_then(|line_index| lines.get(line_index));
    let new_line = new_index.and_then(|line_index| lines.get(line_index));
    let (old_emphasis, new_emphasis) = paired_intraline_emphasis(old_line, new_line);
```

No block discovery is needed because `side_by_side_rows` already did the positional pairing when
it built the row list, and that list is cached across frames (the `diff_rows_key` cache, see
../rendering/viewport.md). What #46 changed on this path is representation: `SideBySideRow` used
to hold `&DiffLine` borrows, tying the row list's lifetime to a single frame; the PR moved it to
index payloads (`Split(Option<usize>, Option<usize>)`) resolved through `lines.get(...)` at draw
time, which is what made the row list storable in `App` across frames at all. The unified and
split paths therefore converge on the same primitive, `paired_intraline_emphasis`, reached through
two different pairing routes that share the same block arithmetic.

### Why the emphasis map is not cached across frames

The row-layout lists gained a cross-frame cache in #46; the emphasis map deliberately did not. Its
natural key would be (document generation, scroll offset, viewport height), and the scroll offset
changes on every wheel tick, so the cache would miss precisely when the pane is busiest. Meanwhile
the rebuild costs O(viewport) with small constants, comfortably inside a frame budget. A cache
that mostly misses, keyed by rapidly changing state, would add invalidation surface for no
measurable win. The general principle, recomputing cheap screen-shaped state every frame while
caching expensive data-shaped state across frames, recurs throughout the renderer and is cataloged
in ../techniques.md.

## How syntect parses and highlights

Quinjet's syntax highlighting is built on [syntect](https://docs.rs/syntect), the Rust
implementation of Sublime Text's grammar system. Understanding three of syntect's concepts,
grammar stacks, scopes, and incremental line parsing, explains every design decision in
`parse_diff`, so this section covers the theory before the code.

### Grammars are stacks of contexts

A Sublime syntax definition (a `.sublime-syntax` YAML file) describes a language as a set of named
contexts. Each context is an ordered list of match rules; each rule is a regular expression, the
scope names to assign to its captures, and an action: push another context onto the stack, pop the
current one, or set (replace) it. Parsing maintains a stack of active contexts. At any position,
the parser tries the rules of the top context in order, takes the earliest match in the line,
emits scope operations for it, applies the action, and continues from the end of the match.

The stack is what makes nesting tractable without a real parser: a string context pushed inside a
function context remembers, by its mere position on the stack, that popping the string returns to
the function. A Rust line like this one:

```rust
let greeting = "hello"; // done
```

drives the stack roughly as follows (scope names abbreviated):

```text
position   event                          stack (bottom -> top)
0          match keyword "let"            [source.rust]
4          identifier                     [source.rust]
15         match '"' -> push string       [source.rust, string.quoted.double]
21         match '"' -> pop string        [source.rust]
24         match "//" -> push comment     [source.rust, comment.line]
end        line ends                      [source.rust, comment.line]
```

That final state is the crucial detail: the stack at the end of a line is the parser's memory. If
the comment had been a block comment `/*` without a closing `*/`, the stack would still hold the
comment context when the next line begins, and the next line would correctly highlight as comment
until the `*/` pops it. Multi-line strings, heredocs, nested template literals, and embedded
languages (regex inside JavaScript, SQL inside a string) all work through carried stack state.

### Scopes select styles

The parser's output is not colors but scope names: dotted, hierarchical labels such as
`string.quoted.double.rust` or `comment.line.double-slash.rust`, stacked so that a token carries
its whole ancestry (`source.rust meta.function.rust string.quoted.double.rust`). A theme is a list
of rules mapping scope selectors to styles: foreground color, background color, bold and italic
flags. More specific selectors win. syntect's `Highlighter` resolves the active scope stack to a
concrete `Style` and caches resolved stacks in a `HighlightState`, so repeated stacks cost a
lookup rather than a re-resolution.

syntect packages the two halves as `ParseState` (the context stack) plus `HighlightState` (the
resolved style stack), and offers `HighlightLines` as the convenience wrapper that owns both and
exposes one method, `highlight_line`, taking a line and returning `Vec<(Style, &str)>`: the line
broken into style runs. Those runs are exactly what `parse_diff` converts into `HighlightSpan`s.

### Incremental line parsing and why order matters

`HighlightLines` is inherently sequential: each `highlight_line` call consumes the state left by
the previous call. Feed it the lines of a file in order and it highlights the file correctly;
feed it lines out of order, or lines from two different files, and the carried stack lies. There
is no random access. This is the property that forces the two-highlighter design in the next
section: a unified diff interleaves lines from two versions of a file, and neither version's
sequence matches the interleaved order.

The cost model also follows from the design. Each line pays for regex attempts against the top
context's rules; a line that matches early and often is cheap, and pathological lines (very long,
uniform content that forces many rules to scan to the end and fail) are expensive, sometimes
super-linearly so with backtracking-capable engines. syntect compiles the grammars' Oniguruma-style
patterns and offers both the `onig` bindings and the pure-Rust `fancy-regex` engine as backends.
Either way, cost scales with line length times rule count, which is exactly why Quinjet enforces
a hard 32 KiB per-line budget rather than trusting the engine to stay linear.

Grammar selection is a separate, one-time step per file: `SyntaxSet::find_syntax_for_file` tries
the file name and extension against every grammar's declared extensions, and only if nothing
matches does it open the file to read its first line for shebang or modeline detection. Quinjet
wraps it in `syntax_for_path`, from `src/git/diff.rs`:

```rust
fn syntax_for_path<'a>(syntaxes: &'a SyntaxSet, path: Option<&Path>) -> &'a SyntaxReference {
    path.and_then(|path| syntaxes.find_syntax_for_file(path).ok().flatten())
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text())
}
```

The `.ok().flatten()` folds two distinct failures into the same fallback: "no grammar claims this
extension" and "the first-line probe could not open the file". The second failure is routine for
Quinjet, because the paths inside a PR patch are repository-relative and the corresponding file
often does not exist at that relative location on disk (PR objects live in a bare workspace under
the cache root, see ../github/pr-workspace.md). Extension matching needs no filesystem, so
`src/app.rs` gets its Rust grammar either way; an extensionless script in a PR degrades to plain
text rather than erroring.

## Two parser states per file section

A unified diff is two files shuffled into one byte stream. Removed lines belong to the old
version, added lines to the new version, and context lines to both. Sequential grammar state makes
this a trap: feed every content line into a single `HighlightLines` and each side corrupts the
other's stack. `parse_diff` documents its defense in the doc comment, quoted from
`src/git/diff.rs`:

```rust
/// Parse a unified diff and highlight code on the old and new sides independently.
/// Keeping two parser states avoids additions corrupting the old-file syntax state and
/// removals corrupting the new-file state.
pub(crate) fn parse_diff(
    raw: &[u8],
    title: impl Into<String>,
    path_hint: Option<&Path>,
    truncated: bool,
) -> DiffDocument {
```

### The corruption scenario, concretely

Take this hunk, where the old version closes a block comment and the new version replaces it with
code:

```diff
@@ -10,4 +10,4 @@
 fn ready() {
-    /* teardown pending
-    cleanup(); */
+    cleanup();
     done();
```

A single shared highlighter processes the lines in stream order. After the first removed line, its
stack holds an open block-comment context. The added line `cleanup();` is then parsed inside that
comment context and colored as comment text, wrongly. Worse, the closing `*/` that would have
popped the context lives on the second removed line, so whether the trailing context line `done();`
renders as code or comment depends on the interleaving order, not on either file's reality. State
corruption is not limited to the corrupted line: it poisons everything after it in the section.

With two states, the old-side highlighter sees `fn ready() {`, both removed lines, and `done();`,
which is a valid prefix of the old file: the comment opens and closes, `done()` is code. The
new-side highlighter sees `fn ready() {`, the added line, and `done();`, a valid prefix of the new
file. Each stack only ever receives lines that genuinely exist in its version, in that version's
order, which is the precondition `HighlightLines` requires.

### The routing in parse_diff

The state lives in two locals initialized per active path:

```rust
let mut old_highlighter = highlighter_for_path(assets, active_path.as_deref());
let mut new_highlighter = highlighter_for_path(assets, active_path.as_deref());
```

and the per-prefix arms route each content line to the correct side. Added lines, from
`src/git/diff.rs`:

```rust
if let Some(content) = raw_line.strip_prefix('+') {
    let number = new_line;
    new_line = new_line.map(|line| line + 1);
    let content = expand_tabs(content);
    let spans = highlight_optional(&mut new_highlighter, &content, assets);
```

Removed lines mirror it with `old_highlighter` and the old counter. Context lines are the
interesting arm, because both files contain them and therefore both stacks must advance:

```rust
} else if let Some(content) = raw_line.strip_prefix(' ') {
    let old_number = old_line;
    let new_number = new_line;
    old_line = old_line.map(|line| line + 1);
    new_line = new_line.map(|line| line + 1);
    let content = expand_tabs(content);
    let spans = highlight_optional(&mut new_highlighter, &content, assets);
    advance_highlighter(&mut old_highlighter, &content, assets);
```

The rendered spans come from the new-side highlighter, on the reasoning that a diff reader is
mentally reading the post-image, and the old-side highlighter is advanced with the same text while
its output is discarded, via `advance_highlighter` in `src/git/diff.rs`:

```rust
fn advance_highlighter<'a>(
    highlighter: &mut Option<HighlightLines<'a>>,
    line: &str,
    assets: Option<&'a HighlightAssets>,
) {
    if line.len() > MAX_SYNTAX_HIGHLIGHT_LINE_BYTES {
        *highlighter = None;
        return;
    }
    if let (Some(highlighter), Some(assets)) = (highlighter.as_mut(), assets) {
        drop(highlighter.highlight_line(line, &assets.syntaxes));
    }
}
```

Skipping the old-side advance would be an easy micro-optimization and a correctness bug: the next
removed line would be parsed with a stack missing every context transition the context lines
performed. Highlighting the same text twice (once for output, once discarded) is the honest price
of two independent sequential states over one interleaved stream. The cost is bounded: context
lines are at most a few per hunk under `--unified=3`.

### Reset points

Both highlighters are rebuilt whenever the active path can change, because a new path can mean a
new grammar and always means new file content:

- at every `diff --git` header (a new file section begins);
- at every `diff --cc` or `diff --combined` header (merge diffs, one path for both sides);
- at every `+++ ` header, since it finalizes the post-image path; the reset here uses the newly
  computed `active_path`, preferring the new path and falling back to the old one, from
  `src/git/diff.rs`:

```rust
if let Some(path) = raw_line.strip_prefix("+++ ") {
    let new_path = patch_path(path, "b/");
    file_mut(&mut current_file, path_hint)
        .new_path
        .clone_from(&new_path);
    active_path =
        new_path.or_else(|| current_file.as_ref().and_then(|file| file.old_path.clone()));
    old_highlighter = highlighter_for_path(assets, active_path.as_deref());
    new_highlighter = highlighter_for_path(assets, active_path.as_deref());
    continue;
}
```

One consequence: both sides of a file section use one grammar, chosen by
`FileBuilder::syntax_path`, which prefers the new path. For a rename that changes language, say
`build.js` renamed to `build.ts`, the removed lines are old-file JavaScript parsed with the
TypeScript grammar. In practice the grammars are near-identical for such pairs, and a rename that
truly changes language rewrites the content anyway; carrying two grammars per section would double
the selection bookkeeping for a case that barely occurs.

### The state that hunks cannot carry

Two highlighters fix the interleaving problem but not the sampling problem: with `--unified=3`, a
patch contains only changed lines plus three context lines around each hunk. The lines between
hunks are never fed to either highlighter, so a multi-line construct that opens in the gap (a
block comment starting between hunks, a raw string spanning the gap) is invisible to the stack,
and the first lines of the next hunk can mis-highlight. The parse also starts mid-file at each
first hunk with an empty stack, so a hunk inside a multi-line string begins in code context.

Quinjet accepts this for compact diffs and offers an escape hatch that removes it entirely: the
expanded view. `revision_diff_file` in `src/git/mod.rs` selects `--unified=1000000` when a file is
expanded, which makes Git emit effectively the whole file as context in one hunk. Every line of
the file then flows through both highlighters in true file order, and the stacks are exact from
the first byte. The reader who cares about a subtle highlight boundary gets correctness by
pressing the expand key; everyone else gets speed. The mechanism and its flags are documented with
the rest of the patch production in ./pipeline.md.

There is a second, quieter defense: mis-highlighting is bounded to a file section. Because both
highlighters are rebuilt at the next `diff --git` boundary, a bad stack never leaks across files,
and because spans only carry foreground colors, the worst case is wrongly colored text, never
wrong text, wrong line numbers, or wrong emphasis.

## two-face and the extra_newlines syntax set

syntect ships a modest default grammar collection. The
[two-face](https://docs.rs/two-face) crate packages the far larger, curated syntax collection
maintained by the [bat](https://github.com/sharkdp/bat) project, precompiled into serialized dumps
embedded in the binary, so loading is a deserialization rather than a YAML compile. Quinjet's
whole highlighting asset story is one lazily initialized static, quoted in full from
`src/git/diff.rs`:

```rust
struct HighlightAssets {
    syntaxes: SyntaxSet,
    theme: Theme,
}

static HIGHLIGHT_ASSETS: OnceLock<HighlightAssets> = OnceLock::new();

fn highlight_assets() -> &'static HighlightAssets {
    HIGHLIGHT_ASSETS.get_or_init(|| {
        let syntaxes = two_face::syntax::extra_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.values().next())
            .cloned()
            .unwrap_or_default();
        HighlightAssets { syntaxes, theme }
    })
}
```

Three decisions are folded into these lines:

**1. `extra_newlines` selects both a size and a variant.** two-face exposes the syntax collection
in two axes. The `extra` axis is coverage: the extended set carries the bat collection's grammars
beyond syntect's bundled defaults, which matters directly for a diff tool because a PR touches
whatever languages the repository contains. The `newlines` axis is the build variant: syntect
syntax sets are compiled either from newline-inclusive definitions (regexes may anchor against the
line terminator) or from stripped ones, and the newline variant is the one syntect's documentation
recommends for general use. Quinjet takes the extended set in the newline build.

**2. Initialization is once per process, and only on demand.** `OnceLock::get_or_init` runs the
deserialization the first time any patch qualifies for highlighting, and every later parse gets
the same `&'static` reference. The call site in `parse_diff` makes the laziness precise:

```rust
let assets = (raw.len() <= MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES).then(highlight_assets);
```

`bool::then` only invokes the closure when the condition holds, so a session that only ever
touches over-budget patches, or a `--json` invocation on a binary-only diff, never pays the asset
load at all. Once loaded, the set is shared by every subsequent parse on the worker thread; there
is no per-document grammar cost beyond `find_syntax_for_file`.

**3. The theme is pinned, with a graceful ladder.** `ThemeSet::load_defaults` provides syntect's
built-in themes; Quinjet asks for `base16-ocean.dark` by name, falls back to any available theme,
and finally to `Theme::default()`. The specific choice of `base16-ocean.dark` is not aesthetic, as
the next section shows: its exact RGB values function as a wire format between syntect and
Quinjet's own theme system, and the fallbacks merely guarantee that a hypothetical syntect
repackaging without the theme degrades to uncolored spans rather than a panic (unrecognized RGB
values map to `SyntaxColor::Text`).

## From base16-ocean.dark to semantic SyntaxColor

Quinjet ships 13 selectable themes, each in light and dark appearances, and parsed documents can
sit in caches for the lifetime of the process. If spans stored the RGB values syntect emits,
switching the theme would require re-highlighting every cached document: reloading grammar work
that the budgets exist to bound, for a purely cosmetic change. Quinjet removes the coupling with a
translation layer: parse once under one fixed, known syntect theme, then immediately convert every
emitted color into a small semantic enum that the active theme resolves at draw time.

### Why base16 makes a good wire format

The base16 convention defines a 16-slot palette with fixed semantic roles: eight monochrome shades
(base00 through base07, backgrounds up to foregrounds) and eight accent hues (base08 through
base0F) with conventional assignments such as variables on base08, integer and boolean constants
on base09, classes and types on base0A, strings on base0B, support and escapes on base0C,
functions on base0D, keywords on base0E, and deprecated or embedded content on base0F. A base16
theme for syntect therefore uses exactly 16 distinct RGB values, and each value corresponds to a
stable role rather than to a scattered set of per-scope decisions.

That makes any base16 theme invertible: given an output color, the role that produced it is
recoverable by exact RGB equality against the palette. `base16-ocean.dark` is one such theme, and
it ships inside syntect's `ThemeSet::load_defaults`, so it is always available without bundling
extra assets. Quinjet runs every highlight under it and inverts the palette immediately, in
`syntax_color`, quoted in full from `src/git/diff.rs`:

```rust
const fn syntax_color(color: syntect::highlighting::Color) -> SyntaxColor {
    match (color.r, color.g, color.b) {
        (101, 115, 126) => SyntaxColor::Comment,
        (191, 97, 106) => SyntaxColor::Red,
        (208, 135, 112) => SyntaxColor::Orange,
        (235, 203, 139) => SyntaxColor::Yellow,
        (163, 190, 140) => SyntaxColor::Green,
        (150, 181, 180) => SyntaxColor::Cyan,
        (143, 161, 179) => SyntaxColor::Blue,
        (180, 142, 173) => SyntaxColor::Purple,
        (171, 121, 103) => SyntaxColor::Brown,
        _ => SyntaxColor::Text,
    }
}
```

Reading the arms against the base16-ocean palette slots and their conventional roles:

| Matched RGB | Hex | Ocean slot | Conventional role | SyntaxColor |
| --- | --- | --- | --- | --- |
| (101, 115, 126) | `#65737e` | base03 | comments, invisibles | `Comment` |
| (191, 97, 106) | `#bf616a` | base08 | variables, markup deletions | `Red` |
| (208, 135, 112) | `#d08770` | base09 | integers, booleans, constants | `Orange` |
| (235, 203, 139) | `#ebcb8b` | base0A | classes, types, markup bold | `Yellow` |
| (163, 190, 140) | `#a3be8c` | base0B | strings, markup insertions | `Green` |
| (150, 181, 180) | `#96b5b4` | base0C | support, regexes, escapes | `Cyan` |
| (143, 161, 179) | `#8fa1b3` | base0D | functions, methods, headings | `Blue` |
| (180, 142, 173) | `#b48ead` | base0E | keywords, storage | `Purple` |
| (171, 121, 103) | `#ab7967` | base0F | deprecated, embedded tags | `Brown` |
| anything else | | base05 and others | plain foreground | `Text` |

The catch-all arm is the safety property: the theme's default foreground (`#c0c5ce`, base05) and
any color this table does not recognize collapse to `SyntaxColor::Text`, which renders as the
active theme's normal text color. An update to syntect that changed the theme's values would
therefore fade Quinjet's highlighting toward monochrome, not break it; and the whole function is
`const`, a compile-time-checkable table with no allocation and no branching beyond the match.

The enum being produced lives in `src/theme.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SyntaxColor {
    Text,
    Comment,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Brown,
}
```

Ten variants, one byte of meaning per span, `Copy`, and serialized as kebab-case strings, so the
`--json` output of the diff subcommands reports `"green"` or `"comment"` for each span: a stable
semantic vocabulary that downstream tooling can style however it wants, rather than RGB values
that would bake one terminal theme into machine output.

The mapping's stability is itself under test: `base16_syntax_colors_have_stable_semantic_roles` in
`src/git/diff.rs` highlights known constructs and asserts they land on the expected variants, so a
grammar or theme drift inside syntect that silently shifted, say, strings off `Green` would fail
the suite rather than quietly recolor every document.

### What the translation buys, in order of value

**1. Theme switching without re-parsing.** Changing themes or appearance rewrites only the
render-side lookup table. Every cached `DiffDocument`, including the 32 MiB PR document cache and
whatever the local diff workspaces hold, remains valid. The only invalidation a theme change
triggers is of pre-rendered row caches that embed concrete `Style` values (the PR overview rows,
whose invalidation on `set_appearance` and `apply_theme` was one of the #46 gap fixes), never of
parsed documents.

**2. Light and dark correctness.** A span highlighted while the user ran a dark theme renders
correctly when the user switches to a light theme, because the stored value never was a color. A
design that cached syntect's dark-theme RGB would paint pale yellow text on white paper after an
appearance switch.

**3. Bounded serialization.** Kebab-case variant names are self-describing and diff-friendly in
JSON output, and ten variants keep the vocabulary small enough that `src/cli/render.rs` and any
external consumer can map them exhaustively.

## Semantic resolution in src/theme.rs

The render side of the contract is `Theme::syntax`, a constant-time array lookup, from
`src/theme.rs`:

```rust
pub(crate) const fn syntax(&self, color: SyntaxColor) -> Color {
    match color {
        SyntaxColor::Text => self.syntax[0],
        SyntaxColor::Comment => self.syntax[1],
        SyntaxColor::Red => self.syntax[2],
        SyntaxColor::Orange => self.syntax[3],
        SyntaxColor::Yellow => self.syntax[4],
        SyntaxColor::Green => self.syntax[5],
        SyntaxColor::Cyan => self.syntax[6],
        SyntaxColor::Blue => self.syntax[7],
        SyntaxColor::Purple => self.syntax[8],
        SyntaxColor::Brown => self.syntax[9],
    }
}
```

`highlight_spans` calls it for every span with a stored color, from `src/ui/mod.rs`:

```rust
let foreground = span.foreground.map_or_else(
    || line_foreground(kind, theme),
    |syntax| theme.syntax(syntax),
);
```

so the full resolution path for one span is: syntect `Style` at parse time, RGB inverted to
`SyntaxColor` by the const table, stored for the document's lifetime, and mapped to the current
theme's RGB by an array index per frame. The per-frame cost is a match and an array load,
insignificant next to the cell writes it feeds.

The ten-entry `syntax` array is populated in `Theme::new` and is where the semantic roles fuse
with each theme's identity, from `src/theme.rs`:

```rust
syntax: [
    text,
    readable(color(palette[3]), &surfaces, appearance, 4.5),
    removed,
    conflict,
    modified,
    added,
    readable(color(palette[12]), &surfaces, appearance, 4.5),
    accent,
    readable(color(palette[14]), &surfaces, appearance, 4.5),
    readable(color(palette[15]), &surfaces, appearance, 4.5),
],
```

Read together with the enum order, this is a deliberate aliasing of syntax roles onto the theme's
existing semantic colors rather than a second, parallel palette:

| Index | SyntaxColor | Resolves to | Which is |
| --- | --- | --- | --- |
| 0 | `Text` | `text` | the theme's corrected foreground |
| 1 | `Comment` | `readable(palette[3])` | the border shade, lifted to 4.5 contrast |
| 2 | `Red` | `removed` | the same hue as removed diff lines |
| 3 | `Orange` | `conflict` | the conflict marker color |
| 4 | `Yellow` | `modified` | the modified-status color |
| 5 | `Green` | `added` | the same hue as added diff lines |
| 6 | `Cyan` | `readable(palette[12])` | the palette's cyan slot |
| 7 | `Blue` | `accent` | the theme accent (links, focus) |
| 8 | `Purple` | `readable(palette[14])` | the palette's purple slot |
| 9 | `Brown` | `readable(palette[15])` | the palette's last accent slot |

The aliasing keeps each theme coherent: strings share the addition green, function names share the
accent blue that also colors links and the focused border, and comments reuse the border gray
raised to text-level contrast. A theme author supplies one 16-slot palette and every syntax color
falls out; there is no way for the syntax layer to drift chromatic distance from the rest of the
UI.

`readable` is the enforcement half, quoted from `src/theme.rs`:

```rust
fn readable(
    mut foreground: Color,
    backgrounds: &[Color],
    appearance: Appearance,
    minimum: f64,
) -> Color {
    let target = match appearance {
        Appearance::Light => 0,
        Appearance::Dark => 0x00ff_ffff,
    };
    for _ in 0..64 {
        if backgrounds
            .iter()
            .all(|background| contrast(foreground, *background) >= minimum)
        {
            return foreground;
        }
        foreground = blend(color_value(foreground), target, 96);
    }
    color(target)
}
```

Each candidate color is nudged 96 percent of the way toward the appearance's pole (white for dark
themes, black for light) until it clears the minimum contrast, computed with the standard relative
luminance formula, against every surface in the list, and the surfaces list includes both
emphasis backgrounds. The intraline emphasis feature therefore imposed a real constraint on the
entire theme system: every syntax color of every theme must survive the 27 percent blend
backgrounds, and the correction loop plus the exhaustive theme test are what guarantee it rather
than convention.

## The syntax budgets: 512 KiB per patch, 32 KiB per line

Grammar parsing is the only unbounded-cost computation in the diff pipeline: regex time grows with
line length and content shape, not just byte count. Two constants at the top of `src/git/diff.rs`
bound it:

```rust
const MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES: usize = 512 * 1024;
const MAX_SYNTAX_HIGHLIGHT_LINE_BYTES: usize = 32 * 1024;
```

They act at different granularities and fail in different directions, so each deserves its own
reading.

### The 512 KiB whole-patch gate

The gate is the single line in `parse_diff` already shown for its laziness:

```rust
let assets = (raw.len() <= MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES).then(highlight_assets);
```

For a patch larger than 512 KiB, `assets` is `None`, and the `None` propagates through every
downstream helper: `highlighter_for_path` returns `None` without touching the syntax set,
`highlight_optional` and `advance_highlighter` short-circuit to plain spans, and the entire parse
runs as a structural pass that never executes a regex. The judgment encoded in the threshold: a
patch that large is dominated by generated or vendored churn (lockfiles, snapshots, bundled
output), where per-token color has no reading value, and the reader's actual need is that the
document appears instantly and scrolls smoothly. Rows still get their kinds, line numbers, file
sections, and hunk structure; the renderer still colors added lines green and removed lines red
through `line_foreground`; only the token-level chroma is absent.

The behavior is pinned by `skips_syntax_grammar_work_for_large_patches` in `src/git/diff.rs`,
which builds a patch just over 512 KiB and asserts every produced span has `foreground: None`.

The gate composes with the byte caps upstream of it. A patch reaches `parse_diff` through a capped
pipe read of at most 8 MiB (`MAX_DIFF_BYTES`), so the worst structural parse is 8 MiB of line
iteration; and per-file PR patches are cached on disk only up to 1 MiB each, so most cached reads
sit well under the highlight gate. Between 512 KiB and 8 MiB there is a band of patches that parse
but do not highlight, which is exactly the intended degradation order: structure always, color
when affordable.

### The 32 KiB per-line kill switch

Within an eligible patch, each content line passes through `highlight_optional`, quoted in full
from `src/git/diff.rs`:

```rust
fn highlight_optional<'a>(
    highlighter: &mut Option<HighlightLines<'a>>,
    line: &str,
    assets: Option<&'a HighlightAssets>,
) -> Vec<HighlightSpan> {
    if line.len() > MAX_SYNTAX_HIGHLIGHT_LINE_BYTES {
        *highlighter = None;
        return vec![HighlightSpan::plain(line)];
    }
    match (highlighter.as_mut(), assets) {
        (Some(highlighter), Some(assets)) => highlight(highlighter, line, &assets.syntaxes),
        _ => vec![HighlightSpan::plain(line)],
    }
}
```

The detail that distinguishes this from a per-line skip: `*highlighter = None` writes through the
mutable reference, permanently disabling that side's highlighter for the remainder of the file
section (it is only reconstructed at the next `diff --git` or `+++ ` reset). A naive
implementation would skip the long line and keep highlighting subsequent lines. Quinjet kills the
highlighter instead, for two reasons that reinforce each other:

**1. Performance.** A 32 KiB line in a text file is a marker for minified or generated content:
where there is one such line there are usually more, and each would pay regex time proportional to
its length before being skipped. Killing the state converts the rest of the section into plain
spans at zero grammar cost.

**2. Honesty.** Sequential grammar state means the skipped line's scope transitions are lost. If a
32 KiB minified line opens a template literal, every following line would be highlighted with a
stack that never saw the opening. Continuing to highlight would present confidently wrong colors;
degrading to plain presents no claim at all. The kill switch turns "state is now unreliable" into
"state no longer exists", which the renderer expresses uniformly.

`advance_highlighter` applies the identical check on the discarded-output path, so a monster
context line kills the old-side state just as a monster added line kills the new side. The two
sides die independently: a patch whose old version contains the minified line but whose new
version replaces it with formatted code keeps full highlighting on the new side.

The test `skips_syntax_grammar_work_for_very_long_lines` in `src/git/diff.rs` pins the behavior
with a single line just over 32 KiB, asserting only plain spans come back. And as noted earlier,
the same 32 KiB figure bounds the intraline pair scan in the render layer, so the two systems
agree on which lines are beyond analysis.

### The degradation ladder, end to end

Putting the budgets together, a file's rows land on one of four rungs:

| Condition | Grammar work | Span colors | Intraline emphasis |
| --- | --- | --- | --- |
| patch <= 512 KiB, lines <= 32 KiB | full | semantic per token | full, per visible pair |
| patch <= 512 KiB, a line > 32 KiB | until that line, per side | plain from there on | skipped for over-cap pairs only |
| patch > 512 KiB | none | all plain | still full for pairs under 32 KiB |
| binary or meta content | none | plain meta rows | never (kind check) |

The third row is worth noticing: the two systems degrade independently. A 600 KiB patch gets no
syntax color, but its short paired lines still get intraline emphasis, because emphasis reads span
text, not span colors. The reader of a giant patch keeps the single most diff-relevant visual cue
even after the more expensive cue is shed.

## Upstream feeders and the 32 MiB document budget

The budgets above bound one parse. What bounds the total highlighting work of a session is how
patches arrive and how long their parsed forms are retained. Both mechanisms live outside
`src/git/diff.rs` but exist substantially because parsing and highlighting are the expensive step
they feed.

### How patch bytes reach parse_diff

Every source funnels into the same parser. The working tree, commits, branch comparisons, and
stashes produce patches through path-scoped `git diff` invocations in `src/git/mod.rs`, each read
through a capped pipe of `MAX_DIFF_BYTES` (8 MiB) that kills the child process on overflow and
trims the buffer back to the last complete line, so `parse_diff` never sees a torn final row. Pull
requests add a batching layer: `PreparedPullRequest::diff_files` in `src/git/github/mod.rs` asks
Git for up to 32 files in one invocation and cuts the combined output back apart at its
`diff --git` boundaries with `split_patch_by_file`, so each file still parses and highlights as
its own document. The full plumbing, including the header-first index that renders before any
patch exists, is documented in ./pipeline.md.

The order in which a huge pull request's files get parsed has its own history, and it matters for
highlighting because parsing is when highlighting happens:

**1. PR #50 introduced smallest-first size tiers.** Once a pull request crossed roughly 100,000
total lines or 1,000 files, the background prefetch began ordering candidate files into size tiers
and fetching the smallest first, so the many cheap documents landed (and were parsed and
highlighted) before the few expensive ones monopolized the pipe.

**2. PR #55 replaced that ordering with viewport-anchored wrap-around prefetch, the current
behavior.** The walk now starts at the file the Files tree is currently showing
(`prefetch_anchor_index` in `src/app.rs`) and wraps around the whole index in order, batching up
to `PULL_REQUEST_PREFETCH_BATCH = 32` files per Git invocation under a
`PULL_REQUEST_PREFETCH_BYTE_BUDGET = 6 * 1024 * 1024` estimated byte budget, with per-file
estimates of `(additions + deletions) * 80 + 4096` bytes
(`PULL_REQUEST_PATCH_LINE_ESTIMATE = 80`) and a `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE = 512 KiB`
when a file has no counts, up to a total of `MAX_PREFETCHED_PULL_REQUEST_FILES = 4_096` files. The
estimator, from `src/app.rs`:

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

Viewport anchoring supersedes the size tiers because it optimizes the metric that matters for a
reader: the files on screen get their patches, and therefore their highlighting, first, and the
rest of the index fills in behind them. The 6 MiB budget stays under the 8 MiB pipe cap so a
full batch survives the read, and a single file whose estimate alone exceeds the budget still
travels alone, guaranteeing progress. The scheduling story, including the mailbox lane that keeps
prefetch batches from displacing a reader's own preview, is told in ../github/prefetch.md and
../rendering/progressive-loading.md; the measured behavior on the benchmark PR is in
../benchmarking.md. For scale, the PR #47 evidence comment on the same benchmark (loading
oven-sh/bun#30412, 2,188 files, from a shallow `blob:none` clone) reports
`time quinjet pr files 30412` at 6.30 s cold and 0.04 s warm, and a single-file
`quinjet pr diff 30412 .buildkite/ci.mjs` at 0.10 s: the per-file path, which is one bounded Git
diff plus one `parse_diff`, is fast enough that highlighting a file on demand is imperceptible.

### The 32 MiB parsed-document budget

A parsed `DiffDocument` is bigger than its raw patch: every line becomes a struct, every style run
its own heap-allocated `String` plus flags. Quinjet therefore budgets the parsed representation
separately from the raw bytes. PR file documents accumulate in
`App.pull_request_documents: HashMap<PathBuf, DiffDocument>` with an insertion-order queue and a
running size estimate, pruned oldest-first against `MAX_PULL_REQUEST_DOCUMENT_BYTES = 32 MiB`,
from `src/app.rs`:

```rust
fn prune_pull_request_documents(&mut self, maximum_bytes: usize) {
    while self.pull_request_document_bytes > maximum_bytes
        && self.pull_request_documents.len() > 1
    {
        let Some(expired) = self.pull_request_document_order.pop_front() else {
            break;
        };
        if let Some(document) = self.pull_request_documents.remove(&expired) {
            self.pull_request_document_bytes = self
                .pull_request_document_bytes
                .saturating_sub(diff_document_size(&document));
        }
    }
}
```

The `len() > 1` guard means the newest document always survives even if it alone exceeds the
budget: evicting the document that was just parsed to make room for nothing would throw away work
with no beneficiary. The size estimate walks the actual retained allocations, from `src/app.rs`:

```rust
fn diff_document_size(document: &DiffDocument) -> usize {
    let lines = document.lines.iter().fold(0_usize, |total, line| {
        let spans = line.spans.iter().fold(0_usize, |span_total, span| {
            span_total.saturating_add(size_of_val(span) + span.text.capacity())
        });
        total
            .saturating_add(size_of_val(line))
            .saturating_add(spans)
    });
    size_of_val(document)
        .saturating_add(document.title.capacity())
        .saturating_add(lines)
}
```

Counting `capacity()` rather than `len()` matters: a `String`'s heap block is its capacity, and
under-counting would let the real footprint drift above the budget. This estimator is also why the
in-memory budget (32 MiB) is four times the raw pipe cap (8 MiB): the parsed form's per-line and
per-span struct overhead multiplies the byte count, and highlighting multiplies the span count
(one span per style run instead of one per line).

Eviction is loss of parse work, not of data: the raw patch usually still sits in the on-disk gh
cache under its immutable `pr-patch-v1` key (bounded at 1 MiB per file), so re-opening an evicted
file re-parses from disk without touching the network or Git. The cache design that makes those
keys immutable is covered in ../github/caching.md, and the object-identity reasoning behind it in
../git-internals/object-model.md.

Highlighting also shapes one subtle policy here: because syntax spans make documents expensive to
produce, documents are moved rather than copied wherever possible. Leaving a single-file PR view
moves the current document into the cache (`cache_current_pull_request_single_document`), and the
combined all-files view materializes collapsed files as synthetic headers instead of cloning their
cached documents into the combined body (invariant 6's "collapsed cached patches are not cloned
into the combined document"), so a 2,000-file PR with everything collapsed renders from an index,
not from 2,000 concatenated span forests.

## Design alternatives and why they lost

Each mechanism on this page displaced at least one plausible alternative. Collecting them in one
place shows the shared reasoning: bound the worst case first, keep the render thread out of data
work, and prefer degrading a cosmetic layer over stalling a structural one.

**1. Word-level LCS for intraline emphasis.** The highest-fidelity option: tokenize both lines,
run a shortest-edit-script diff over tokens, emit per-token ranges. Lost on three counts. Cost: an
O(ND) diff per visible pair per frame versus one linear scan. Complexity: a tokenizer needs
per-language rules to avoid absurd tokens inside strings and operators. Renderer fit: multiple
disjoint ranges per line would complicate `highlight_spans`, whose three-piece split depends on
the range being contiguous. The affix scan is exact for single-edit lines, which dominate real
diffs, and degrades to a wider-than-minimal range otherwise.

**2. Character-level minimal diff.** Even the minimal character ranges are often worse to read
than the affix range, because scattered single-character matches inside changed regions fragment
the highlight. Editors that ship this add merge heuristics to re-widen the ranges toward exactly
what the affix scan produces in one pass.

**3. Similarity-scored pairing instead of positional pairing.** Score each removed x added
combination in a block (say by common-affix length) and pair greedily, so a deleted comment above
a modified line does not shift every pairing by one. Lost on cost and predictability: scoring is
O(removed_run x added_run) text scans per block, unbounded for tall blocks, and the greedy
assignment can flip pairings as the user scrolls new rows into view (the visible subset would
influence nothing, but recomputing scores per frame would). Positional pairing is O(1) arithmetic
per row, stable under scrolling, and matches what the side-by-side layout displays, so the
emphasis and the visual pairing can never disagree.

**4. Computing emphasis at parse time and storing it in the document.** Attractive symmetry with
syntax spans, and it would amortize the work across frames. Lost because the workload shape is
opposite: syntax highlighting is expensive per line and viewport-invariant, emphasis is cheap per
line and only meaningful for visible pairs. Storing it would grow every document (two ranges per
paired row, present for the 99 percent of rows never scrolled to), complicate the parser with
pairing logic that the renderer already needs for side-by-side layout, and still require a render
pass to intersect ranges with spans. The measured render cost after #46 is O(viewport), which
storage could not improve.

**5. Caching the per-frame emphasis map.** Covered in the #46 section: the natural cache key
changes on every scroll tick, so the cache would miss exactly under load, and the rebuild it would
save is O(viewport) with small constants.

**6. tree-sitter instead of syntect.** tree-sitter parses incrementally into real syntax trees
and would give more accurate highlighting, plus true incremental re-parse on edits. Lost for this
workload: grammars are per-language native artifacts that complicate a static Rust binary; a diff
is not an editable buffer, so incremental re-parse buys nothing (documents are parsed once); and
diffs are fragments, which tree-sitter handles by error recovery on every hunk boundary versus
syntect's tolerant regex scanning that simply starts matching wherever it is. The Sublime grammar
ecosystem, via two-face, also covers more file types than compiled-in tree-sitter grammars
practically can.

**7. Highlighting at render time instead of parse time.** The dual of alternative 4: run syntect
over only visible rows each frame, and skip parse-time work entirely. Lost on the sequential-state
property: correct highlighting of row N requires the grammar stack from rows 0..N, so a render
pass starting mid-file would either re-run from the top of the file section (O(document) per
frame, the exact regression #46 removed for emphasis) or accept stateless per-line highlighting,
which breaks multi-line constructs. Parse-time highlighting pays the sequential cost exactly once,
off the render thread, in the worker that already owns the patch bytes (the threading split is
covered in ../rendering/concurrency.md).

**8. Storing RGB and re-highlighting on theme change.** Simpler than the `SyntaxColor`
indirection. Lost the moment the theme picker existed: re-parsing every cached document on a theme
or appearance switch would turn a cosmetic toggle into seconds of grammar work on a large PR, and
caches keyed by content would need theme-aware keys or wholesale invalidation. The semantic enum
makes theme switching O(1) per span at draw time with no invalidation at all.

**9. A configurable parse theme instead of pinned base16-ocean.dark.** Letting users pick the
syntect theme would break the RGB inversion table, which depends on the parse theme's exact
palette values. Since the parse theme is invisible (only the semantic roles survive translation),
configurability would offer nothing except the ability to break the mapping. The user-facing
theme choice lives entirely in `src/theme.rs`, where all 13 themes resolve the same ten variants.

**10. Per-line budgets enforced by truncation instead of a kill switch.** Highlighting only the
first 32 KiB of a monster line would keep partial color. Lost on state honesty: the un-highlighted
tail can contain scope transitions, so continuing on later lines with the pre-truncation stack
would color them wrongly. Killing the highlighter for the section is the only cheap option that
never presents a wrong stack, and pairs with the same-valued intraline cap for a consistent
"beyond analysis" boundary.

## Edge cases and failure modes

The catalog below records behaviors that follow from the design but are easy to miss, each
traceable to a specific line of code quoted or cited above.

**1. Tab expansion happens before both systems.** `expand_tabs` rewrites tabs to spaces (stops
every `TAB_WIDTH = 4` columns) before `highlight_optional` sees the line, so stored span text,
`DiffLine::text()`, emphasis byte offsets, and display-width math all describe the same
tab-free string. A tab-indented file diffs with aligned emphasis precisely because no coordinate
system ever contains a tab. The behavior is pinned by
`preserves_space_indentation_and_expands_tabs_to_tab_stops` in `src/git/diff.rs`.

**2. Whitespace-only changes are visible.** An indentation change such as `    foo` to
`        foo` produces `(None, Some(4..8))`: the prefix claims the four common spaces, the suffix
claims `foo` and stops at the prefix boundary, and the four inserted spaces get the emphasis
background. Because emphasis is a background, not a foreground, changed whitespace renders as a
visible colored block, one of the few terminal-friendly ways to show an invisible edit.

**3. Identical paired lines emphasize nothing.** Positional pairing can align equal lines;
`changed_ranges` returns `(None, None)` and the rows keep only their line-level tint. No special
case needed: the general algorithm produces the right answer.

**4. Pure insertions and deletions emphasize one side only.** `    for item in list {` against
`    for item in &list {` yields `(None, Some(16..17))`: only the `&` is emphasized, and the
removed row shows no intraline mark because none of its bytes changed. The `Option` per side is
the API expressing this asymmetry.

**5. Repeated characters attribute the change as late as possible.** The `"aa"` to `"aaa"` case
lands the emphasis on the final `a` because the prefix greedily claims two characters and the
suffix guard yields. Any answer marking one inserted `a` is correct; the algorithm picks the
rightmost deterministic one.

**6. Non-UTF-8 patch bytes cannot desynchronize the systems.** `parse_diff` decodes with
`String::from_utf8_lossy`, so invalid sequences become U+FFFD replacement characters in span text.
Emphasis, spans, and width math all operate on the post-replacement string, so offsets stay
mutually consistent; the replacement character simply participates in prefix and suffix matching
like any other char.

**7. Binary files opt out everywhere.** A `Binary files ` or `GIT binary patch` line marks the
whole section binary; every subsequent line becomes a `Meta` row, which fails both the emphasis
kind check and the content-highlighting path. The `\ No newline at end of file` marker likewise
falls through to a `Meta` row (its `\` prefix matches no content arm) and is displayed but never
analyzed.

**8. Hunk headers never reach the screen.** `parse_diff` keeps `@@` rows as `HunkHeader` lines
for structure, but both layout builders skip them (`unified_row_indices` and `side_by_side_rows`),
so they can never appear in the visible index stream fed to `visible_intraline_emphasis`. The test
`hides_raw_hunk_coordinates_in_both_diff_layouts` in `src/ui/mod.rs` pins the invisibility.

**9. Collapsed files cost nothing.** A collapsed file contributes only its `FileHeader` row to
the row lists; header rows fail the emphasis kind check, and the file's body rows are absent from
the visible stream entirely. Folding a 100,000-row file therefore removes it from per-frame
emphasis consideration without any dedicated logic.

**10. A replacement block can straddle a fold or file boundary only in theory.** Runs are scanned
over `document.lines`, and a file's rows are bracketed by `FileHeader` and `FileFooter` rows,
which terminate any `Removed` or `Added` run. Two adjacent files can therefore never pair across
their boundary: the intervening footer and header rows break the runs by kind.

**11. Emphasis under horizontal scroll clips by columns, not bytes.** The byte-range split happens
before `skip` consumption in `push_highlight_piece`, so scrolling sideways through an emphasized
region slides the background with the text, including through double-width characters, because
`slice_width` and `text.width()` measure display columns.

**12. Rename sections highlight both sides with the post-image grammar.** `syntax_path` prefers
`new_path`, so `build.js` renamed to `build.ts` parses its removed JavaScript lines with the
TypeScript grammar. Accepted mis-fit; see the reset-points discussion above.

**13. The prefix guard makes overlapping claims impossible.** `old_index < prefix || new_index <
prefix` in the suffix loop guarantees `prefix <= old_end` and `prefix <= new_end`, so the returned
ranges are always well-formed; the `(prefix < end)` checks then convert the degenerate equalities
to `None`. There is no input for which `changed_ranges` returns an inverted range.

**14. Over-cap lines degrade the pair, not the document.** A 40 KiB line pair skips emphasis; the
pairs above and below it in the same block still compute normally, because the cap check runs per
pair inside `paired_intraline_emphasis`, not per block.

## The tests that pin the contract

Both halves of this page are covered by focused unit tests, worth listing because they define the
observable contract more precisely than prose can.

In `src/git/diff.rs`:

- `highlights_typescript_and_hides_git_transport_headers`: a `.tsx` patch produces exactly five
  rows (header, hunk header, removed, added, footer) and the added row carries more than one
  distinct foreground, proving the grammar actually ran and transport noise was dropped.
- `base16_syntax_colors_have_stable_semantic_roles`: the RGB inversion table maps known constructs
  to the expected `SyntaxColor` variants, guarding against silent drift in syntect's theme or
  grammars.
- `skips_syntax_grammar_work_for_large_patches`: a patch just over 512 KiB yields only
  `foreground: None` spans.
- `skips_syntax_grammar_work_for_very_long_lines`: a single line over 32 KiB yields only plain
  spans.
- `preserves_space_indentation_and_expands_tabs_to_tab_stops`: `"\tnewValue,"` becomes
  `"    newValue,"` and `"\t  nestedValue,"` becomes `"      nestedValue,"`, fixing the coordinate
  system every later computation shares.
- `parses_hunks_and_tracks_line_numbers`: the old and new line counters that give emphasis rows
  their gutter numbers advance correctly through mixed hunks.

In `src/ui/mod.rs`:

- `computes_vscode_style_intraline_changed_ranges`: the three canonical `changed_ranges` results,
  including `(Some(6..18), Some(6..18))` for the `oldValue`/`newValue` pair and `(None, None)` for
  identical lines.
- `visible_intraline_emphasis_matches_block_pairing`: positional pairing, surplus-line exclusion,
  the unpartnered added run, and the partner-outside-the-viewport property, all against one
  six-line document.
- `skips_intraline_work_for_very_long_rows`: the 32 KiB pair cap bails to `(None, None)`.
- `side_by_side_pairs_replacements`: the split layout's positional `Split` pairing, which the
  emphasis path must mirror.
- `diff_rows_are_cached_between_draws_and_rebuilt_on_document_change`: the row lists that carry
  the visible indices into `visible_intraline_emphasis` are pointer-stable across frames until the
  document changes.
- `hides_raw_hunk_coordinates_in_both_diff_layouts` and
  `collapse_all_keeps_only_selectable_file_headers`: the row streams that reach the emphasis loop
  contain no hunk headers and no collapsed bodies.

In `src/theme.rs`:

- `every_theme_keeps_text_and_graphics_readable_on_every_surface`: every syntax and semantic
  foreground clears 4.5 contrast on both emphasis backgrounds across all 13 themes and both
  appearances, the guarantee that makes the 27 percent blend safe to stack under any highlighted
  token.

Together the tests encode the page's two theses: syntax color is a parse-time, semantic,
budget-bounded property of the document, and intraline emphasis is a render-time, viewport-bounded
property of a frame. Everything else, the caps, the two parser states, the blend arithmetic, and
the #46 rescoping, exists to keep those two computations inside their budgets on the largest pull
requests Quinjet is pointed at.
