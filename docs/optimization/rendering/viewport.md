# Viewport rendering and terminal frame economics

Quinjet redraws its entire interface every frame, and stays fast anyway. This page explains why
that combination works: what a terminal frame actually costs, how ratatui's immediate-mode buffer
diffing pushes the expensive part of a redraw down to the changed cells, and the discipline Quinjet
layers on top so that the part ratatui cannot protect, the cost of composing the frame, scales with
the viewport instead of with the document. It covers the render path that never spawns Git, the
universal skip/take windowing pattern, the diff row layout cache and its invalidation, horizontal
outgrowth versus pre-wrapped prose, anchored headers and one-shot step reveal, the per-frame mouse
hit map, the build-once Files tree, wheel panning decoupled from selection, the way the viewport
steers background patch loading, and the compile-time icon catalogs. The companion pages
[progressive loading](./progressive-loading.md) and [concurrency](./concurrency.md) cover how data
arrives; this page covers what happens once it is in memory and has to reach the screen.

## Contents

- [Terminal rendering economics](#terminal-rendering-economics)
- [A render path that never spawns Git](#a-render-path-that-never-spawns-git)
- [Viewport-only drawing everywhere](#viewport-only-drawing-everywhere)
- [The diff row layout cache](#the-diff-row-layout-cache)
- [Intraline emphasis stays inside the viewport](#intraline-emphasis-stays-inside-the-viewport)
- [The overview pane: rows composed from state](#the-overview-pane-rows-composed-from-state)
- [Horizontal outgrowth versus wrapped prose](#horizontal-outgrowth-versus-wrapped-prose)
- [Anchored headers, step reveal, and End clamping](#anchored-headers-step-reveal-and-end-clamping)
- [The mouse hit map](#the-mouse-hit-map)
- [The Files tree is built once](#the-files-tree-is-built-once)
- [Wheel panning decoupled from selection](#wheel-panning-decoupled-from-selection)
- [Where the viewport steers background loading](#where-the-viewport-steers-background-loading)
- [Compile-time icon catalogs](#compile-time-icon-catalogs)
- [Every rendering bound in one place](#every-rendering-bound-in-one-place)
- [One frame, end to end](#one-frame-end-to-end)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [Design alternatives that lost](#design-alternatives-that-lost)
- [Related pages](#related-pages)

## Terminal rendering economics

### The terminal as an output device

A terminal is a character grid addressed through an in-band byte protocol. The application does not
own a framebuffer; it writes a stream of bytes to a pseudo-terminal, and the terminal emulator on
the other side parses that stream, updates its own grid model, and rasterizes glyphs. Three kinds
of bytes matter for cost accounting:

- Printable text advances the cursor and replaces cells, one cell per column of display width.
- CSI control sequences position the cursor (`ESC [ row ; col H`) and change the pen state
  (`ESC [ ... m`, the SGR sequence that selects colors, bold, italic, underline).
- OSC sequences carry out-of-band payloads such as window titles and hyperlinks
  (`ESC ] 8 ; ; url ST` opens a hyperlink region, an empty one closes it).

A fully repainted frame therefore costs, at minimum, one glyph per cell, plus an SGR sequence at
every style boundary, plus a cursor move at every discontinuity. On a modest 80x24 grid that is
1,920 cells; on a large modern terminal at 200x60 it is 12,000 cells. A styled cell can easily need
ten to twenty bytes once its color changes are counted, so naive full repaints push tens of
kilobytes per frame through the PTY. Locally that wastes CPU in the emulator's parser; over SSH it
wastes round trips and bandwidth, and the interface visibly smears as partial frames arrive.

The byte stream is also strictly sequential. There is no damage-rectangle API and no compositor:
the only way to change one cell without touching its neighbors is to move the cursor there and
write exactly that cell. Efficient terminal rendering is therefore a diffing problem: compute the
smallest set of cell writes that transforms what the terminal currently shows into what the next
frame should show, and emit only those.

### Immediate mode and retained mode

There are two classic architectures for driving a display from application state.

**Retained mode** keeps a long-lived tree of widget objects. The application mutates the tree, the
framework marks the mutated subtrees dirty, and a render pass walks only the dirty parts. Browsers
and desktop toolkits work this way. The strength is incrementality; the price is that every piece
of application state needs a corresponding tree node, every state change needs a correct
invalidation, and the tree itself is a second copy of the truth that can drift from the first.

**Immediate mode** keeps no tree. Every frame, the application re-describes the entire desired
screen from its own state, and the framework figures out what actually changed. The strength is
that there is nothing to keep consistent: the screen is a pure function of state, and a forgotten
invalidation is impossible by construction. The price is that the describe step runs every frame,
so its cost must be bounded by the application.

Immediate mode fits terminals unusually well. The output space is small (a grid of cells, not a
scene graph), which makes the per-frame describe step cheap if the application only describes what
is visible, and it makes the diff step trivial: two grids of equal size compare cell by cell.
Quinjet uses [ratatui](https://docs.rs/ratatui), the standard immediate-mode terminal UI library
for Rust, with [crossterm](https://docs.rs/crossterm) as the backend that owns the raw byte
protocol.

### How ratatui turns a frame into bytes

ratatui's `Terminal` owns two `Buffer` values: the frame being composed and the frame currently on
screen. A `Buffer` is a dense vector of `Cell`s, each holding a grapheme, foreground and background
colors, and modifier flags. One frame proceeds in three steps:

1. The application's draw callback receives a `Frame` and renders widgets into the back buffer.
   Widgets are plain values built on the stack; rendering them writes cells and drops them.
2. On flush, the two buffers are compared cell by cell. The result is the list of positions whose
   cell content differs from the previous frame.
3. The backend emits one cursor move per run of contiguous changed cells, the minimal SGR changes
   between them, and the changed glyphs. The buffers swap roles and the frame is done.

A worked example makes the write-side savings concrete. Suppose a 12-column, 3-row region shows a
file list and only the selection marker moves between two frames:

```text
frame N          frame N+1        cells emitted for N+1
> src/app.rs     src/app.rs      (0,0) ' '   (0,1) '>'
  src/ui.rs    > src/ui.rs       plus the style runs covering
  src/git.rs     src/git.rs      the two changed rows
```

Out of 36 cells, only the cells whose symbol or style changed are written. Scrolling a diff by one
row is the pathological case for this scheme, since every cell in the pane changes, but even then
the cost is bounded by the pane area, never by the document behind it.

The version Quinjet builds against (ratatui 0.30, per `Cargo.toml`) also exposes a per-cell
`CellDiffOption`, which lets a renderer mark a cell as needing special diff treatment. Quinjet uses
it in exactly one place, terminal hyperlinks, covered in
[the never-spawn-Git section](#a-render-path-that-never-spawns-git) below: a cell whose symbol has
OSC 8 escape sequences embedded in it declares a forced width of one column so the diff does not
misjudge the display width of the escape-laden symbol.

### What buffer diffing does not protect

Buffer diffing bounds the bytes written to the terminal. It does nothing for the cost of step 1,
composing the frame. If the draw callback walks a million-line diff document to build widgets for
rows that end up outside the pane, ratatui will dutifully diff the same final buffer and emit few
bytes, but the CPU time is already spent. Immediate mode moves the performance burden from the
framework to the application: the screen is recomputed every frame, so everything the application
computes per frame must be proportional to the screen, not to the data.

That is the single organizing rule of Quinjet's render layer, and the rest of this page is a tour
of its enforcement:

- Every scrollable surface renders only the rows inside its window (skip/take everywhere).
- Derived layouts that are expensive to compute (diff row lists, wrapped overview rows) are cached
  on `App` and rebuilt only when their inputs change, with explicit generation counters.
- Per-row enrichment that is expensive (intraline emphasis) is computed only for visible rows.
- Structures that describe the whole dataset (the Files tree) are built once per data change, not
  per frame.
- Anything static (icon catalogs, help rows) is a compile-time constant.

The minimum terminal size gives the frame a hard floor: `draw` refuses to render below 72x18 and
shows a centered 50x8 card instead. The maximum is whatever the user's terminal provides, and every
per-frame cost above is O(that area) or O(cached-pointer reuse), never O(document).

## A render path that never spawns Git

### The rule and where it comes from

ARCHITECTURE.md states the design goal directly: "The terminal render path never spawns Git or
performs filesystem traversal." Responsiveness invariant 1 restates it from the thread's point of
view: "The UI thread mutates only in-memory state and renders visible rows." Invariant 1b extends
it to assets: file icons come from compile-time catalogs, and "Rendering never reads SVGs, font
files, configuration, or the filesystem."

The consequence is architectural, not stylistic. Because a frame can never block on a subprocess,
a pipe, or a disk read, frame time is bounded by CPU work over in-memory state, and input latency
is bounded by frame time. All Git and GitHub work happens on the worker thread behind a bounded
command queue, and its results arrive as typed snapshots that mutate `App` between frames; the
threading model is the subject of [the concurrency page](./concurrency.md). The render layer's
contract is narrower and easier to audit: `draw` takes `&mut App` and a `&Theme`, and everything it
does is read state, write cells, and update the caches and geometry that live on `App` itself.

### The draw entry point

The whole draw layer is one module, `src/ui/mod.rs`, and its entry point is `draw` at
src/ui/mod.rs:404, called once per frame by the event loop. Its opening establishes the fixed
frame geometry, from src/ui/mod.rs:

```rust
pub(crate) fn draw(frame: &mut Frame<'_>, app: &mut App, theme: &Theme) {
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        frame.area(),
    );

    if frame.area().width < 72 || frame.area().height < 18 {
        draw_too_small(frame, theme);
        return;
    }

    let [tabs, main, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(2),
    ])
    .areas(frame.area());
    let maximum_sidebar = main.width.saturating_sub(32).max(22);
    app.sidebar_width = app.sidebar_width.clamp(22, maximum_sidebar);
```

Reading it top to bottom:

**1. The background fill is the first widget.** Every cell of the frame gets the theme background
before anything else renders, so no pass afterwards has to worry about clearing stale content; the
buffer diff absorbs the cost of cells that end up unchanged.

**2. The minimum size gate is a real render path, not an assertion.** Below 72 columns or 18 rows,
`draw_too_small` renders a centered 50x8 box asking for "Resize to at least 72 × 18" and the frame
ends. Every layout constant downstream (the 12 reserved columns of a unified diff row, the 22-cell
minimum sidebar, the 31-cell minimum content pane) is chosen to be satisfiable at exactly that
floor, which is what makes the arithmetic below safe with plain saturating operations instead of
per-line defensive checks.

**3. The vertical skeleton is constant.** Three rows of tabs, a main region of at least 8 rows, two
rows of footer. The sidebar width is a live, user-draggable value, clamped every frame to
`22..=main.width - 32` so the content pane always keeps at least its own minimum; when the sidebar
is hidden the whole main region becomes content. The horizontal split reserves one column for the
divider and gives the content pane `Constraint::Min(31)`.

### The pass order and geometry assembly

After layout, `draw` runs its passes in a fixed order: `draw_tabs`, `draw_sidebar`,
`draw_main_divider` (a one-column "│" rule that highlights while dragging), `draw_content`,
`draw_jump_to_bottom` (whose hit, if visible, is appended to the SCM action hits), and
`draw_footer`. Each pass returns the clickable regions it produced, and `draw` assembles them into
one value, from src/ui/mod.rs:

```rust
    app.geometry = UiGeometry {
        changes_tab,
        history_tab,
        pull_requests_tab,
        main,
        sidebar: sidebar_area,
        sidebar_divider,
        content: content_area,
        diff_divider,
        sidebar_hits,
        scm_action_hits,
        modal_action_hits: Vec::new(),
        content_file_hits,
        content_step_hits,
        link_hits,
        help_hits: Vec::new(),
        project_hits,
    };
```

The hit map is a per-frame product of rendering, which is what makes it viewport-scoped for free;
[its own section](#the-mouse-hit-map) walks through the record types. Two overlay passes follow the
geometry assembly, and both read the finished buffer rather than app data.

### The frame snapshot and the selection overlay

`snapshot_cells` (src/ui/mod.rs:495) copies the first character of every cell in the frame into
`app.rendered_cells: Vec<Vec<char>>`. This is the data source for drag-select copy: when the user
drags across the screen, the copied text comes from what was actually rendered, not from any
model, so what you see is exactly what you copy. It is an O(cells) copy per frame, part of the
fixed frame cost, and it means the copy feature needs no cooperation from any widget.

`draw_text_selection` then recolors the cells between the selection's ordered endpoints with the
selection colors. `TextSelection` (src/app.rs:376) carries the pane rectangle it was started in,
and every row of the selection is clamped to that pane, so a drag inside one half of a side-by-side
diff never bleeds across the divider into the other file version (pinned by the test at
src/ui/mod.rs:7106).

### Terminal hyperlinks without layout cost

When no modal is open, `draw_terminal_links` embeds OSC 8 hyperlink escapes directly into the
symbols of cells inside link hit areas, so terminals that support hyperlinks make the rendered text
clickable. The mechanism, from src/ui/mod.rs:

```rust
let symbol = cell.symbol().to_owned();
cell.set_symbol(&format!("\x1b]8;;{url}\x1b\\{symbol}\x1b]8;;\x1b\\"))
    .diff_option = CellDiffOption::ForcedWidth(NonZeroU16::MIN);
```

The escape sequences are zero-width in the terminal but not zero-length in the cell's symbol
string, so the cell declares `ForcedWidth` of one column and the buffer diff treats it as a normal
single cell. URLs containing control characters are skipped entirely (a URL is attacker-influenced
data from PR metadata, and letting a control character into an escape sequence would corrupt the
stream). The links are only embedded when mouse capture is off or a link is hovered, and
`draw_link_hover` adds `Modifier::UNDERLINED` to the hovered hit's cells so hovering gives visual
feedback without re-rendering anything. All of this happens after the frame is composed, as pure
cell rewrites: link decoration costs nothing during layout.

The last two passes are `draw_modal` (when a modal is open) and `draw_toast`, both drawing over the
finished frame. Nothing in any pass opens a file, spawns a process, or blocks; every glyph placed
came from `App` state, a compile-time constant, or the theme.

## Viewport-only drawing everywhere

### The pattern

Every scrollable surface in Quinjet follows one pattern: keep a scroll offset, compute the window
of rows that intersects the pane, and build widgets for exactly those rows. Nothing offscreen is
allocated, styled, measured, or hit-registered. In code the pattern is a `skip`/`take` over a row
list, paired with an offset-maintenance function that keeps the selection visible.

The offset maintenance is the classic follow-cursor clamp, `ensure_offset` at src/ui/mod.rs:6473:

```rust
fn ensure_offset(offset: &mut usize, cursor: usize, height: usize, length: usize) {
    if height == 0 || length == 0 {
        *offset = 0;
        return;
    }
    if cursor < *offset {
        *offset = cursor;
    } else if cursor >= *offset + height {
        *offset = cursor + 1 - height;
    }
    *offset = (*offset).min(length.saturating_sub(height));
}
```

Three properties are worth naming because every windowed list in the codebase inherits them:

- The cursor is always inside `[offset, offset + height)` after the call, so a selection can never
  be scrolled out of existence by its own list.
- The final clamp guarantees the window never runs past the end, so the last page is always full
  when the list is longer than the pane.
- Degenerate panes (zero height, empty list) reset the offset to zero instead of underflowing.

The sidebar lists use a wheel-aware variant of this function, `App::sidebar_viewport`, described in
[the wheel panning section](#wheel-panning-decoupled-from-selection); its interior clamp is the
same logic.

### Where the pattern appears

The windowing shows up in every list-shaped surface, with the offset source varying by pane:

- The Changes sidebar computes `let end = (app.sidebar_offset + height).min(rows.len());` and
  iterates `rows.iter().take(end).skip(app.sidebar_offset)` (src/ui/mod.rs:856).
- The History sidebar windows `app.visible_commit_indices()` the same way (src/ui/mod.rs:1197).
- The PR Files tree iterates `rows.iter().skip(app.sidebar_offset).take(area.height as usize)`
  (src/ui/mod.rs:1765), where `rows` is the cached flattened tree.
- The PR check list windows `app.check_list_rows()` (src/ui/mod.rs:1918).
- The PR overview pane iterates `rows.iter().skip(app.content_scroll).take(inner.height as usize)`
  over its cached composed rows (src/ui/mod.rs:2282).
- The unified diff iterates `rows.iter().copied().skip(diff_scroll).take(content_height as usize)`
  over the cached row-index list (src/ui/mod.rs:4144), and the side-by-side diff does the same over
  cached `SideBySideRow`s (src/ui/mod.rs:4379).
- Every modal picker (branches, history branches, compare, stashes, projects, repositories, the
  command palette, choice pickers) derives a keep-selection-visible offset as
  `selected.saturating_sub(list_area.height.saturating_sub(1) as usize)` and then windows with
  `.skip(offset).take(list_area.height as usize)`.

The uniformity matters more than any single instance. A new list-shaped feature copies an existing
drawer and inherits the bound; there is no fast path and slow path to choose between, and no
surface where "render it all and let the widget clip" is the easy default. ratatui's own widgets
would happily accept a ten-thousand-line `Paragraph` and clip it to the pane, but every line of it
would still be styled and measured first; Quinjet never hands ratatui more than a paneful.

### The scrollbar as a pure function of the window

Because every windowed surface knows its `(offset, length, height)` triple, the scrollbar is one
shared function over those three numbers, `draw_scrollbar` at src/ui/mod.rs:4911:

```rust
fn draw_scrollbar(frame: &mut Frame<'_>, area: Rect, offset: usize, length: usize, theme: &Theme) {
    if length <= area.height as usize || area.width == 0 {
        return;
    }
    let height = area.height as usize;
    let thumb_height = (height * height / length).max(1).min(height);
    let max_offset = length.saturating_sub(height).max(1);
    let thumb_start = offset.min(max_offset) * (height - thumb_height) / max_offset;
    for row in thumb_start..thumb_start + thumb_height {
        frame.render_widget(
            Paragraph::new("▐").style(Style::default().fg(theme.accent_soft)),
            Rect::new(area.right().saturating_sub(1), area.y + cells(row), 1, 1),
        );
    }
}
```

The thumb height is the pane's share of the whole (`height * height / length`), floored at one row
so it never vanishes, and the thumb position maps the offset range onto the track linearly. Worked
example: a 50,000-row check log in a 40-row pane gives `thumb_height = (40 * 40 / 50_000).max(1) =
1`, `max_offset = 49_960`, and at offset 25,000 the thumb starts at row
`25_000 * 39 / 49_960 = 19`, near the middle of the track, as expected. The function is a no-op
whenever the content fits, so short lists pay nothing.

### Proof by test

The virtualization is pinned by tests that construct large data and assert small work:

- `pull_request_file_tree_virtualizes_a_thousand_files` (src/ui/mod.rs:7410) builds 1,000 files,
  puts the cursor at index 999, draws into a 48x12 terminal, and asserts the window scrolled
  (`app.sidebar_offset > 0`) and the last file is visible. The draw succeeds in a 12-row terminal
  precisely because only the visible window was rendered.
- `a_large_check_log_scrolls_from_a_cached_layout` (src/ui/mod.rs:7946) builds a 50,000-line step
  log, draws twice, and asserts the cached row `Vec` pointer is unchanged between draws while the
  final line ("output line 49999") is still reachable by scrolling. The test pins both properties
  at once: the layout was not rebuilt, and the window can reach every row.
- `a_long_conversation_stays_bounded_to_render` (src/ui/mod.rs:7890) builds a 500-entry
  conversation and asserts the composed row list stays under 3,000 rows and its pointer is stable
  across draws, tying the render bound to the fetch-time entry cap described in
  [the conversation page](../github/conversation-and-checks.md).

### Windowing a composed card: the offscreen buffer trick

One place uses a different windowing technique. The commit details card and the pull request
details card sit above the diff and participate in the same vertical scroll as the diff rows, so a
half-scrolled frame must show the bottom part of the card and then the top of the diff. Slicing a
multi-widget card layout at an arbitrary row would force every row renderer to understand partial
visibility, so `draw_commit_details_scrolled` (src/ui/mod.rs:3459) and
`draw_pull_request_details_scrolled` (src/ui/mod.rs:3624) instead render the whole card into an
offscreen `Buffer::empty(Rect::new(0, 0, area.width, cells(total_rows)))` and copy only the visible
row range into the frame cell by cell.

This looks like a violation of the render-only-what-is-visible rule, and it is allowed precisely
because the card is bounded: `commit_details_row_count` is `7.min(height - 3)` rows and
`pull_request_details_row_count` is `12.min(height - 3)` rows (src/ui/mod.rs:3443-3449). Rendering
at most twelve rows offscreen to get exact partial-scroll behavior is a constant cost, and the
technique buys a real simplification: links inside the card are registered through
`scrolled_detail_link_area` (src/ui/mod.rs:4047), which maps the link's card-local coordinates
through the scroll and returns an empty rectangle when the link has scrolled off, so a
half-scrolled link is clipped or dropped without any widget knowing about scrolling. The general
lesson: viewport discipline is about bounding work, and a bounded whole-render plus copy is
sometimes the cheapest correct window.

## The diff row layout cache

### Why a row list exists at all

The diff content pane renders a `DiffDocument`, the parsed model built by `src/git/diff.rs` and
described in depth in [the diff pipeline page](../diff/pipeline.md). A document is a flat
`Vec<DiffLine>` where every line has a kind: `FileHeader`, `FileFooter`, `HunkHeader`, `Context`,
`Added`, `Removed`, or `Meta`. The pane cannot render document lines directly, for two reasons:

- Not every line produces a visual row. Raw `@@` hunk headers are never rendered (their
  coordinates are transport detail, pinned by `hides_raw_hunk_coordinates_in_both_diff_layouts` at
  src/ui/mod.rs:7459), and a collapsed file contributes only its header row while its whole body
  is skipped.
- The side-by-side layout is not a per-line mapping at all. A removed run followed by an added run
  becomes a set of paired rows, two document lines per visual row, with blanks on the shorter side.

So each layout derives a row list: `Vec<usize>` of line indices for the unified view, and
`Vec<SideBySideRow>` for the split view. Deriving either list is a full walk of the document. On
the benchmark pull request that drove this optimization stack (oven-sh/bun#30412, 2,188 files,
+1,009,257 additions and -4,024 deletions), the assembled all-files document reaches hundreds of
thousands of lines, and before PR #46 that walk ran on every single frame, for every keypress,
wheel tick, and poll-triggered redraw. PR #46 made the row list a cached artifact on `App` with an
explicit key, and the walk now runs only when the answer can actually change.

### The cache key and the rebuild site

The cache is three fields on `App` (src/app.rs:1103): `unified_diff_rows: Vec<usize>`,
`side_by_side_diff_rows: Vec<SideBySideRow>`, and the key `diff_rows_key: Option<(u64, bool)>`,
plus the counter `document_layout_generation: u64` that feeds it. The consumer is `draw_content`,
from src/ui/mod.rs:

```rust
let side_by_side = app.diff_layout == DiffLayout::SideBySide && inner.width >= 72;
let rows_key = (app.document_layout_generation, side_by_side);
if app.diff_rows_key != Some(rows_key) {
    if side_by_side {
        app.side_by_side_diff_rows = side_by_side_rows(&app.document, app);
        app.unified_diff_rows = Vec::new();
    } else {
        app.unified_diff_rows = unified_row_indices(&app.document, app);
        app.side_by_side_diff_rows = Vec::new();
    }
    app.diff_rows_key = Some(rows_key);
}
```

The key has exactly two components, and each earns its place:

**1. The layout generation stands in for "the document or its folds changed."** Comparing documents
by value every frame would itself be O(document), so the generation is bumped instead, by exactly
the code paths that change what rows exist. `set_document` and `invalidate_diff_rows` at
src/app.rs:6070:

```rust
pub(crate) fn set_document(&mut self, document: DiffDocument) {
    self.document = document;
    self.invalidate_diff_rows();
}

pub(crate) const fn invalidate_diff_rows(&mut self) {
    self.document_layout_generation = self.document_layout_generation.wrapping_add(1);
    self.diff_rows_key = None;
}
```

PR #46 rewrote every direct `self.document = ...` assignment in `src/app.rs` (roughly eighteen
call sites: empty documents, loading placeholders, local diff rebuilds, the PR all-files rebuild,
view switches, document restore paths) to go through `set_document`, and added explicit
`invalidate_diff_rows()` calls at the places where the row layout changes without the document
being replaced: `reset_preview_file_folds`, the collapse-all toggle, per-file fold toggles, and
`cache_current_pull_request_single_document` (which moves the document out with `std::mem::take`).
Fold state has to invalidate because both row builders consult `app.preview_file_collapsed(path)`;
a fold changes which lines produce rows even though no line changed.

**2. The boolean is the effective layout, not the user's preference.** `side_by_side` is true only
when the user selected the split layout and the pane is at least 72 columns wide. Both inputs can
flip independently: toggling the layout key flips the preference, and resizing the terminal or
dragging the sidebar can push the content pane across the 72-column threshold either way. Folding
the width test into the cached boolean means a narrow pane silently renders unified from the
unified row cache, and widening back across the threshold rebuilds once.

Notice what is absent from the key: the pane width itself. Row lists are index structures, not
rendered text; clipping each row to the current width happens at draw time inside the row
renderers, so resizing within a layout regime reuses the cached rows unchanged and costs only the
per-visible-row clipping that every frame pays anyway. Putting raw width into the key would rebuild
the whole list on every column of a drag resize, for no change in the answer.

Only one of the two vectors is ever populated; the other is assigned `Vec::new()` to release its
allocation, since toggling layouts is rare and holding both lists for a huge document would double
the memory for no read benefit.

The behavior is pinned by `diff_rows_are_cached_between_draws_and_rebuilt_on_document_change`
(src/ui/mod.rs:7260), which draws the same document twice and asserts pointer identity of
`unified_diff_rows` across the draws, then calls `set_document` and asserts the key changed and the
rows match the new document.

### unified_row_indices: skipping what will not draw

The unified builder, `unified_row_indices` at src/ui/mod.rs:4080, is a single forward walk with a
fast-forward for collapsed files:

```rust
fn unified_row_indices(document: &DiffDocument, app: &App) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut index = 0;
    while let Some(line) = document.lines.get(index) {
        if line.kind == DiffLineKind::HunkHeader {
            index += 1;
            continue;
        }
        rows.push(index);
        let collapsed = line.kind == DiffLineKind::FileHeader
            && file_header_path(line).is_some_and(|path| app.preview_file_collapsed(path));
        index += 1;
        if collapsed {
            while document
                .lines
                .get(index)
                .is_some_and(|line| line.kind != DiffLineKind::FileFooter)
            {
                index += 1;
            }
            if index < document.lines.len() {
                index += 1;
            }
        }
    }
    rows
}
```

A worked example. Take a ten-line document with one expanded file and one collapsed file:

```text
index  kind         content                row list effect
0      FileHeader   a.rs  · modified      push 0
1      HunkHeader   @@ -1,3 +1,3 @@       skipped
2      Context      fn main() {           push 2
3      Removed      old();                push 3
4      Added        new();                push 4
5      FileFooter                         push 5
6      FileHeader   b.rs  · modified      push 6, b.rs collapsed:
7      HunkHeader   @@ -1 +1 @@             fast-forward
8      Added        only();                 fast-forward
9      FileFooter                           fast-forward past footer
```

The result is `[0, 2, 3, 4, 5, 6]`: six visual rows for ten document lines. The collapsed file
contributes exactly one row, its header, and its body lines cost nothing at draw time because they
do not exist in the row list; scrolling can never land inside them. The degenerate all-collapsed
case is pinned by `collapse_all_keeps_only_selectable_file_headers` (src/ui/mod.rs:8720): two
collapsed files produce exactly `vec![0, 4]`, the two header indices.

The header path lookup that drives the collapse test is itself cheap: `file_header_path`
(src/ui/mod.rs:4248) reads the header's first span text up to the `"  · "` separator, no
allocation, no parsing.

### side_by_side_rows: positional pairing

The split builder, `side_by_side_rows` at src/ui/mod.rs:4446, produces values of the `SideBySideRow`
enum defined at src/app.rs:940:

```rust
pub(crate) enum SideBySideRow {
    FileHeader(usize),
    FileFooter,
    Full { index: usize, boxed: bool },
    Split(Option<usize>, Option<usize>),
}
```

Every payload is an index into `document.lines`. `FileHeader` and `FileFooter` frame each file;
`Full` is a Meta row rendered across both panes (`boxed` records whether it sits inside a file box
so the renderer can pick the right background); `Split(old, new)` is one visual row with an
optional line on each side. Headers of collapsed files fast-forward exactly like the unified
builder, and `HunkHeader` lines are skipped in this layout too.

The interesting arm is `Removed`. The builder measures the whole contiguous removed run, then the
immediately following added run, and pairs them by position, from src/ui/mod.rs:

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

Worked example: a replacement block of three removed lines followed by two added lines, at document
indices 10 through 14, becomes:

```text
pair_index  row
0           Split(Some(10), Some(13))   old line 1 beside new line 1
1           Split(Some(11), Some(14))   old line 2 beside new line 2
2           Split(Some(12), None)       old line 3 beside a blank filler
```

This is the visual convention diff readers expect: "old one" sits beside "new one", and the surplus
side of an unbalanced replacement is blank (rendered by `draw_diff_side` as a filler cell in the
alternate panel color). Lone added lines outside a replacement become `Split(None, Some(index))`,
and context lines become `Split(Some(index), Some(index))`, the same line number on both sides.
The pairing behavior is pinned by `side_by_side_pairs_replacements` (src/ui/mod.rs:7315).

Positional pairing is a deliberate simplification. Git's own diff has already chosen the hunk
shapes (see [the diff algorithms page](../diff/algorithms.md) for how), so within one replacement
block the i-th removed and i-th added line are overwhelmingly the same logical line edited, and a
positional pair is right without any similarity scoring. A smarter alignment (pairing by content
similarity across the runs) would cost O(run length squared) comparisons in the row builder for a
marginal improvement on the rare block where an insertion shifts the alignment; the same positional
rule is reused by intraline emphasis, so the two features always agree about which lines are
partners.

### Why 72 columns gates the split layout

The split renderer divides the content pane at a draggable percentage: `usable_width = width - 1`
for the divider column, `left_width = usable_width * app.diff_split_percent / 100`
(src/ui/mod.rs:4327). Each side then reserves 7 columns for its line number and marker gutter
(`{number:>4} ` plus a 2-cell marker, src/ui/mod.rs:4710), against 12 in the unified layout (two
4-wide numbers plus the marker). At 72 columns the two sides get roughly 35 cells each, about 28
cells of actual code after gutters, which is the floor at which split code is still readable. Below
that, showing two mutilated columns is strictly worse than one good one, so the pane silently
renders unified and the cached boolean flips back, with no modal, warning, or user decision. The
same threshold appears in the minimum terminal size: a 72-column terminal with a hidden sidebar is
exactly wide enough to qualify.

### How this cache came to exist

PR #46 ("perf: viewport-scoped diff rendering and cached PR layouts") created the cache, and the
shape of the change explains a Rust-specific design constraint. Before it, `SideBySideRow` was a
UI-local enum holding borrows into the document:

```rust
enum SideBySideRow<'a> {
    FileHeader(&'a DiffLine),
    FileFooter,
    Full { line: &'a DiffLine, boxed: bool },
    Split(Option<&'a DiffLine>, Option<&'a DiffLine>),
}
```

A borrow-holding row list cannot outlive the frame that borrowed the document, so it could not be
stored on `App` at all; recomputing per frame was not a lazy choice but the only thing the types
allowed. The fix was to make the rows index-based: `usize` payloads into `document.lines`, resolved
at draw time with `lines.get(index)`, with a `continue` on a miss so a stale row can never panic
even if a document swap and a draw interleave unexpectedly. Index-based rows are also four to
sixteen bytes each instead of holding references with lifetime baggage, so a hundred-thousand-row
list is a few megabytes of plain integers with no drop glue.

The alternative of caching fully rendered rows (styled ratatui `Line` values instead of indices)
was rejected implicitly by the same design: rendered rows bake in the theme, the pane width, the
horizontal scroll, and the selection state, any of which would either invalidate the cache
constantly or require the cache key to grow until misses dominated. Indices are the smallest
artifact that removes the O(document) walk, and everything style- and width-dependent stays in the
per-visible-row draw where it is O(viewport) anyway.

## Intraline emphasis stays inside the viewport

The most expensive per-row enrichment in the diff pane is intraline emphasis: the highlighted
changed region inside a paired removed/added line, the way an edited word lights up while the rest
of the line stays dim. The full algorithm, its budgets, and its relationship to syntax highlighting
live in [the intraline and highlighting page](../diff/intraline-and-highlighting.md); this section
covers only the viewport contract, because it is the clearest example of the render layer's
scaling rule.

Before PR #46, emphasis was computed by a function that walked the entire document and allocated a
`Vec<Option<Range<usize>>>` sized to every line, pairing every removed run with its following added
run whether or not any of those lines were on screen. PR #46 replaced it with
`visible_intraline_emphasis(lines, visible)` (src/ui/mod.rs:4581), which takes an iterator of
exactly the visible line indices and returns a `HashMap<usize, Range<usize>>` keyed by line index.
The unified drawer passes precisely its window:

```rust
let emphasis = visible_intraline_emphasis(
    &app.document.lines,
    rows.iter()
        .copied()
        .skip(diff_scroll)
        .take(area.height as usize),
);
```

Per visible index, the function skips non-changed kinds, then resolves the replacement block the
line belongs to. The block is described by `EmphasisBlock { removed_start, added_start, added_end }`
(src/ui/mod.rs:4524), discovered by scanning outward from the visible line over runs of one kind.
Two details make the viewport scoping correct rather than merely fast:

- **The block is found locally, so the partner can be offscreen.** A removed line at the bottom of
  the pane whose added partner is scrolled just below still gets its emphasis, because the block
  scan runs over the document lines around the visible index, not over the visible window. The
  test `visible_intraline_emphasis_matches_block_pairing` (src/ui/mod.rs:8645) passes a single
  visible index and asserts the emphasis still matches the full-block pairing.
- **The block is cached across the iteration.** Consecutive visible lines usually belong to the
  same block, so the function reuses the current `EmphasisBlock` while indices stay inside it and
  recomputes only when leaving. A pane full of one replacement block computes the block once.

Pairing inside a block is positional, `pair_count = min(removed_run_len, added_run_len)`, exactly
mirroring the side-by-side row pairing, and surplus unpaired lines get no emphasis. The paired
computation itself, `paired_intraline_emphasis` (src/ui/mod.rs:4628), refuses pairs where either
side exceeds `MAX_INTRALINE_SOURCE_BYTES = 32 * 1024` bytes (src/ui/mod.rs:38), so a pathological
minified line costs a length check and nothing else (pinned by
`skips_intraline_work_for_very_long_rows` at src/ui/mod.rs:8628). Within budget, `changed_ranges`
computes the longest common prefix and suffix by characters and returns the byte range between
them, O(line length) with no allocation beyond the two line texts.

The side-by-side view does not build the map at all: a `Split` row already holds both partners, so
the split renderer calls `paired_intraline_emphasis(old_line, new_line)` lazily per visible row
(src/ui/mod.rs:4417). Same budget, same pairing rule, zero precomputation.

The combined effect is that emphasis cost per frame is O(pane height) with a 32 KiB per-line
ceiling, regardless of document size, and scrolling through a million-line diff computes emphasis
for each screenful as it appears, never for the whole document.

## The overview pane: rows composed from state

### A pane with no document

The pull request Overview pane, which shows the conversation thread or a check run's log, never
uses a `DiffDocument`. Its content is composed directly from `App` state: PR metadata, the
flattened conversation entries, the check list, and the parsed check log. ARCHITECTURE.md invariant
10b names the design: "The pull-request pane composes rows from app state rather than from a diff
document and reuses the resulting layout until its data, pane width, or ten-second relative-time
generation changes, so large conversations and logs are not rebuilt on every frame."

The composed artifact is a `Vec<PullRequestContentRow>` (src/app.rs:925), where each row carries a
pre-styled ratatui `Line<'static>`, an optional `step: Option<usize>` anchoring it to a check step,
and a `wide: bool` flag declaring whether the row may exceed the pane width. The doc comment at
src/ui/mod.rs:2143 describes the type as "A pre-wrapped content row, optionally anchored to a check
step so a click or the step cursor can find it after scrolling." Alongside the rows, the pane
caches the maximum wide-row width (for the horizontal overflow indicator) and a list of
`PullRequestContentLink` records positioned by row index and display column.

### The four-part key

The cache check sits at the top of `draw_pull_request_overview`, from src/ui/mod.rs:

```rust
let width = inner.width as usize;
let rows_key = (
    showing_check,
    width,
    app.pull_request_content_generation,
    relative_time_generation(),
);
if app.pull_request_content_rows_key != Some(rows_key) {
    app.pull_request_content_rows = if showing_check {
        check_run_rows(app, width, theme)
    } else {
        conversation_rows(app, width, theme)
    };
    app.pull_request_content_width = app
        .pull_request_content_rows
        .iter()
        .filter(|row| row.wide)
        .map(|row| row.line.width())
        .max()
        .unwrap_or_default();
    app.pull_request_content_links =
        pull_request_content_links(app, showing_check, &app.pull_request_content_rows);
    app.pull_request_content_rows_key = Some(rows_key);
}
```

Each key component covers one axis of change:

- `showing_check` is whether a check run's detail is displayed instead of the conversation
  (`app.pull_request_check_cursor.is_some()`); switching between the two views swaps the entire
  row source.
- `width` is the inner pane width in cells. It belongs in this key, unlike in the diff rows key,
  because prose is wrapped to the pane width at build time; a width change genuinely changes the
  rows. The diff pane clips at draw time instead, so its key omits width.
- `pull_request_content_generation` is the single data counter, bumped by
  `invalidate_pull_request_content_rows` (src/app.rs, `wrapping_add(1)` plus clearing the key)
  whenever PR metadata, the conversation, the checks, or the check log actually change.
- `relative_time_generation()` advances every ten seconds. It keeps relative timestamps live
  without rebuilding the rows on every frame or issuing any repository or GitHub request.

On a miss the pane rebuilds all three artifacts together: the rows, the maximum wide width, and
the link positions, so they can never disagree with each other.

### Making frequent polls cheap: the changed guards

The interesting engineering in PR #46 was not the cache but the invalidation audit. The pane's
data streams refresh on an adaptive poll (5 seconds while a check runs, 20 seconds settled, 2
minutes from another view, with per-stream floors; see
[the API strategy page](../github/api-strategy.md)). Before #46 the cache key was a 7-tuple of
per-stream generations, and any poll reply invalidated the rows even when the reply carried
identical content, which meant the pane re-wrapped a 500-entry conversation every 5 seconds while
a check was running.

The fix collapsed the key to the single content generation and moved the judgment into the worker
event handlers: every arrival compares the new snapshot against the stored one and bumps the
generation only on a real difference. The audited paths, all in `src/app.rs`:

- A checks snapshot invalidates only when the error state, the from-cache flag, or the check list
  itself changed, or the list is empty (the pane renders an empty-state row that depends on it).
- A check log arrival invalidates when a step was auto-expanded, a step is still running, an error
  cleared, or the log differs from the stored one; a log error invalidates when it replaced a
  present log or changed text.
- A conversation arrival invalidates when an error cleared, the entries are empty, or the
  conversation differs; a conversation error only when its text changed.
- A PR metadata snapshot invalidates only when it differs from the previous snapshot.
- Loading-state transitions that change what the pane shows also invalidate: starting a checks,
  log, or conversation load when nothing is loaded yet, because the pane renders "loading" rows in
  those states.
- Theme changes invalidate, because cached rows embed resolved theme styles.

The result is that the steady-state poll costs a comparison, not a rebuild. The comparisons are
O(new snapshot), which the fetch caps already bound, and they run on the worker-event path, not on
the render path.

### What the rows contain, bounded

`conversation_rows` (src/ui/mod.rs:2507) composes the header block (title, state, source,
destination, changes, check summary, URL, all marked wide so long values scroll rather than clip),
the wrapped description, and then one block per conversation entry: a marker row with the actor,
action, and timestamp, up to 8 lines of quoted code context for review comments behind a
` │ ▏ ` gutter, and the wrapped body. Every count in that composition is bounded: the conversation
itself is capped at 500 entries at fetch time (with newest-first paging so the cap can only drop
the oldest activity, the subject of [the conversation page](../github/conversation-and-checks.md)),
the context excerpt at 8 lines, the description preview in the details card at exactly 3 lines.
The 500-entry worst case composes to under 3,000 rows, pinned by
`a_long_conversation_stays_bounded_to_render` (src/ui/mod.rs:7890).

`check_run_rows` (src/ui/mod.rs:2880) composes the check header, a rule naming the step count, one
row per step (disclosure glyph, status icon, name, right-aligned duration), and, for expanded
steps, one wide row per log line colored by severity. The check log is capped at fetch time at
8 MiB and 200,000 lines, and the 50,000-line render test cited earlier draws it from this cached
row list without rebuilding.

Rendering the cached rows is the standard window: clamp `content_scroll`, `skip`/`take` the
visible range, apply `theme.selected` to the row matching the step cursor, register a
`ContentStepHit` for every visible step row, and re-register the visible links through the
horizontal scroll mapping. When the widest wide row exceeds the pane, the pane title gains a
`·  ←/→ {scroll}/{overflow}` indicator so the reader knows there is more to the right and how far
they have panned.

## Horizontal outgrowth versus wrapped prose

### Two regimes, declared per row

A terminal pane has one width, and content that exceeds it can either wrap or scroll. Quinjet
refuses to pick one rule globally, because the two failure modes are asymmetric: wrapped code is
unreadable (indentation is destroyed, alignment across lines is lost, a diff's shape disappears),
while horizontally scrolled prose is unreadable in the other direction (a paragraph should never
require panning). So every composed row declares which regime it belongs to, and the two regimes
never mix within a row:

- **Wide rows** (code, log output, diff lines, single-line metadata values like URLs and branch
  labels) keep their full composed width and are windowed horizontally at draw time, in display
  columns.
- **Prose rows** (comment bodies, descriptions, headings, quotes) are wrapped to the pane width at
  row-build time and are never wider than the pane by construction.

In the overview pane the declaration is the `wide: bool` on `PullRequestContentRow`; in the diff
pane every line is code and therefore wide. The doc comment on `push_prose` (src/ui/mod.rs:3095)
states the rule from the builder's side: "Code carries a second marker because it is the one kind
of line that is not wrapped to the pane."

### Display columns, not bytes

Horizontal windowing must be measured in display columns because terminal cells are not bytes and
not characters. A CJK ideograph occupies two columns; a combining accent occupies zero; an emoji
is typically two. Slicing a string by byte offset can split a UTF-8 sequence, and slicing by char
count misaligns everything after the first wide character. Quinjet measures with the
[unicode-width](https://docs.rs/unicode-width) crate everywhere, and the windowing primitive is
`shift_line`, from src/ui/mod.rs:

```rust
/// Window a composed row horizontally, in display columns rather than bytes, so
/// wide code and log lines can be read past the edge of the pane.
fn shift_line(line: &Line<'static>, skip: usize, width: usize) -> Line<'static> {
    let mut spans = Vec::with_capacity(line.spans.len());
    let mut scanned = 0;
    let mut used = 0;
    for span in &line.spans {
        if used >= width {
            break;
        }
        let span_width = span.content.width();
        if scanned + span_width <= skip {
            scanned += span_width;
            continue;
        }
        let text = slice_width(&span.content, skip.saturating_sub(scanned), width - used);
        scanned += span_width;
        if text.is_empty() {
            continue;
        }
        used += text.width();
        spans.push(Span::styled(text, span.style));
    }
    Line::from(spans)
}
```

The function walks the styled spans of a composed row, drops `skip` columns from the left, and
emits at most `width` columns, preserving each surviving span's style. Whole spans left of the
window are skipped without slicing (the `scanned + span_width <= skip` early continue), so panning
deep into a long line does not pay for re-slicing its prefix. The character-level slicing lives in
`slice_width` (src/ui/mod.rs:6531), which advances char by char using `UnicodeWidthChar`, so a
double-width character that straddles the window edge is dropped whole rather than half-rendered.

The diff pane achieves the same windowing inside `highlight_spans` (src/ui/mod.rs:4751) with a
`skip`/`remaining` column budget threaded through the span walk; it is fused there with emphasis
range splitting so one pass over the spans handles horizontal scroll, right clipping, and the
emphasis background at once. All three diff row renderers (unified, full-width, split side)
consume the single shared `app.horizontal_scroll` offset, adjusted by `h`/`l`, arrow keys, and
horizontal wheel events, so panning is consistent across layouts.

### Wrapping prose at build time

Prose takes the opposite path: `wrap_prose` (src/ui/mod.rs:3151) wraps Markdown-ish comment bodies
to the pane width when the rows are built, which is exactly why the pane width is part of the
overview cache key. Its rules are deliberately small, a display formatter rather than a Markdown
engine:

- Fenced code blocks toggle code mode; the fence lines themselves are dropped, and code lines are
  emitted unwrapped as code-styled rows, which become wide rows with a distinct gutter
  (`{gutter} ▏ `) in `push_prose`.
- Consecutive blank lines collapse to one; trailing blanks are popped.
- `> ` becomes a quote style with a 2-space indent; leading `#` becomes a heading style; `- `,
  `* `, and `+ ` become bullets rendered as `• ` with a hanging indent.
- `*` emphasis markers are stripped rather than interpreted.
- Word wrap is greedy (`wrap_words`, src/ui/mod.rs:3214), and a single word longer than the pane
  is truncated with an ellipsis rather than overflowing; the minimum wrap width is clamped to 8
  columns so degenerate panes cannot loop.

The contract between the regimes is pinned by
`a_comment_shows_its_code_intact_and_only_that_code_scrolls` (src/ui/mod.rs:8176): fence markers
never render; the code lines and single-line metadata are the only scrollable rows; every non-wide
row fits the pane (`line.width() <= pane width`); `shift_line(prose, 0, 80)` is the identity; and
shifting a wide line by 60 columns reaches its tail while the unscrolled view is clipped.

### Reserved columns

Every windowed row renderer reserves a fixed number of columns for its gutter before the content
budget starts, and those constants are the ones the 72-column minimum was chosen against:

| Surface | Reserved columns | What they hold |
| --- | --- | --- |
| Unified diff row | 12 | two 4-wide line numbers, each with a trailing space, plus a 2-cell marker |
| Side-by-side row, per side | 7 | one 4-wide line number, a space, and a 2-cell marker |
| Full-width diff row | 2 | the marker alone |
| File header row | 10 plus the count widths | rule characters, disclosure, icon, and the count fields |
| Check step row | 8 plus the duration width | disclosure, status icon, and padding |
| Detail label column | 12 | the fixed `DETAIL_LABEL_WIDTH` for label/value lines |

The content width handed to `highlight_spans` or `shift_line` is the pane width minus the
reservation, so gutters never scroll: line numbers and markers stay put while code pans under
them, the same convention editors use for their gutters.

## Anchored headers, step reveal, and End clamping

### The sticky file header

When a diff is scrolled into the middle of a file, the pane's top row shows that file's header
instead of a bare code line, so the reader always knows which file they are inside. The lookup is
`sticky_file_header`, from src/ui/mod.rs:

```rust
fn sticky_file_header(document: &DiffDocument, line_index: usize) -> Option<&DiffLine> {
    let mut header = None;
    for line in document.lines.iter().take(line_index.saturating_add(1)) {
        match line.kind {
            DiffLineKind::FileHeader => header = Some(line),
            DiffLineKind::FileFooter => header = None,
            _ => {}
        }
    }
    header
}
```

The unified drawer calls it for the first visible row when that row is not itself a header, and if
a header is found the top pane row renders it via `draw_file_header` while the diff body takes the
remaining `height - 1` rows (src/ui/mod.rs:4126). The side-by-side drawer gets the same effect
cheaper: its row list already exists, so it scans `rows[..diff_scroll]` backward for the nearest
`FileHeader` row (src/ui/mod.rs:4354). Both register a `ContentFileHit` for the sticky row, so
clicking the anchored header selects or toggles the file exactly like clicking the header in
place. The replay in `sticky_file_header` is O(scroll offset), a bounded walk over enum tags, and
it resets to `None` at each `FileFooter` so scrolling into the gap between files correctly shows
no anchor.

The overview pane anchors differently but for the same reason. Its header block (who wrote the
comment, what state the PR is in) is composed of wide rows that participate in vertical scroll,
but invariant 10b names the goal: "Anchoring the header keeps authorship on screen while a
comment's code is read sideways." Horizontal panning moves only the wide rows' window; the
wrapped prose rows and the entry marker rows do not shift, so the identity of what is being read
survives sideways travel.

### One-shot step reveal

Selecting a check step must scroll the pane so the step is visible. The naive implementation,
calling `ensure_offset` on every frame for the selected step, has a serious usability bug: while a
step is selected, the pane would be pinned to it, and scrolling down to read that step's log
output would snap back to the step row on the next frame, making long output unreadable.

Quinjet makes the reveal an event, not a constraint. `app.pull_request_step_reveal` is a one-shot
flag set when the step selection moves, and the draw that consumes it clears it, from
src/ui/mod.rs:

```rust
if showing_check && app.pull_request_step_reveal {
    app.pull_request_step_reveal = false;
    if let Some(cursor_row) = rows
        .iter()
        .position(|row| row.step == Some(app.pull_request_step_cursor))
    {
        ensure_offset(
            &mut app.content_scroll,
            cursor_row,
            inner.height as usize,
            rows.len(),
        );
    }
}
```

The row search uses the `step` anchor carried by every composed step row, which is what lets the
reveal find the step even though wrapping and expansion have made row indices unpredictable. After
the one adjustment, the pane scrolls freely; the guard test
`an_expanded_step_can_be_scrolled_past_to_reach_the_steps_below_it` (src/ui/mod.rs:7998) scrolls
to offset 120 inside an expanded step's output, redraws, and asserts `content_scroll` is still 120.
The field's doc comment (src/app.rs:1087) records the rationale so the constraint-style
implementation cannot sneak back in.

### End means "the end of whatever is there"

Jumping to the bottom of a pane is a moving target: the content length changes as patches stream
in, logs grow, and folds toggle. Quinjet uses a saturation idiom instead of computing the target:
`End`, the jump-to-bottom control, and log follow all set `content_scroll = usize::MAX`, and the
next draw clamps it to the real maximum, since every drawer already computes
`app.content_scroll = app.content_scroll.min(max_scroll)`. Invariant 10b names the idiom: "`End`
asks for the end and lets the draw clamp to whatever that pane holds." The same clamp pass sets
`app.content_at_bottom = app.content_scroll >= max_scroll`, one boolean that feeds two consumers:
the check-log follow behavior (the view follows new output only while the reader is already at the
end) and the jump-to-bottom control.

That control, `draw_jump_to_bottom` (src/ui/mod.rs:3408), renders a ` ↓ Bottom ` label on the
content pane's bottom border, right-aligned, and registers it as an `ScmAction::JumpToBottom` hit.
It hides itself when the reader is already at the bottom, when a modal is open, or when the pane
is under 20 columns or 3 rows. The action handler jumps focus to the content pane and sets the
saturated scroll; on a huge diff or conversation it replaces paging through thousands of rows with
one click, and it appeared in PR #48 alongside the newest-first conversation paging precisely
because bounded newest-first threads made "the bottom" the most valuable place in the pane.

## The mouse hit map

### Hit testing in an immediate-mode UI

Mouse support in a retained-mode toolkit is free: the widget tree is a spatial structure, and hit
testing walks it. An immediate-mode UI has no tree to walk, so it needs another answer to "what is
under the cursor". The standard immediate-mode answer, and Quinjet's, is to make hit regions a
byproduct of rendering: every drawer that renders something clickable also records the rectangle
it just drew and the action it means, and the frame's collected records are the hit map until the
next frame replaces them.

This inverts the usual dependency. Instead of a layout engine that both draws and answers queries,
the draw pass is the single source of truth and the hit map is derived data with a one-frame
lifetime. Three properties fall out:

- **The hit map is always in sync with the pixels.** It was produced by the same code, in the same
  pass, from the same state. There is no invalidation problem because there is no retained state.
- **It is viewport-scoped by construction.** Only rows that were actually drawn this frame
  registered hits, so the hit map for a million-line diff contains a paneful of rectangles, not a
  million. Nothing offscreen can be clicked, which is also the correct semantics.
- **Hit testing is a linear scan of a small list.** With at most a few hundred visible interactive
  regions, a `Vec` scan beats any spatial index at this scale, and the records are plain
  `(Rect, action)` pairs with no lifetime ties into render objects.

### The UiGeometry record

The collected map lives on `App` as `UiGeometry` (src/app.rs:993):

```rust
pub(crate) struct UiGeometry {
    pub changes_tab: Rect,
    pub history_tab: Rect,
    pub pull_requests_tab: Rect,
    pub main: Rect,
    pub sidebar: Rect,
    pub sidebar_divider: Rect,
    pub content: Rect,
    pub diff_divider: Option<Rect>,
    pub sidebar_hits: Vec<SidebarHitArea>,
    pub scm_action_hits: Vec<ScmActionHit>,
    pub modal_action_hits: Vec<(Rect, ModalAction)>,
    pub content_file_hits: Vec<ContentFileHit>,
    pub content_step_hits: Vec<ContentStepHit>,
    pub link_hits: Vec<LinkHit>,
    pub help_hits: Vec<HelpHit>,
    pub project_hits: Vec<Rect>,
}
```

The fixed rectangles at the top are the frame's stable regions: the three tabs, the main area, the
sidebar and its drag divider, the content pane, and the side-by-side split divider when present
(the drag target for resizing the split). The vectors are the per-frame hits, typed by what
clicking them means rather than by where they live:

- `SidebarHitArea` wraps a `SidebarHit` enum covering every sidebar row kind: change sections and
  entries, commits, the PR section tabs, recent PRs, tree directories and files, check sections
  and checks, the repository chooser, and the PR number lookup field.
- `ScmActionHit` wraps `ScmAction`: stage/unstage/resolve per file and per section, the stash
  checkboxes, the primary buttons and their overflow menus, and `JumpToBottom`.
- `ContentFileHit { area, path }` is registered for each visible file header row in a diff,
  including the sticky header, so clicking a header selects or toggles that file.
- `ContentStepHit { area, step }` is registered for each visible check step row.
- `LinkHit { area, target }` carries an `OpenTarget::Browser(String)`; these double as the source
  for the OSC 8 embedding and the hover underline described earlier.
- `HelpHit` rows map help-modal shortcuts, `project_hits` are the regions that open the projects
  modal (the header path and the footer worktrees label), and `modal_action_hits` are pushed by
  whichever modal drawer ran this frame.

The event side then dispatches by scanning the appropriate vector for the first rectangle
containing the click. Because the map is rebuilt into `app.geometry` at the end of every `draw`,
a click between frames tests against the frame the user was actually looking at.

### Registration is guarded and scroll-aware

The registration helpers keep degenerate and scrolled-away regions out of the map. `Link::register`
(src/ui/mod.rs:2149) refuses zero-area rectangles. `clipped_link_area` clips a one-row link to its
container. `horizontally_scrolled_link_area` (src/ui/mod.rs:4026) maps a link recorded in content
column coordinates through the current horizontal scroll, so a link inside a panned wide row is
clickable exactly where its text currently renders. `scrolled_detail_link_area` does the same for
the vertically scrolled details card and returns an empty rectangle when the link has scrolled
off. The overview pane stores its links as data (`PullRequestContentLink`, row plus start column
plus width) in the cached artifact and re-registers the visible ones through these mappers each
frame, which is the split PR #46 introduced: the expensive part (finding the link positions in the
composed text) is cached with the rows, and the cheap part (mapping through scroll and pushing a
rectangle) is per-frame.

### The cost accounting

Rebuilding the hit map every frame sounds wasteful and is not. The work is proportional to what
was drawn (a few pushes per visible interactive row), the allocation is a handful of short `Vec`s
whose capacity is warm after the first frame, and in exchange the codebase contains no hit-region
invalidation logic anywhere. The alternative, a retained spatial index updated by events, would
need exactly the invalidation machinery this page keeps celebrating the absence of, and its
failure mode (a stale rectangle dispatching yesterday's action) is a correctness bug, not a
performance one. This is the same trade immediate mode makes for pixels, applied to input.

## The Files tree is built once

### From flat index to tree rows

The PR Files sidebar shows the changed files as a directory tree with collapsible directories.
The underlying data is flat: `app.pull_request_files` is the bounded changed-file index (up to
16,384 entries) described in [the PR workspace page](../github/pr-workspace.md). The tree the
sidebar renders is a flattened list of typed entries (src/app.rs:264):

```rust
pub(crate) enum PullRequestTreeEntry {
    Directory { path: PathBuf, label: String, depth: usize },
    File { index: usize, depth: usize },
}
```

A `File` entry carries an index into `pull_request_files` rather than a copy of the file record,
the same index-not-borrow pattern as `SideBySideRow`. Building the list,
`rebuild_pull_request_tree` from src/app.rs:

```rust
fn rebuild_pull_request_tree(&mut self) {
    let mut entries = Vec::with_capacity(self.pull_request_files.len().saturating_mul(2));
    let mut root = PullRequestTreeNode::default();
    for (index, file) in self.pull_request_files.iter().enumerate() {
        root.insert(&file.path, index);
    }
    root.append_entries(0, &self.collapsed_pull_request_directories, &mut entries);
    self.pull_request_tree = entries;
}
```

`PullRequestTreeNode` (src/app.rs:291) holds its child directories in a
`BTreeMap<OsString, Self>`, so directory order is sorted lexically for free, with each directory's
files after its subdirectories in the flattened output. The flatten step, `append_entries`, takes
the collapsed-directory set and simply does not recurse into collapsed subtrees, so a collapsed
directory's descendants are absent from the flat list entirely, not present-but-hidden. That
absence is load-bearing for invariant 10: "hidden descendants never trigger diff work." A file
that does not exist in the row list cannot be scrolled to, cannot be selected, cannot register a
hit, and cannot become the prefetch anchor; collapsing a directory of a thousand vendored files
removes them from every downstream computation at once.

### Build-once discipline

The accessor enforces that the tree is built when needed and only then, from src/app.rs:

```rust
pub(crate) fn pull_request_tree_entries(&mut self) -> &[PullRequestTreeEntry] {
    if self.pull_request_tree.is_empty() && !self.pull_request_files.is_empty() {
        self.rebuild_pull_request_tree();
    }
    &self.pull_request_tree
}
```

Emptiness is the cache key. The vector is cleared when a new file index arrives, so the next
accessor call rebuilds; fold toggles (`toggle_pull_request_directory`, which flips membership in
`collapsed_pull_request_directories`) and the cursor-to-file sync call the rebuild directly
because they know they changed the answer. The draw pass, which calls the accessor every frame,
rebuilds nothing in the steady state; the UI test at src/ui/mod.rs:7389 has to clear the vector
manually after mutating collapse state to force a rebuild, which documents the discipline from the
test's point of view.

The cost profile follows: building the tree is O(files x path depth) with a `BTreeMap` insert per
directory component, paid once per index arrival or fold change. Drawing it is the standard
window: `draw_pull_request_file_tree` (src/ui/mod.rs:1747) clamps the cursor, calls
`sidebar_viewport`, and renders only the visible rows, with indentation capped at 16 columns
(`depth * 2`, and `"  ".repeat(depth.min(8))`) so a pathologically deep path cannot push its name
out of the pane. Directory rows render a disclosure glyph and register a
`SidebarHit::PullRequestDirectory(path)`; file rows render the selection bullet, the compile-time
file icon, the truncated name, and a right-aligned one-letter status code (A, M, D, R, C, T, U,
or ?) colored by status, registering `SidebarHit::PullRequestFile(index)`. The thousand-file
virtualization test cited earlier draws this exact surface.

### Skeletons before the tree exists

The Files sidebar also demonstrates the progressive states that precede data, covered in full in
[the progressive loading page](./progressive-loading.md). While the PR is still being prepared,
the sidebar renders up to 6 skeleton rows (`   ◌ ────` with staggered widths) driven by
`app.pull_request_loading`; once metadata exists but the local index does not, the Files section
shows "Preparing local diff index…"; the moment the index arrives the full tree renders with
status letters, even though no patch bodies exist yet, because tree structure needs only the
bounded name-status index. Headers whose counts GitHub could not report show a `+·· -··`
placeholder (two middle dots, a loading skeleton rather than an error) until the arrived patch
backfills the real numbers. Every state renders from in-memory data that a bounded read already
produced; no rendering state waits on an unbounded one.

## Wheel panning decoupled from selection

### The problem: a wheel that loads diffs

Until PR #54, a mouse wheel tick over the sidebar called the same `navigate` path as an arrow key:
it moved the selection cursor one row. That coupling has a cascading cost in an application where
selection means something. Moving the selection in the Changes or Files list changes the preview,
and changing the preview requests a diff; wheel-scrolling through a 2,000-file tree therefore
issued a preview request per two rows of travel, flooding the preview mailbox with work for files
the user was merely passing, and made it impossible to look at one file while browsing the list
for another. The wheel was not a scrolling device at all; it was a repeat-rate-limited selection
key.

The fix decouples the two ideas: the wheel pans the window, and only deliberate selection input
moves the cursor. What makes the implementation interesting is the reattachment rule, which keeps
both behaviors coherent without a mode the user has to manage.

### Detach on wheel, reattach on selection

Two fields on `App` carry the state (src/app.rs:1121): `sidebar_free_scroll: bool`, the detach
flag, and `sidebar_last_cursor: Option<usize>`, the cursor value the window last followed. The
wheel handler in `handle_mouse`, from src/app.rs:

```rust
MouseEventKind::ScrollDown => {
    if self
        .geometry
        .sidebar
        .contains((event.column, event.row).into())
    {
        self.focus = Focus::Sidebar;
        self.sidebar_free_scroll = true;
        self.sidebar_offset = self.sidebar_offset.saturating_add(2);
    } else {
        self.focus = Focus::Content;
        self.content_scroll = self.content_scroll.saturating_add(2);
    }
}
```

The routing decision reads `self.geometry.sidebar`, the hit map again: the wheel affects whichever
pane it is physically over, and when the sidebar is hidden its rectangle is empty so everything
routes to the content pane. A sidebar tick sets the detach flag and pans the offset by 2 rows, the
same step the content pane uses. Critically, the handler emits no effects: no `navigate`, no
preview request, no worker command. Panning is a pure in-memory mutation, which is what the test
`sidebar_wheel_scroll_pans_without_moving_the_selection` (src/app.rs:8880) asserts first: after a
wheel event, the effects list is empty and the cursor is unchanged.

The reconciliation happens in `App::sidebar_viewport`, which every sidebar drawer calls once per
frame in place of the plain `ensure_offset`, from src/app.rs:

```rust
/// The sidebar viewport for this frame. Wheel scrolling detaches the
/// window from the selection so the list can be browsed without changing
/// the preview; any selection movement reattaches it.
pub(crate) fn sidebar_viewport(&mut self, cursor: usize, height: usize, length: usize) {
    if self.sidebar_last_cursor != Some(cursor) {
        self.sidebar_last_cursor = Some(cursor);
        self.sidebar_free_scroll = false;
    }
    if height == 0 || length == 0 {
        self.sidebar_offset = 0;
        return;
    }
    if !self.sidebar_free_scroll {
        if cursor < self.sidebar_offset {
            self.sidebar_offset = cursor;
        } else if cursor >= self.sidebar_offset.saturating_add(height) {
            self.sidebar_offset = cursor.saturating_add(1).saturating_sub(height);
        }
    }
    self.sidebar_offset = self.sidebar_offset.min(length.saturating_sub(height));
}
```

Walking the design:

**1. Reattachment is edge-triggered on the cursor value.** The function compares the cursor
against `sidebar_last_cursor`; any change, whatever caused it (arrow key, click, a filter that
moved the selection), clears the detach flag. The user never issues a "reattach" gesture; touching
the selection is the gesture. Conversely, redrawing with an unmoved cursor keeps the detached
window exactly where the wheel left it, which the test pins ("an unmoved selection does not snap
the window back").

**2. While detached, the follow-cursor clamp is skipped entirely.** The selection can be scrolled
fully offscreen, which is the point: the reader is browsing, not selecting, and the preview pane
still shows the selected file, unchanged.

**3. The end clamp applies in both modes.** Wheel overscroll past the end of the list snaps to the
last full window rather than showing blank rows, and the same clamp protects the attached mode
against a shrinking list.

`reset_sidebar_scroll` (src/app.rs:6064) zeroes all three fields together and replaced nine bare
`sidebar_offset = 0` assignments across `src/app.rs` (view switches, section changes, PR resets,
history branch switches), so every context change also drops any leftover detach state; a stale
detach flag surviving into a different list would freeze that list's window inexplicably.

All four sidebar drawers (changes, history, PR file tree, PR check list) switched to
`sidebar_viewport` in PR #54, while `ensure_offset` survives for non-sidebar lists that have no
wheel interaction, such as the help modal. One function owns the policy, so the four lists cannot
drift apart in behavior.

### Why this belongs in a performance page

The feature reads as pure UX, but its mechanism is the render layer's economics applied to input.
The wheel used to convert kinetic scrolling into O(rows traveled) preview requests, each one a
generation-tagged worker command whose reply would be discarded when the next tick superseded it;
now it converts into O(1) integer additions and a redraw. And because `sidebar_offset` is the same
field the drawers window by, the detached pan integrates with everything downstream for free,
including the prefetch anchor in the next section, where the panned-to position actively steers
network and Git work toward what is on screen.

## Where the viewport steers background loading

### The prefetch problem on huge pull requests

Opening a large PR produces a bounded file index quickly, but patch bodies arrive through
background prefetch, batch by batch, as described in [the prefetch page](../github/prefetch.md)
and [the progressive loading page](./progressive-loading.md). The scheduling question is which
files to fetch first, and its answer went through two designs in this optimization stack. The
current behavior is viewport-anchored; the earlier size-tiered ordering is documented here as the
evolution step it was.

The batch machinery itself is stable across both designs. Each batch is one Git invocation of at
most `PULL_REQUEST_PREFETCH_BATCH = 32` files, filled until the estimated combined patch size
would exceed `PULL_REQUEST_PREFETCH_BYTE_BUDGET = 6 * 1024 * 1024` (6 MiB, deliberately under the
8 MiB capped pipe read so a batch essentially never truncates), with per-file estimates from
`estimated_patch_bytes` (src/app.rs:7052): `(additions + deletions) * 80 + 4096` bytes when
counts are known (`PULL_REQUEST_PATCH_LINE_ESTIMATE = 80` bytes per changed line plus a fixed
4,096-byte header overhead), and a `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE = 512 * 1024` (512 KiB)
assumption for a file with no counts. A single file whose estimate alone exceeds the budget still
travels, alone in its batch, so progress is guaranteed. Background fill stops after
`MAX_PREFETCHED_PULL_REQUEST_FILES = 4_096` files.

### The superseded design: smallest files first

PR #50 ("prefetch smallest files first on huge pull requests") attacked the ordering problem
globally. Before it, prefetch walked the index in file order, and on a huge PR the first files in
index order could be enormous: the whole 6 MiB budget went to a handful of giant patches while
thousands of tiny files stayed unloaded. #50 added a "huge" predicate, PR-level additions plus
deletions of at least 100,000 or at least 1,000 changed files, and when it tripped, sorted the
candidate list ascending by `estimated_patch_bytes` before filling batches. The stable sort kept
index order among equal sizes. The objective was throughput of completed files: filling batches
smallest-first maximizes how fast the count of files with a ready patch grows, so "most of the
tree opens instantly," as that PR put it.

Smallest-first optimizes a global metric that is not what the reader experiences. The reader is
looking at one place in the Files tree, and the file under their cursor is as likely as any to be
large, in which case it sorted to the back of the queue; the tree's checkmarks filled in fast
everywhere except, possibly, where the user was actually looking.

### The current design: start where the reader is looking

PR #55 deleted the size tiers (both threshold constants are gone) and replaced the ordering with a
viewport anchor and wrap-around walk, raising the total prefetch cap from 400 to 4,096 files at
the same time so background fill covers essentially the whole index of even huge PRs. The anchor,
from src/app.rs:

```rust
/// Where background fill should start: the first file visible in the
/// Files tree, so patches land where the reader is looking and then wrap
/// around the rest of the index in order.
fn prefetch_anchor_index(&self) -> usize {
    if self.view != View::PullRequests || self.pull_request_section != PullRequestSection::Files
    {
        return 0;
    }
    self.pull_request_tree
        .iter()
        .skip(self.sidebar_offset)
        .find_map(|entry| match entry {
            PullRequestTreeEntry::File { index, .. } => Some(*index),
            PullRequestTreeEntry::Directory { .. } => None,
        })
        .unwrap_or(0)
}
```

The anchor is derived from rendering state, and every piece of this page's machinery meets here:
`self.sidebar_offset` is the tree's scroll offset, so "the first file visible" means visible in
the drawn window, skipping directory rows; the tree being consulted is the cached flattened list,
so collapsed directories' files can never anchor; and because #54 lets the wheel move
`sidebar_offset` without moving the selection, panning the tree retargets where background fill
lands, with zero requests issued by the panning itself. Outside the Files section the anchor is
zero and the walk degenerates to plain index order.

The batch fill then rotates the index around the anchor instead of sorting, from
`request_pull_request_prefetch` in src/app.rs:

```rust
let anchor = self
    .prefetch_anchor_index()
    .min(self.pull_request_files.len());
let (before, from_anchor) = self.pull_request_files.split_at(anchor);
let mut batch_bytes = 0_usize;
let mut paths: Vec<PathBuf> = Vec::new();
for file in from_anchor.iter().chain(before.iter()) {
```

The chained iterator walks from the anchor to the end, then wraps to the beginning, so every file
is still reached and the cap and budget logic are untouched; only the order changed. The rewritten
test `prefetch_starts_at_the_files_viewport_and_wraps_around` (src/app.rs:8972) pins the rotation:
four files a.rs through d.rs with the tree scrolled so c.rs is the first visible file
(`sidebar_offset = 2`) produce a batch ordered `c.rs, d.rs, a.rs, b.rs`.

The design judgment worth recording: smallest-first and viewport-first optimize different
latencies. Smallest-first minimizes average time-to-ready across all files; viewport-first
minimizes time-to-ready for the files whose readiness the user can currently perceive, and
accepts that a giant file at the top of the viewport delays its batch. The batch byte budget
softens that cost (a giant anchor file travels alone while the next batch carries its neighbors),
and the wrap-around guarantees global coverage regardless. Since the perceived latency is the one
that makes a TUI feel instant, viewport-first won, and the sort disappeared rather than being kept
behind the huge-PR gate; ordering by smallest-first exists only in the history between the two
commits.

ARCHITECTURE.md invariant 5 records the current contract: "Background prefetch walks the whole
index up to 4,096 files, starting at the file the Files tree is showing and wrapping around the
rest in order, sizes each batch by per-file count estimates to stay under the 8 MiB patch read,
and backfills a header's counts from its arrived patch when GitHub could not report them."

### The benchmark that shaped the stack

Every ordering decision above was made against one stress case: the Bun rewrite pull request,
oven-sh/bun#30412 ("Rewrite Bun in Rust"), with 2,188 changed files, +1,009,257 additions and
-4,024 deletions, tested from a shallow `blob:none` clone. The measured evidence posted on PR #47,
which introduced API merge-base resolution and byte-sized batches for that workload, quotes
`time quinjet pr files 30412` at `real 0m6.30s` on a cold cache and `real 0m0.04s` warm, and a
single-file `quinjet pr diff 30412 .buildkite/ci.mjs` at `real 0m0.10s`. Those numbers measure
the CLI verbs over the same index, cache, and batching machinery the TUI renders from; the full
reproduction setup and every measured figure live in [the benchmarking page](../benchmarking.md).
The counts that feed `estimated_patch_bytes` come from the GitHub pulls files endpoint rather
than a blob-materializing local numstat (PR #49), which is what makes size estimates available
before any blob has been downloaded; that story belongs to
[the API strategy page](../github/api-strategy.md).

## Compile-time icon catalogs

### The invariant and the temptation it forbids

Every file row in every list renders a file-type icon: a Nerd Font glyph colored by language.
ARCHITECTURE.md invariant 1b pins how that must work: "File icons are static glyphs resolved by
allocation-free, compile-time sorted hash catalogs in the render layer. Rendering never reads
SVGs, font files, configuration, or the filesystem, and unknown paths use one generic glyph."

The temptation the invariant forbids is the way icon themes usually work: a configuration file
mapping names and extensions to glyphs, parsed at startup into a heap map, consulted per row. That
design puts a filesystem read into startup, allocations into the render path (typically a
lowercased copy of every file name, since icon matching is case-insensitive), and a runtime
failure mode (missing or malformed icon config) into a layer that should not be able to fail.
Quinjet's catalog is instead a compile-time artifact in `src/file_icons.rs`: the mapping is Rust
source, the hash table is computed by const evaluation, and lookup at render time is a hash plus a
binary search over a static array, with zero allocation and zero I/O.

### The data: one struct, semantic colors

An icon is two words of static data (src/file_icons.rs):

```rust
pub(crate) struct FileIcon {
    pub glyph: &'static str,
    pub color: SyntaxColor,
}

const RUST: FileIcon = FileIcon {
    glyph: "\u{e7a8}",
    color: SyntaxColor::Orange,
};
```

The color is a `SyntaxColor`, the same theme-independent semantic enum the syntax highlighter
stores (see [the intraline and highlighting page](../diff/intraline-and-highlighting.md)), not an
RGB value. The actual on-screen color resolves through `Theme::syntax` at draw time, so icons
recolor with the theme without any catalog change, and the catalog stays a pure data table. A test
(`every_icon_occupies_one_terminal_cell`) asserts each glyph is one display column wide, because a
two-column glyph would shear every list layout that reserves one cell for it.

### Hashing at compile time

The catalog macro turns a readable name-to-icon listing into a sorted static array of 64-bit
hashes, from src/file_icons.rs:

```rust
macro_rules! hashed_icon {
    ($value:expr, $default:expr, $($needle:literal => $icon:expr),+ $(,)?) => {{
        const CATALOG: &[IconMapping] = &sort_catalog([
            $(IconMapping {
                hash: ascii_hash($needle.as_bytes()),
                icon: &$icon,
            },)+
        ]);
        let value = $value;
        lookup_icon(value, CATALOG).unwrap_or($default)
    }};
}
```

Both helper functions are `const fn`, so the whole table exists in the binary's read-only data
with no startup cost. The hash is FNV-1a over case-folded bytes, from src/file_icons.rs:

```rust
const fn ascii_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index].to_ascii_lowercase() as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}
```

Those constants are the standard 64-bit FNV offset basis and prime; the one modification is the
`to_ascii_lowercase` fold, which buys case-insensitive matching (`Makefile`, `makefile`, and
`MAKEFILE` hash identically) without ever allocating a lowercased string, the allocation that a
`name.to_lowercase()` lookup key would cost on every row of every frame. Sorting happens in
`sort_catalog`, a const-evaluated insertion sort over the hash values; insertion sort is quadratic
but runs at compile time over catalogs of at most a few hundred entries, and it is one of the few
sorts expressible in a `const fn` without allocation.

### Lookup at render time

`lookup_icon` (src/file_icons.rs:750) hashes the query the same way and binary-searches the sorted
catalog: a lower-bound loop, then a final `filter(|mapping| mapping.hash == hash)` so a miss
returns `None` and falls back to the caller's default. The entry point composes two catalogs,
from src/file_icons.rs:

```rust
pub(crate) fn for_path(path: &Path) -> FileIcon {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return FILE;
    };
    let special = special_name_icon(name);
    if special != FILE {
        return special;
    }
    path.extension()
        .and_then(|extension| extension.to_str())
        .map_or(FILE, extension_icon)
}
```

Special whole-file names win over extensions (a `Dockerfile` is a Dockerfile, and
`package.json` gets the npm icon rather than the generic JSON one, pinned by
`recognizes_exact_ecosystem_files_before_their_extensions`), then the extension catalog runs, and
anything unknown gets the one generic glyph. One family of names is matched structurally instead
of by hash: `environment_name` recognizes `.env` and any `.env.*` variant with a four-byte prefix
comparison, because hashing every possible `.env.production.local` spelling into the catalog is
not possible.

The total render cost per icon is: one hash of a short name (a few dozen wrapping multiplies),
one binary search over a static array (six to nine probes), no allocation, no locks, no I/O. The
lookup compares only 64-bit hashes, never the strings; with FNV-64 over a fixed, compile-time
catalog of short names, a colliding pair inside the catalog would be a deterministic build-time
fact caught by the mapping tests, and a runtime name colliding with a catalog entry would cost a
cosmetically wrong icon, an accepted trade for keeping the comparison branch-free. The design
alternatives, an external perfect-hash crate, a lazily initialized `HashMap`, or a giant `match`
on string literals, are weighed in [the alternatives section](#design-alternatives-that-lost).

## Every rendering bound in one place

Bounded work is only auditable if the bounds are enumerable. This table collects every constant
the render layer relies on, each named where it appears earlier on this page or in the notes'
source references; the loading-side caps that feed rendering (fetch caps, poll floors, cache
sizes) live with their own pages.

| Bound | Value | Where |
| --- | --- | --- |
| Minimum terminal size | 72 x 18 cells | `draw`, src/ui/mod.rs:410 |
| Too-small notice card | 50 x 8 cells | `draw_too_small` |
| Tabs row, footer, main minimum | 3, 2, 8 rows | `draw` vertical layout |
| Tab widths | 13, 13, 17 cells | `draw_tabs` |
| Sidebar width clamp | 22 to main minus 32 cells | `draw`, src/ui/mod.rs:421 |
| Content pane minimum | 31 cells | `draw` horizontal layout |
| Wheel pan step | 2 rows per tick | `handle_mouse`, src/app.rs |
| Side-by-side minimum inner width | 72 cells | `draw_content`, src/ui/mod.rs:3312 |
| Unified row reserved columns | 12 | `draw_unified_line` |
| Split side reserved columns | 7 | `draw_diff_side` |
| Full-width row reserved columns | 2 | `draw_full_width_diff_line` |
| File header reserved columns | 10 plus count widths | `draw_file_header` |
| Detail label column | 12 cells | `DETAIL_LABEL_WIDTH`, src/ui/mod.rs:37 |
| Intraline emphasis source cap | 32 KiB per line pair | `MAX_INTRALINE_SOURCE_BYTES`, src/ui/mod.rs:38 |
| Commit details card | 7 rows, capped to height minus 3 | `commit_details_row_count` |
| PR details card | 12 rows, capped to height minus 3 | `pull_request_details_row_count` |
| Description preview | exactly 3 lines | `description_preview_lines` |
| Review-comment context excerpt | first 8 lines | `push_conversation_entry` |
| PR sidebar loading skeleton | at most 6 rows | `draw_pull_requests_sidebar` |
| Tree indentation | depth x 2, capped at 16 columns | `draw_pull_request_file_tree` |
| Minimum prose wrap width | 8 columns | `wrap_prose` |
| Footer progress bar | 12 cells | `draw_footer` |
| Help modal | 72 x at most 34 cells | `draw_help` |
| Command palette width | at most 76 cells, anchored near the top | `draw_command_palette` |
| Choice picker width | 44 cells | `draw_choice_picker` |
| Toast height | at most 7 rows | `draw_toast` |
| Overflow menu height | items plus 2, opening upward | `overflow_menu_area` |
| Conversation render rows | under 3,000 for a 500-entry thread | test bound, src/ui/mod.rs:7921 |

The loading-side constants that appeared in the prefetch discussion, for completeness: 32 files
per batch, a 6 MiB estimated byte budget per batch, 80 bytes per changed line plus 4,096 per file
as the estimate, 512 KiB assumed for a countless file, and a 4,096-file total prefetch cap, all in
`src/app.rs`.

The cache inventory, with each cache's key and invalidator:

| Cache | Key | Rebuilt in | Invalidated by |
| --- | --- | --- | --- |
| Diff row layout | `(document_layout_generation, side_by_side)` | `draw_content` | `set_document` or `invalidate_diff_rows` on document replace or fold change; the layout flag flipping |
| PR overview rows, width, links | `(showing_check, width, pull_request_content_generation)` | `draw_pull_request_overview` | `invalidate_pull_request_content_rows` on real data change; pane width change; conversation/check switch |
| PR Files tree | emptiness of the cached `Vec` | `pull_request_tree_entries` | index replacement clears the `Vec`; fold toggles rebuild directly |
| Frame char snapshot | none, every frame | `draw` | not applicable |
| Mouse hit map | none, every frame | `draw` | not applicable |

All generation counters are `u64` values bumped with `wrapping_add(1)`, and every key starts as
`None`, so the first draw after startup or after a reset always builds. The two per-frame entries
are deliberate: the snapshot and the hit map are cheap O(frame) products whose caching would cost
more in invalidation risk than their rebuild costs in time, the same judgment call, made in the
opposite direction, as the caches above it.

## One frame, end to end

The pieces are easiest to weigh together in one concrete walk. Take a 200x60 terminal, the Bun
benchmark PR open in the Files section, the unified layout, a file selected mid-tree, wheel
recently used so the sidebar is in free scroll, and no modal. The event loop calls `draw` once.
What actually happens, pass by pass, with the work each pass is allowed to do:

**1. Background fill and layout.** The 12,000-cell background fill is one widget render; the size
gate passes; the vertical and horizontal splits produce a 3-row tab bar, a 2-row footer, a 55-row
main region, the clamped sidebar (say its default 42 columns), a 1-column divider, and a 157-column
content pane. Cost: O(cells) writes into the back buffer, all arithmetic in whole cells.

**2. Tabs.** Three fixed-width tabs (13, 13, 17 cells), the repository name registered as a link
hit, the branch label with ahead/behind counts on the right. Constant work.

**3. Sidebar.** The Files tree accessor returns the cached flattened entries; nothing rebuilds
because neither the index nor a fold changed. `sidebar_viewport` sees an unmoved cursor and an
active free-scroll flag, so the offset stays where the wheel left it, clamped to the end. The
drawer windows the entries with `skip(sidebar_offset).take(53)` for the body rows, renders each
visible row (indent, disclosure or icon, truncated name, status letter), and registers one
`SidebarHitArea` per drawn row. Icon lookups are 53 hash-plus-binary-search probes into the static
catalogs. Cost: O(53), independent of the 2,188 files in the tree.

**4. Content pane.** The document is the assembled all-files document. The pane computes
`side_by_side = false` (unified selected), forms the rows key, and finds `diff_rows_key` already
equal: the cached `unified_diff_rows` is reused by pointer, zero walk. The details card
contributes its capped 12 rows to the scroll length; `content_scroll` is clamped against
`details_rows + diff_rows`; the sticky header replay scans line kinds up to the first visible row.
The drawer then windows the row list, `skip(diff_scroll).take(~40)`, computes
`visible_intraline_emphasis` over exactly those indices (block scans around the viewport, 32 KiB
per-pair cap), and renders each row through `highlight_spans` with the shared horizontal scroll
and the pane width minus 12 reserved columns. Every visible file header registers a
`ContentFileHit`. Cost: O(pane rows), with each row O(row width) in display columns.

**5. Jump-to-bottom and footer.** `content_at_bottom` came out of the clamp; if false and the pane
is big enough, the ` ↓ Bottom ` hit is appended. The footer renders the progress spinner and bar
if a prepare is still running, else branch and counts. Constant work.

**6. Geometry, snapshot, overlays.** The collected hit vectors move into `app.geometry`;
`snapshot_cells` copies 12,000 first-characters into `rendered_cells`; no selection is active; link
cells get their OSC 8 symbols and forced widths; no modal, no toast. Cost: O(cells).

**7. Flush.** ratatui diffs 12,000 cell pairs against the previous frame. If this frame was
triggered by a two-row wheel pan of the sidebar, the changed cells are roughly the sidebar's
42 x 53 region and the scrollbar, and only those bytes leave the process.

Now the same frame while a prefetch batch lands: the worker event replaced the document via
`set_document`, so `document_layout_generation` moved and step 4 rebuilds the row list once, a
single O(document lines) walk over enum tags, and every subsequent frame is back to pointer reuse.
The batch reply itself was sized by the 6 MiB budget and split at `diff --git` boundaries on the
worker thread (see [the diff pipeline page](../diff/pipeline.md)); by the time the render thread
sees it, it is already a parsed document. At no point in either frame did the render path touch a
socket, a pipe, a subprocess, or the filesystem, and nothing scaled with the million added lines
that the document ultimately represents.

## Failure modes and edge cases

Bounded rendering earns its keep at the edges. This section collects the corner cases the render
layer handles explicitly, grouped by the kind of hazard, each with the mechanism that defuses it.

### Degenerate geometry

- **A terminal below 72x18** gets the dedicated too-small frame rather than a best-effort layout;
  every downstream constant assumes the floor, so rendering under it would underflow width
  budgets. The gate runs before any layout math.
- **Zero-dimension inner rectangles** short-circuit: `draw_content` returns empty hit lists when
  the block's inner area has no width or height, and `draw_pull_request_overview` renders only the
  titled block. Widgets never receive an empty canvas to divide by.
- **Degenerate truncation widths** are defined, not defended against ad hoc: `truncate_end` maps
  width 0 to an empty string and width 1 to a bare ellipsis; `truncate_middle` keeps roughly two
  thirds of its budget on the left and the rest on the right around one ellipsis, so a truncated
  path keeps both its root and its file name.
- **Empty lists with live offsets** reset to zero inside `ensure_offset` and `sidebar_viewport`
  (the `height == 0 || length == 0` arm), so a list that emptied out from under its scroll state
  cannot render from a phantom offset.

### Unicode at the window edges

- **A double-width character straddling the left or right edge** of a horizontal window is dropped
  whole: `slice_width` advances by `UnicodeWidthChar` and never emits a partial glyph, so a CJK
  ideograph at a window boundary becomes empty space rather than a broken half-cell.
- **Emphasis ranges are byte ranges but computed on char boundaries**: `changed_ranges` extends
  its common prefix only while the byte indices of both sides agree and steps by whole characters,
  so the range endpoints always fall on UTF-8 boundaries and the span-splitting in
  `highlight_spans` can slice safely.
- **Escape-laden hyperlink cells** would confuse width accounting, since their symbol strings are
  dozens of bytes for one visual column; the `CellDiffOption::ForcedWidth` declaration pins them
  to one column in the buffer diff.

### Staleness and races

- **A row list can outlive its document by one frame** in the presence of interleaved worker
  events; every index-based row resolves through `lines.get(index)` and skips on a miss, so the
  worst case is a blank row for one frame, never a panic.
- **Generation counters wrap** by design (`wrapping_add(1)` on a `u64`); a collision would require
  the exact same counter value with the same layout flag after 2^64 bumps, and the keys start as
  `None`, so a fresh session or a reset always rebuilds regardless of the counter's value.
- **Prefetch replies are keyed to the workspace, not to a preview generation** (invariant 10a), so
  a background batch landing late can update the document but can never displace or invalidate the
  preview a reader explicitly requested; the render layer only ever sees the resulting
  `set_document`, which routes through the normal invalidation.
- **A poll reply with identical content does not touch the render caches** because the changed
  guards refuse to bump the content generation; the steady-state cost of the 5-second poll on the
  overview pane is a snapshot comparison on the event path and pointer-stable rows on the render
  path.

### Scroll-state pathologies

- **Wheel overscroll past the end** parks the offset arbitrarily far out; the final clamp in
  `sidebar_viewport` snaps it to the last full window on the next frame, in both attached and
  detached modes.
- **A detach flag surviving a context switch** would freeze the new list's window; every list
  reset path goes through `reset_sidebar_scroll`, which clears the offset, the flag, and the
  last-cursor memory together.
- **`content_scroll` saturated to `usize::MAX`** is the intended End idiom; every drawer clamps to
  its own maximum before use, so the sentinel never renders and never survives arithmetic.
- **A shrinking pane or document** (folds collapsing, a smaller document arriving) is handled by
  the same clamps: offsets are clamped to the current length every frame, so no state cleanup is
  needed at the mutation site.
- **A step reveal whose step vanished** (log replaced, fold changed) finds no matching row in the
  position scan and simply clears the one-shot flag; the pane stays where it was rather than
  jumping to a guess.

### Input mapping hazards

- **A hidden sidebar** stores `Rect::default()` as its geometry; the wheel handler's containment
  test then fails for every coordinate and wheel input routes to the content pane, with no special
  case for hiddenness.
- **A drag selection across the side-by-side divider** is clamped to the pane the selection
  started in, so copied text never interleaves the old and new sides of a diff.
- **A URL containing control characters** is never embedded into an OSC 8 sequence; the render
  layer treats PR-sourced URLs as untrusted bytes and skips the decoration rather than sanitizing.
- **Zero-area hit registration** is refused at the helper level (`Link::register` checks both
  dimensions), so fully truncated or clipped-away controls cannot leave invisible click targets.
- **Links inside scrolled regions** re-derive their rectangles through the scroll mappers each
  frame; a link scrolled halfway off is clipped to its visible part, and one scrolled fully off
  contributes no rectangle at all.

### The resize storm

Dragging the sidebar divider or resizing the terminal delivers a burst of frames with changing
widths, and the caches respond differently by design. The diff row cache ignores width entirely
except for the single 72-column threshold, so a drag inside one layout regime reuses rows every
frame, and crossing the threshold costs exactly one rebuild in each direction. The overview cache
keys on the exact width because its prose is wrapped at build time, so a drag that changes the
content pane width rebuilds the composed rows per distinct width; the rebuild is bounded by the
conversation and log caps (under 3,000 rows for the worst thread), which keeps the worst drag
frame at a bounded rebuild rather than an unbounded one. The bounded-versus-cached trade is
explicit: the diff pane clips per frame because its rows are width-independent indices, and the
overview pane rebuilds because wrapping is inherently width-dependent; each pane pays the cheaper
of the two costs for its own data shape.

## Design alternatives that lost

The current architecture is best understood against the designs it displaced or declined. Some of
these were real code that PRs removed; others are the standard alternatives any terminal UI picks
between. Each entry states the alternative, its genuine advantage, and the reason it lost here.

### ratatui's built-in widget scrolling

ratatui's `Paragraph` accepts a scroll offset, and the obvious way to render a long document is to
hand the whole thing to one `Paragraph` and set the offset. The advantage is simplicity: no row
lists, no windows, no offset math in application code. It lost because the widget must still
build, style, and wrap every line it was given before discarding the offscreen ones; the offset
moves the window, not the work. For a document measured in hundreds of thousands of lines, frame
time becomes O(document) inside the widget, invisible to the application but paid all the same.
Quinjet builds a paneful of single-row widgets from a windowed row list instead, so the widget
layer never sees the document at all.

### Delegating big documents to a pager

Many Git TUIs shell out to a pager (or embed a pager mode) for large diffs: hand the bytes to
something built for scrolling text and take the rendering problem off the table. The advantage is
robustness for the one surface. It lost because the diff pane is not a text stream: it carries
per-file fold state, click targets on every header, intraline emphasis, a live-updating document
that patches stream into, side-by-side pairing, and selection-linked previews. A pager either
drops that interactivity or grows into exactly the renderer this page describes. It would also
break the one-frame model, where every pane composes from the same `App` state under the same
input loop, and the never-spawn rule, since a pager is a subprocess.

### A retained widget tree

The retained alternative, a persistent tree of pane and row objects with dirty marking, is the
architecture of desktop toolkits, and its incrementality is real: an unchanged frame costs nothing
at all. It lost on correctness economics. Every optimization in this page is an explicit cache
with an explicit key, and the page's own history (the PR #46 invalidation audit) shows how much
care one such cache costs; a retained tree makes every widget's every property such a cache. The
immediate-mode discipline concentrates the invalidation problem into three well-named keys instead
of diffusing it across a tree, and accepts a bounded O(viewport) recompute per frame as the price.

### Application-level dirty-region tracking

Between the two extremes sits dirty-rectangle tracking: keep the immediate-mode draw, but skip
composing panes whose state has not changed. The advantage is skipping the O(cells) fixed costs on
idle frames. It lost because ratatui's buffer diff already reduces an unchanged pane's terminal
cost to zero bytes, the compose cost of an unchanged pane is already pointer-reuse of cached rows
plus cheap cell writes, and a skip layer would need its own change tracking on every input to
every pane, a second generation system shadowing the first. The measured-costly parts were made
cheap directly instead of adding machinery to avoid the cheap parts.

### Caching rendered lines instead of row indices

For the diff pane, the cache could hold fully styled ratatui `Line` values rather than `usize`
indices. The advantage is skipping per-frame span assembly for visible rows. It lost because
rendered lines bake in theme, width, horizontal scroll, selection, and emphasis, so the key would
grow until misses dominated, and the memory per row would be an order of magnitude larger than an
index. The chosen split caches the O(document) part (which rows exist) and recomputes the
O(viewport) part (how the visible ones look), which is the division that matches where the work
actually is. The overview pane makes the opposite choice, caching styled rows, because its data is
small and bounded and its wrapping is genuinely width-dependent; the two caches disagreeing is the
design working, not an inconsistency.

### Width in the diff rows key

A symmetric-looking design would put the pane width into the diff rows key just as the overview
key does. It lost for the reason spelled out in the cache section: row identity is
width-independent in the diff pane, so keying on width would rebuild an identical list on every
resize tick. Only the 72-column layout flip changes the answer, so only that boolean participates.

### Follow-the-selection as a standing constraint

Both the step reveal and the sidebar window could be standing constraints ("the selection is
always visible") instead of events. Standing constraints are simpler to state and impossible to
forget. They lost twice, in two forms: the step reveal became one-shot because a pinned pane makes
an expanded step's output unreadable, and the sidebar window gained the detach flag because a
glued window makes browsing impossible without selecting. In both cases the fix has the same
shape: the constraint runs once at the moment of intent (selection moved) and stays out of the way
otherwise. This is the interaction-design twin of cache invalidation: apply the rule on the edge,
not on the level.

### Wheel as a selection device

The pre-#54 behavior, wheel ticks calling `navigate`, was itself a deliberate first design: it
kept one code path for all list movement and guaranteed the preview always matched the hovered
region. It lost when large PRs made the coupled cost visible, a preview request per two rows of
travel and no way to browse without loading. The replacement keeps the single-path property where
it matters (all selection movement still flows through `navigate`) and reroutes only the wheel
into pure offset mutation.

### Smallest-first prefetch ordering

PR #50's size-tiered ordering was a real, merged design with a real win: past 100,000 changed
lines or 1,000 files, filling batches smallest-first maximized how fast the tree's loaded count
grew. It lost to PR #55's viewport anchor because the metric it optimized, total files ready, is
not the perceived one, files ready where the reader is looking. The wrap-around walk preserved the
global-coverage property that made smallest-first attractive, and the byte-budgeted batches
already prevented giant files from starving a batch, so the sort could be deleted outright rather
than gated. The full sequence is documented in
[the prefetch steering section](#where-the-viewport-steers-background-loading).

### An icon table with runtime machinery

Three standard implementations of the icon catalog lost to the const-evaluated one. A
`match name { "dockerfile" => DOCKER, ... }` over string literals is allocation-free but
case-sensitive unless every arm lowercases, and the compiler's string matching does not
case-fold; pre-lowercasing the query allocates per lookup. A lazily initialized
`HashMap<String, FileIcon>` costs startup work, heap residency, and a hash of an owned key, and
puts a lock or `OnceLock` on the render path. A perfect-hash crate generates exactly the static
table Quinjet wants, but as a build dependency and proc-macro cost for what a 30-line `const fn`
FNV hash, insertion sort, and binary search provide with no dependency at all. The chosen design
is the same trade as everywhere else on this page: a small amount of explicit, auditable code in
exchange for zero runtime machinery.

### A spatial index for hit testing

A retained hit structure (a quadtree, or even a persistent per-pane rectangle list updated on
change) would answer clicks without per-frame rebuilding. It lost because the per-frame rebuild is
O(visible interactive rows) pushes into warm vectors, the retained structure would need
invalidation on every scroll, fold, resize, and data change, and any staleness bug dispatches a
click to the wrong target, a correctness failure. The per-frame map is also what makes the hit map
agree with the frame the user saw by construction, click-during-update races included.

## Related pages

- [Rendering group hub](./README.md): the group's map and reading order.
- [Progressive loading](./progressive-loading.md): what renders at each stage while a huge PR
  file view fills in, and the end-to-end behavior on the benchmark PR.
- [Concurrency](./concurrency.md): the UI thread contract, generations, mailboxes, and how worker
  replies become the state this page renders.
- [Diff pipeline](../diff/pipeline.md): the document model, collapsed-headers-first indexing, and
  the batched patch production the row caches consume.
- [Intraline and highlighting](../diff/intraline-and-highlighting.md): the full intraline
  algorithm and the syntax highlighting budgets behind `HighlightSpan`.
- [Prefetch](../github/prefetch.md): the mailbox placement and batch mechanics that the viewport
  anchor steers.
- [API strategy](../github/api-strategy.md): the pulls files endpoint counts that size the
  batches, and the adaptive poll whose replies the changed guards absorb.
- [Conversation and checks](../github/conversation-and-checks.md): the newest-first paging and
  log attachment that produce the overview pane's data.
- [Benchmarking](../benchmarking.md): the Bun PR setup and every measured number for the stack.
- [Techniques catalog](../techniques.md): each pattern on this page, generalized and
  cross-referenced.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record time to first useful rows |
| 2 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record steady frame cost |
| 3 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record Git and gh process count |
| 5 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record stale reply rejection |
| 8 | Check latency for Viewport rendering and terminal frame economics in a small local repository | Record visible state after failure |
| 9 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record stale reply rejection |
| 16 | Check latency for Viewport rendering and terminal frame economics in a monorepo with many changed paths | Record visible state after failure |
| 17 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record time to first useful rows |
| 18 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record steady frame cost |
| 19 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record bytes accepted from child output |
| 20 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record Git and gh process count |
| 21 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record maximum retained document bytes |
| 22 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record cache disposition and complete key |
| 23 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record stale reply rejection |
| 24 | Check latency for Viewport rendering and terminal frame economics in a pull request containing generated files | Record visible state after failure |
| 25 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record time to first useful rows |
| 26 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record steady frame cost |
| 27 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record bytes accepted from child output |
| 28 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record Git and gh process count |
| 29 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record maximum retained document bytes |
| 30 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record cache disposition and complete key |
| 31 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record stale reply rejection |
| 32 | Check latency for Viewport rendering and terminal frame economics in a deeply diverged branch | Record visible state after failure |
| 33 | Check latency for Viewport rendering and terminal frame economics in an unavailable network | Record time to first useful rows |
| 34 | Check latency for Viewport rendering and terminal frame economics in an unavailable network | Record steady frame cost |
| 35 | Check latency for Viewport rendering and terminal frame economics in an unavailable network | Record bytes accepted from child output |
| 36 | Check latency for Viewport rendering and terminal frame economics in an unavailable network | Record Git and gh process count |
