# Progressive viewport-first loading

This page documents how Quinjet opens a pull request whose diff is far too large to load in one
piece, and keeps every intermediate state on screen useful. It covers PR #55 ("progressive
viewport-first loading for huge PR file views") in full depth: the skeleton states that render
before any data exists, the bounded index that seeds collapsed headers, the viewport-anchored
wrap-around prefetch that replaced #50's smallest-first ordering, the `DiffIndex` machinery that
makes a half-loaded document render coherently, the generation tags that keep stale replies out,
the poll stop for merged and closed pull requests, and the alternates borrow that makes
squash-merged pull requests fast against a full local clone. The benchmark target throughout is
[oven-sh/bun](https://github.com/oven-sh/bun) pull request #30412, "Rewrite Bun in Rust":
2,188 changed files and +1,009,257 added lines.

## Contents

- [The problem: a million-line pull request](#the-problem-a-million-line-pull-request)
- [Progressive loading as a discipline](#progressive-loading-as-a-discipline)
- [The loading sequence, stage by stage](#the-loading-sequence-stage-by-stage)
- [The bounded index that seeds the skeleton](#the-bounded-index-that-seeds-the-skeleton)
- [Paging the counts endpoint](#paging-the-counts-endpoint)
- [Rendering partial documents coherently](#rendering-partial-documents-coherently)
- [Count backfill from arrived patches](#count-backfill-from-arrived-patches)
- [Folding as a loading control](#folding-as-a-loading-control)
- [Viewport-anchored wrap-around prefetch](#viewport-anchored-wrap-around-prefetch)
- [Planning a batch by hand](#planning-a-batch-by-hand)
- [From smallest-first to viewport-first](#from-smallest-first-to-viewport-first)
- [Generation tags and stale replies](#generation-tags-and-stale-replies)
- [Memory bounds while patches stream](#memory-bounds-while-patches-stream)
- [Stopping the poll on settled pull requests](#stopping-the-poll-on-settled-pull-requests)
- [The alternates borrow for squash-merged pull requests](#the-alternates-borrow-for-squash-merged-pull-requests)
- [Measured results](#measured-results)
- [Failure modes and edge cases](#failure-modes-and-edge-cases)
- [One batch end to end](#one-batch-end-to-end)
- [The single-file path beside the stream](#the-single-file-path-beside-the-stream)
- [Launching straight into the stream](#launching-straight-into-the-stream)
- [Design alternatives that lost](#design-alternatives-that-lost)
- [Testing the progressive path](#testing-the-progressive-path)
- [Related pages](#related-pages)

## The problem: a million-line pull request

A terminal Git client that renders pull requests has an implicit scale assumption baked into
every one of its data paths: the changed-file list fits in one read, the combined patch fits in
memory, and the interval between "user pressed a key" and "everything is loaded" is short enough
that a single loading state is acceptable. bun#30412 broke every one of those assumptions at
once. The pull request rewrites an entire runtime: 2,188 changed files, over a million added
lines, patches for single generated files that alone exceed the 8 MiB per-read cap, and a fork
history deep enough that naive merge-base discovery fails outright.

The session that produced the optimization stack opened with exactly this complaint, quoted in
the working notes:

> For now the PRs and all look good but for very big PRs like the bun rewrite PR, it is taking
> so much time that it is even breaking right now. Can you work on optimizing the fetching
> mechanism that we have? Try to optimize it by breaking it down and optimizing it so that we
> can load in chunks for bigger PRs.

### What the pre-stack pipeline did wrong

Before the optimization stack (PRs #46 through #50, then #52, #54, #55), opening a huge pull
request stalled or froze for several distinct reasons, all catalogued during the baseline
analysis. The ones progressive loading addresses directly:

**1. The prefetch never got close to covering the pull request.** Background fill issued fixed
12-path batches and stopped after 400 files total. On bun#30412 that is barely 18 percent of the
index (400 of 2,188 files); on a hypothetical 20,000-file pull request it is 2 percent. Every
file past coverage was a blocking on-demand `git diff` routed through the coalescing preview
slot, which an in-flight batch could block for tens of seconds.

**2. Batch order ignored the reader entirely.** Batches walked the index in sorted path order
from the top. The files the user was actually looking at, halfway down the tree, loaded no
sooner than anything else, and the first files in path order on a huge pull request can be
enormous, so the budget burned on patches nobody had scrolled to.

**3. Counts required blobs.** In a blob-less disposable workspace, the `git diff --numstat` pass
used for per-file `+n -n` counts forced Git to lazily download essentially every changed blob in
one uninterruptible invocation while the UI sat at "Enumerating changed files". PR #49 replaced
that with counts from the GitHub pulls files endpoint; the details live in
[the API strategy page](../github/api-strategy.md). Progressive loading inherits the residue of
that decision: GitHub reports 0/0 for files whose counts it could not compute, so some headers
open with unknown counts and need a later fill-in.

**4. The all-files document was rebuilt from scratch after every batch.** Roughly 34 rebuilds to
reach the old 400-file stop, each re-cloning the whole file index and every cached document's
lines into one new combined `DiffDocument`, quadratic work on the UI thread.

**5. Everything between "open" and "loaded" was a single opaque loading state.** No file tree,
no counts, no partial diffs: just a message and a spinner, for as long as the fetch and the blob
storm took.

PRs #46 and #47 fixed the per-frame rendering costs and the batch sizing (see
[the viewport page](./viewport.md) and [the prefetch page](../github/prefetch.md)); #49 removed
the blob storm; #50 tried a size-tiered batch order; and #55, the subject of this page, is the
capstone that turned the whole file view into a progressively filling surface: render what
exists, label what does not, fill in from where the reader is looking, and never let a late
reply overwrite a newer state.

### The shape of the merged fix

PR #55 landed as commit `1261472` with two squashed checkpoint commits, quoted from the squash
body:

- "feat: viewport-first file fill, count backfill, and settled-PR poll stop"
- "perf: borrow local objects in the PR workspace and keep pure-rename counts"

The PR body summarizes the intent in one line: "Fill the Files view from the visible file
outward across the whole index, backfill counts from arrived patches with skeleton placeholders
until then, and stop polling merged or closed pull requests." The diffstat is small for its
effect: 128 insertions and 43 deletions across `ARCHITECTURE.md`, `src/app.rs`,
`src/git/diff.rs`, and `src/git/github/mod.rs`. Progressive loading is not a subsystem; it is a
set of small decisions distributed across scheduling, parsing, and rendering, each of which this
page examines in turn.

After #55, ARCHITECTURE.md invariant 5 reads, in the clause governing this page:

> Background prefetch walks the whole index up to 4,096 files, starting at the file the Files
> tree is showing and wrapping around the rest in order, sizes each batch by per-file count
> estimates to stay under the 8 MiB patch read, and backfills a header's counts from its arrived
> patch when GitHub could not report them.

## Progressive loading as a discipline

Progressive loading is an old idea from a different medium. Interlaced images render a coarse
full-frame pass first and refine it; browsers paint above-the-fold content before the rest of
the page has arrived; editors open a file's visible screen before indexing the rest. The common
principle: perceived latency is governed by the time to the first useful frame, not the time to
the last byte, so a loader should be judged by what the user can do at every intermediate
moment, not by its total duration.

Applying the principle to a diff viewer imposes four requirements, and each maps to a concrete
mechanism in Quinjet:

**1. Every intermediate state must be a valid render.** There is no moment where the view shows
half a data structure. The document model must be able to represent "this file's patch has not
arrived" as a first-class row, not as an absence that crashes layout. Quinjet does this with
`DiffIndex::document_with_visibility` in `src/git/diff.rs`, which assembles a complete, sorted,
navigable document out of whatever mixture of loaded patches and placeholder rows currently
exists. The section [Rendering partial documents coherently](#rendering-partial-documents-coherently)
walks through it line by line.

**2. Late data must merge monotonically.** New information may only replace placeholders or
refine estimates; it must never regress a header that already shows real data, and re-applying
the same reply twice must be harmless. Quinjet's count backfill only fills headers whose
`counts` field is still `None`, and the batch handler skips documents already cached, so arrival
order cannot corrupt the view.

**3. Stale data must be rejected, not merged.** A progressive loader has many replies in flight;
if the user switches pull requests mid-stream, replies for the old one continue to arrive. Every
reply must carry an identifier of the question it answers, checked at the single point where
replies mutate state. Quinjet uses two generation counters with deliberately different scopes,
covered in [Generation tags and stale replies](#generation-tags-and-stale-replies).

**4. Fill order should follow attention.** When the whole cannot arrive at once, the order in
which pieces arrive is a scheduling policy, and the best policy is the one that minimizes the
time until the piece the user is looking at is ready. Quinjet anchors its background fill to the
first file visible in the Files tree and wraps around the index from there, covered in
[Viewport-anchored wrap-around prefetch](#viewport-anchored-wrap-around-prefetch).

There is a fifth, quieter requirement: bounded memory. A stream that fills in forever must not
accumulate forever. Quinjet holds parsed patches under a 32 MiB budget with FIFO eviction,
covered in [Memory bounds while patches stream](#memory-bounds-while-patches-stream).

### Why the terminal makes this easier, not harder

A browser doing progressive rendering fights its layout engine: late content reflows the page
and moves what the user is reading. A terminal UI has none of that. Quinjet's render layer is an
immediate-mode pass over app state (see [the viewport page](./viewport.md)); every frame is
rebuilt from scratch from whatever the state currently says, and only the rows inside the
viewport are materialized. That means progressive loading needs no invalidation protocol between
the loader and the renderer: the loader mutates state, bumps a generation counter, and the next
frame simply draws the new truth. The row-layout cache keyed by
`(document_layout_generation, side_by_side)` in `src/app.rs` makes the unchanged-frame case
cheap, and `set_document` invalidates it whenever a batch arrival rebuilds the combined
document, so the two systems compose without special cases.

The flip side is that the terminal gives no partial-paint escape hatch: a frame either renders
in a few milliseconds or the whole interface stutters. Progressive loading in Quinjet is
therefore inseparable from the viewport-scoping work of #46; streaming patches into a document
that is re-scanned in full on every keystroke would merely move the freeze from open time to
scroll time. This page assumes the rendering economics of [the viewport page](./viewport.md)
and focuses on the loading side.

## The loading sequence, stage by stage

What follows is the exact on-screen sequence for a huge pull request, worked against bun#30412.
Each stage names the state that gates it, the code that draws it, and the event that advances to
the next stage. The stages overlap in practice because the streams run on different worker lanes
(see [the concurrency page](./concurrency.md)); the numbering is the order in which each surface
first becomes useful.

### Stage 0: skeleton rows before any metadata

The instant a lookup starts, `app.pull_request_loading` is set and the sidebar has nothing real
to show. Rather than a blank pane or a spinner, it draws a loading skeleton: up to six ghost
rows of staggered width, suggesting the list that is about to exist. From
`draw_pull_requests_sidebar` in `src/ui/mod.rs:1424-1437`:

```rust
    } else if app.pull_request_loading {
        let skeleton_count = body_area.height.min(6);
        for offset in 0..skeleton_count {
            let y = body_area.y + offset;
            let width = body_area.width.saturating_sub(8 + (offset % 3) * 5);
            frame.render_widget(
                Paragraph::new(format!(
                    "   ◌ {}",
                    "─".repeat(width.saturating_sub(6) as usize)
                ))
                .style(Style::default().fg(theme.border).bg(theme.panel)),
                Rect::new(body_area.x, y, body_area.width, 1),
            );
        }
    }
```

Three details are worth noting. The row count is capped at 6 regardless of pane height, because
a full pane of ghost rows reads as broken rather than loading. The widths cycle through three
lengths (`8 + (offset % 3) * 5` cells subtracted from the pane width), which is what makes the
rows read as a list of differently named items instead of a striped rectangle. And the rows are
drawn in `theme.border` color, the most muted tone in the palette, so the skeleton never
competes with real content elsewhere on screen.

While the workspace prepares, two other chrome surfaces show quantified progress. The sidebar
title carries a percent suffix (`"  · {percent}%"`, `src/ui/mod.rs:1323-1327`), and the footer
shows a spinner, the current stage label, a 12-cell bar, and the percent
(`src/ui/mod.rs:4949-4969`). The bar itself is two glyph runs
(`src/ui/mod.rs:6334-6341`):

```rust
fn progress_bar(percent: u16, width: usize) -> String {
    let filled = usize::from(percent.min(100)).saturating_mul(width) / 100;
    format!(
        "{}{}",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}
```

The percent comes from `PullRequestProgress` (`src/git/github/mod.rs:237-269`), whose variants
are emitted by the workspace preparation code as it advances. The stages and their labels:

| Variant | Percent | Label |
| --- | --- | --- |
| `LoadingMetadata` | 10 | Fetching pull-request metadata |
| `PreparingRepository` | 20 | Preparing an isolated diff workspace |
| `FetchingBase` | 35 | Fetching the destination commit |
| `FetchingHead` | 50 | Fetching the source commit |
| `FindingMergeBase` | 65 | Finding the merge base |
| `EnumeratingFiles` | 90 | Enumerating changed files |

These are milestones, not measurements: the bar communicates which phase of the fetch ladder the
workspace is in, which matters because the phases have very different costs (a network fetch of
the head versus a local merge-base computation). The fetch ladder itself is documented in
[the PR workspace page](../github/pr-workspace.md) and
[the shallow and partial clone page](../git-internals/shallow-and-partial-clone.md).

### Stage 1: the Files section before the index exists

If the user switches to the Files section before the changed-file index has arrived, the tree
pane distinguishes "still working" from "genuinely empty". From `draw_pull_request_file_tree` in
`src/ui/mod.rs:1753-1763`:

```rust
    if app.pull_request_files.is_empty() {
        let message = if app.document_loading || app.pull_request_progress.is_some() {
            "\n  Preparing local diff index…"
        } else {
            "\n  No changed files"
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(theme.muted)),
            area,
        );
        return Vec::new();
    }
```

The predicate is a disjunction on purpose: `document_loading` covers the window where a document
request is in flight, and `pull_request_progress.is_some()` covers the earlier window where the
workspace itself is still being prepared. Only when both are quiet does an empty list mean the
pull request truly changes nothing.

### Stage 2: the tree renders in full, with count placeholders

The changed-file index arrives as one bounded read (a `git diff --name-status -z` listing capped
at 8 MiB and 16,384 entries; see [the next section](#the-bounded-index-that-seeds-the-skeleton)).
The moment it lands in `app.pull_request_files`, the entire tree renders: every directory, every
file name, every status letter, fully navigable, foldable, and scrollable, even though not a
single patch body exists yet. This is the single most important perceptual moment of the load:
the pull request has gone from "loading" to "explorable".

Headers whose counts are known (from the pulls files endpoint, PR #49) render their real
`+n -n` immediately. Headers whose counts GitHub could not report render a skeleton placeholder
instead: `+··` and `-··`, two middle dots per side. The choice of glyph was itself a #55 change:
the placeholder used to be `+?` and `-?`, which read as an error. Two middle dots read as "not
yet", matching the ghost-row skeleton of stage 0. From `DiffFileIndexEntry::count_spans` in
`src/git/diff.rs:131-141`:

```rust
    fn count_spans(&self) -> (String, String) {
        self.counts.map_or_else(
            || ("+··".to_owned(), "-··".to_owned()),
            |counts| {
                (
                    format!("+{}", counts.additions),
                    format!("-{}", counts.deletions),
                )
            },
        )
    }
```

The pull-request details card in the content pane participates in the same staging: its
"Selected" row shows "Preparing files" until a file is actually selected
(`src/ui/mod.rs:3697-3708`), and the commit-details variant of the card (the same card family
used by local diff views) shows a live counter built from
`App::local_diff_load_progress` (`src/app.rs:1766-1779`), rendered as
`"  ·  {loaded}/{total} diffs loaded"` (`src/ui/mod.rs:3576-3578`):

```rust
    pub(crate) fn local_diff_load_progress(&self) -> Option<(usize, usize)> {
        self.local_diff_index.as_ref().map(|index| {
            let loaded = if index.files.len() == 1 && self.local_diff_single_loaded {
                1
            } else {
                index
                    .files
                    .iter()
                    .filter(|file| self.local_diff_documents.contains_key(&file.path))
                    .count()
            };
            (loaded, index.files.len())
        })
    }
```

The counter is derived, not stored: it counts how many index paths have a cached document, with
a special case for a single-file index whose one document lives in the main document slot rather
than the cache. Deriving it means it can never drift from the truth it summarizes, which is the
kind of small decision that keeps a many-stage loader debuggable.

### Stage 3: the first patch suppresses the loading label

Patches begin to arrive, first the selected or first file through the preview lane, then batches
through the prefetch lane. As soon as any file's diff exists in the combined document, the
content pane stops advertising that it is loading, even though most of the pull request is still
in flight. From `draw_content` in `src/ui/mod.rs:3272-3283`:

```rust
    let loading = app.pull_request_progress.map_or_else(
        || {
            if app.document_loading
                && !(app.view == View::PullRequests && app.document.file_count() > 0)
            {
                "  · loading".to_owned()
            } else {
                String::new()
            }
        },
        |progress| format!("  · {}%", progress.percent()),
    );
```

The guard is precise: the `loading` suffix is suppressed only in the pull-request view, and only
once `app.document.file_count() > 0`. `file_count` (`src/git/diff.rs:365-384`) counts the
`FileHeader` lines actually materialized in the document, so the suppression triggers exactly
when the reader has at least one real diff to look at. The reasoning is a perceptual one: a
loading mark next to content the user is already reading communicates "this might change under
you", which is wrong; the remaining fill-in only adds rows the user has not reached. The
skeleton placeholders inside the document carry the remaining "not yet" signal at the exact rows
where it is true, which is strictly more informative than a global flag.

### Stage 4: the rest streams in through prefetch

From here the sequence is a loop: the scheduler builds a batch of up to 32 unpatched files
starting from the first file visible in the tree, sized under a 6 MiB estimated byte budget; the
worker runs one `git diff` for the batch; the reply merges into the document cache; the combined
document rebuilds if anything visible changed; and the scheduler immediately requests the next
batch. On bun#30412 that is 69 batches at the 32-file ceiling (2,188 divided by 32, rounded up),
fewer in practice where byte budgets end batches early, each landing as soon as it parses. The
loop continues until every file has a patch or the 4,096-file prefetch cap is reached. Every
piece of that loop is examined in the sections that follow.

## The bounded index that seeds the skeleton

Everything in stages 2 through 4 renders out of one data structure: the changed-file index. Its
job is to be small, complete-enough, and available long before any patch. Progressive loading
works precisely because this index is cheap: a list of paths with statuses and (mostly known)
counts costs kilobytes per thousand files, while the patches it stands in for cost megabytes.

### The name-status listing

The index is produced in the prepared workspace by `changed_files_in_repository`
(`src/git/github/mod.rs:1981-2089`) running:

```bash
git diff --name-status -z --find-renames <merge_base> <head> --
```

The `-z` flag is what makes the listing parseable at any scale: records are NUL-separated, so
paths containing newlines, tabs, or quoting-sensitive bytes survive verbatim (see
[the plumbing page](../git-internals/plumbing-and-porcelain.md) for the full `-z` story). The
record stream alternates status and path records, with renames and copies carrying two paths:

```text
offset  bytes                          meaning
0       "A" NUL                        status: added
2       "src/new_file.rs" NUL          path of the added file
19      "M" NUL                        status: modified
21      "README.md" NUL                path of the modified file
31      "R100" NUL                     status: renamed, similarity 100
36      "src/old_name.rs" NUL          pre-image path
53      "src/new_name.rs" NUL          post-image path
```

The parser maps the first status byte to `PullRequestFileStatus` (A, M, D, R, C, T, U, anything
else Unknown), so a similarity-scored `R100` still parses as a rename. Two caps bound the read:
stdout is capped at `MAX_PR_PATH_BYTES` (8 MiB), with the Git child killed on overflow and the
buffer trimmed back to the last NUL so only whole records parse, and the entry count is capped
at `MAX_PR_PATHS` (16,384), past which the index is marked `truncated` and `total_files` falls
back to GitHub's own `changedFiles` figure. bun#30412's 2,188 files sit comfortably inside both
caps; the caps exist so that a pathological pull request degrades to a truncated index instead
of unbounded memory.

The listing is cached under the immutable key `pr-files-v1\n{merge_base}\n{head}`
(`src/git/github/mod.rs:1997`). Because both key components are commit OIDs, the entry can never
go stale, only get evicted; [the caching page](../github/caching.md) develops that argument, and
[the object model page](../git-internals/object-model.md) explains why OID-keyed content is
immutable by construction.

### Counts without blobs

Each `PullRequestFile` carries `counts: Option<DiffLineCounts>`, and the `Option` is
load-bearing: it is the difference between a header that renders `+12 -3` and one that renders
the `+·· -··` skeleton. On the disposable-workspace path the counts come from the GitHub REST
endpoint `pulls/{number}/files`, read as up to 64 pages of 100 records each
(`MAX_FILE_COUNT_PAGES` at `src/git/github/mod.rs:39`), each record reduced by a jq program to a
four-field TSV row:

```text
filename TAB additions TAB deletions TAB status
```

The parser, `parse_api_file_counts` (`src/git/github/mod.rs:1918-1943`), skips malformed rows
and applies one semantic filter, refined twice across the stack:

```rust
if additions == 0 && deletions == 0 && status != "renamed" {
    continue;
}
```

GitHub reports `additions: 0, deletions: 0` for files whose counts it could not compute, which
in practice means very large or generated files, exactly the files a million-line rewrite is
full of. Storing that 0/0 would render a false `+0 -0`; skipping it leaves `counts` as `None`,
which renders the honest skeleton and leaves the header eligible for
[count backfill](#count-backfill-from-arrived-patches) once its patch arrives. The `renamed`
exemption is #55's correction to #49's original skip rule: a pure rename genuinely has zero
changed lines, so its 0/0 is truth, not absence, and must render as `+0 -0` immediately rather
than as a skeleton that never resolves. The fix also bumped the cache key from
`pr-file-counts-v2` to `pr-file-counts-v3` so entries recorded under the over-broad rule could
never serve again. The unit test `api_file_counts_parse_and_skip_malformed_records`
(`src/git/github/mod.rs:3177-3203`) pins all three behaviors: malformed rows dropped, non-rename
0/0 rows dropped, pure renames kept with zero counts.

During the session this exact case surfaced on real data: some of bun#30412's generated
`h2_client` files came back from the API as 0/0, rendered the placeholder, and then switched to
real counts once their patches loaded, which is the intended lifecycle.

### From index to skeleton document

The app-side index type is `PullRequestFile` (`src/git/github/mod.rs:189-196`): `path`,
`old_path`, `status`, `counts`. When the all-files document is rebuilt, the app converts this
into the diff layer's `DiffIndex`, from `rebuild_pull_request_all_files_document` in
`src/app.rs:5703-5727`:

```rust
        let index = DiffIndex {
            title,
            files: self
                .pull_request_files
                .iter()
                .map(|file| crate::git::diff::DiffFileIndexEntry {
                    path: file.path.clone(),
                    old_path: file.old_path.clone(),
                    status: pull_request_file_status_label(file.status).to_owned(),
                    counts: file.counts,
                })
                .collect(),
            truncated: self.pull_request_files_truncated,
            commit_details: None,
        };
        let paths = index
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let visible = self.visible_preview_paths(&paths);
        let mut document = index
            .document_with_visibility(&self.pull_request_documents, |path| visible.contains(path));
        document.pull_request_details = Some(pull_request_details(pull_request));
        self.set_document(document);
```

Two things happen here that make the pull-request view and the local diff views share one
mechanism. First, the GitHub-specific status enum flattens into the same lowercase string labels
(`"added"`, `"modified"`, `"renamed"`) that the local index uses, so the assembler downstream is
provider-agnostic. Second, the assembly is a pure function of three inputs: the index, the map
of loaded documents, and a visibility predicate reflecting fold state. That purity is what makes
the progressive rebuild safe to run after any batch arrival: there is no incremental mutation
that could drift, just reassembly from current truth, with `set_document` bumping the layout
generation so the row cache rebuilds once (see [the viewport page](./viewport.md)).

The counts field also feeds a second consumer: the batch scheduler's per-file size estimate,
covered in [Viewport-anchored wrap-around prefetch](#viewport-anchored-wrap-around-prefetch).
One `Option<DiffLineCounts>` per file thus drives both what the header renders and how the
loader budgets its batches, which is why PR #49's decision to source it from API metadata
rather than local blob materialization mattered so much for cold-load time.

## Paging the counts endpoint

The counts read deserves its own close look, because it is the one piece of stage 2 that talks
to the network in pages, and its failure semantics are unusually strict. The full REST context
lives in [the API strategy page](../github/api-strategy.md); this section covers what the
progressive pipeline depends on.

### When it runs

`pull_request_file_counts_from_api` is called inside `prepare_pull_request_diff`
(`src/git/github/mod.rs:786-787`), after the merge-base hint but before the
`TemporaryBareRepository` is even created. The ordering is deliberate: the counts are pure
metadata about immutable commits, so nothing about the workspace can change the answer, and
running the read up front means the index assembly at the end of preparation finds the counts
already in hand instead of adding a network pause between "Enumerating changed files" and the
stage-2 tree render. On the fully local path (both PR commits present in the opened
repository) the API read is skipped entirely and counts come from a local numstat, because
blobs are already on disk and a local read beats a network read.

### The page loop

For bun#30412's 2,188 files at `per_page=100`, the read is 22 pages, each one `gh api -i`
invocation whose `-i` flag includes response headers so the `Link` header can be parsed for
`rel="next"`. Pages advance `1..=MAX_FILE_COUNT_PAGES` (64), giving a hard ceiling of 6,400
files' worth of counts. Each page body is a jq-reduced TSV stream, and the page reader
`api_page` (`src/git/github/mod.rs:1202-1233`) enforces record integrity at the byte level: if
the capped pipe truncated stdout, trailing bytes are popped until the buffer ends at a newline,
so a record can never be split across the cap:

```rust
    if output.stdout_truncated {
        while data.last().is_some_and(|byte| *byte != b'\n') {
            let _ = data.pop();
        }
    }
```

The helper's doc comment states the contract: "One bounded page of a listing endpoint: its body
trimmed to whole records, plus whether GitHub advertises another page after it." The helper
itself is a #49 refactor with history: it began as a private conversation-paging function and
was hoisted to the repository level so counts and conversations page through one audited code
path.

### Failure semantics: strict in, permissive out

The loop's error policy has two asymmetric halves. On the way in, any page read error or any
truncated page aborts the whole function with `None`: the caller then falls back to local
numstat, which is slower but exact, on the reasoning the notes record as "partial counts are
worse than none, the numstat fallback stays correct". A silently half-populated counts map
would render a tree where some placeholders mean "GitHub could not count" and others mean "the
read broke", indistinguishably.

On the way out, permissiveness returns in one narrow case: if the page cap (not an error)
stopped pagination early, the accumulated records are still parsed and used for the files they
cover, but the accumulation is not cached. The cache-write condition is
`complete && collected.len() <= MAX_PR_PATH_BYTES`: only a full, bounded answer is worth
remembering under the immutable `pr-file-counts-v3` key, because an immutable cache entry that
was secretly partial would serve its gaps forever. Incomplete data may inform one session; it
may not become permanent truth. This complete-before-caching rule is the same one the validated
ETag reads apply ("a partial page must never be validated as if whole") and is a house pattern
throughout [the caching page](../github/caching.md).

## Rendering partial documents coherently

The heart of progressive loading is the document assembler:
`DiffIndex::document_with_visibility` (`src/git/diff.rs:221-296`). It answers the question every
streaming viewer must answer: what exactly does the reader see when 300 of 2,188 patches have
arrived, 40 files are folded shut, and one patch was truncated? The answer must be a single,
stable, navigable document. Here is the function in full:

```rust
    pub(crate) fn document_with_visibility(
        &self,
        loaded: &HashMap<PathBuf, DiffDocument>,
        mut visible: impl FnMut(&Path) -> bool,
    ) -> DiffDocument {
        if self.files.is_empty() {
            let mut document = DiffDocument::empty(&self.title, "No file changes to display");
            document.commit_details.clone_from(&self.commit_details);
            return document;
        }

        let mut lines = Vec::with_capacity(self.files.len().saturating_mul(3));
        let mut truncated = self.truncated;
        for file in &self.files {
            let loaded_document = loaded.get(&file.path);
            let show_body = visible(&file.path);
            truncated |= loaded_document.is_some_and(|document| document.truncated);
            let loaded_header = loaded_document.and_then(|document| {
                document
                    .lines
                    .iter()
                    .find(|line| line.kind == DiffLineKind::FileHeader)
            });
            if show_body && let Some(document) = loaded_document.filter(|_| loaded_header.is_some())
            {
                let mut file_lines = document.lines.clone();
                if let Some(label) = file_lines
                    .iter_mut()
                    .find(|line| line.kind == DiffLineKind::FileHeader)
                    .and_then(|header| header.spans.first_mut())
                {
                    label.text = file.label();
                }
                lines.extend(file_lines);
                continue;
            }

            let mut header = index_file_header(file);
            if let Some(loaded_header) = loaded_header {
                for span_index in 1..=2 {
                    if let (Some(target), Some(source)) = (
                        header.spans.get_mut(span_index),
                        loaded_header.spans.get(span_index),
                    ) {
                        target.text.clone_from(&source.text);
                    }
                }
            }
            lines.push(header);
            if show_body {
                if let Some(document) = loaded_document {
                    lines.extend(document.lines.clone());
                } else {
                    lines.push(meta_line(DiffLineKind::Meta, "Loading diff…"));
                }
            } else {
                lines.push(meta_line(
                    DiffLineKind::Meta,
                    if loaded_document.is_some() {
                        "Diff loaded · expand this file to display it"
                    } else {
                        "Expand this file to load its diff"
                    },
                ));
            }
            lines.push(meta_line(DiffLineKind::FileFooter, ""));
        }

        DiffDocument {
            title: self.title.clone(),
            lines,
            truncated,
            commit_details: self.commit_details.clone(),
            pull_request_details: None,
        }
    }
```

### The per-file state machine

For each file the assembler evaluates two booleans, loaded and visible, and emits one of four
row shapes. Laid out as a table:

| Loaded | Visible | Rows emitted |
| --- | --- | --- |
| yes | yes | the loaded document's real rows, header label rewritten from the index |
| no | yes | synthetic header, then a Meta row "Loading diff…" |
| yes | no | synthetic header with real counts, then "Diff loaded · expand this file to display it" |
| no | no | synthetic header, then "Expand this file to load its diff" |

Every branch ends with a `FileFooter` row, so file boundaries exist in all four states and the
row structure the fold and navigation logic depends on (header, body, footer) is invariant
across loading progress. The capacity hint of `files.len() * 3` encodes the expected skeleton
shape: header, one meta row, footer per file.

The two collapsed-state messages are deliberately different sentences. "Diff loaded · expand
this file to display it" tells the reader the cost of expanding is zero (the data is already in
memory); "Expand this file to load its diff" tells them expanding will trigger a fetch. A
progressive UI earns trust by never letting the same action have two silently different costs.

### Two directions of header truth

The header handling is the subtlest part, because information flows in both directions between
the index and the loaded patch:

**1. The index wins on the label.** When a loaded document is spliced in whole, its header's
first span is overwritten with `file.label()`. The index knows things the patch parse may not:
the rename origin ("renamed from ..."), the status word, the binary marker, all derived from the
name-status listing and the counts (`DiffFileIndexEntry::label`, `src/git/diff.rs:116-129`). The
patch-derived header text would be a regression, so it never survives.

**2. The patch wins on the counts.** When a synthetic header is built for a file that is loaded
but collapsed, spans 1 and 2 (the `+n` and `-n` strings) are copied from the loaded header,
replacing whatever the index had, including the `+··` placeholder. A collapsed file whose patch
arrived therefore shows its real totals even though not one body row of it is materialized. The
test `lazy_index_keeps_all_headers_while_merging_one_loaded_file` (`src/git/diff.rs:987-1060`)
pins this: with every file collapsed, the loaded file's header still reads `+1` while the
unloaded one keeps its placeholders, and both collapsed-state meta strings appear.

### Totals that do not depend on loading progress

A subtle trap in a progressive viewer is any aggregate computed over materialized rows: it would
creep upward as patches load, making the summary line a progress bar in disguise showing wrong
data. Quinjet splits the aggregates by source. `DiffIndex::line_counts`
(`src/git/diff.rs:205-214`) folds per-file counts from the index alone, skipping unknown-count
files, so the totals a details card shows are stable from the moment the index arrives.
`DiffDocument::file_count`, `addition_count`, and `deletion_count` count materialized rows and
are used only where materialization is the question being asked, such as the loading-label
suppression in stage 3. The test `indexed_totals_do_not_depend_on_loaded_or_visible_patches`
(`src/git/diff.rs:1175-1219`) makes the split explicit: an index reports 15 additions through
`line_counts()` while the fully collapsed document's `addition_count()` is 0.

### Why collapsed files cost nothing

The visibility predicate passed by the app reflects fold state, and it interacts with the memory
budget: a collapsed file's loaded rows are not cloned into the combined document at all, just
its header. ARCHITECTURE.md invariant 6 states the consequence: "collapsed cached patches are
not cloned into the combined document, so post-processing remains bounded after Git returns".
On a 2,188-file pull request with everything collapsed except the file being read, the combined
document is roughly `3 x 2,188` skeleton rows plus one real body, a few thousand rows instead of
a million, and the row-layout pass over it (see [the viewport page](./viewport.md)) stays
proportional to the skeleton, not to the patch data. Folding is thus not just a reading
convenience; it is the memory and layout escape valve for pathological pull requests, and it
composes with loading state because the assembler treats "collapsed" and "unloaded" as
independent axes.

## Count backfill from arrived patches

Stage 2 leaves some headers showing `+·· -··`: files the API reported as 0/0 because it could
not count them, minus the pure-rename exemption. Those placeholders need an exit path, and #55
gives them one: the moment any file's real patch arrives, its true totals are computed from the
parsed document and written into the index entry. From `src/app.rs:5879-5907`:

```rust
    /// A finished patch knows its real totals, so a file whose counts GitHub
    /// could not report fills its header in as soon as its document arrives.
    fn backfill_pull_request_counts(&mut self, path: &Path, document: &DiffDocument) -> bool {
        if document.truncated {
            return false;
        }
        let Some(file) = self
            .pull_request_files
            .iter_mut()
            .find(|file| file.path == path && file.counts.is_none())
        else {
            return false;
        };
        let mut additions = 0_usize;
        let mut deletions = 0_usize;
        for line in &document.lines {
            match line.kind {
                DiffLineKind::Added => additions = additions.saturating_add(1),
                DiffLineKind::Removed => deletions = deletions.saturating_add(1),
                _ => {}
            }
        }
        file.counts = Some(DiffLineCounts {
            additions,
            deletions,
            binary: false,
        });
        true
    }
```

Each guard implements one of the progressive-loading requirements from
[the discipline section](#progressive-loading-as-a-discipline):

**1. Truncated documents are not truth.** A patch cut at the 8 MiB read cap has lost an unknown
number of trailing lines; counting its rows would produce a confident wrong number. The function
refuses, leaving the honest placeholder, and the file remains eligible for a later complete
read.

**2. Known counts are never overwritten.** The find predicate requires `counts.is_none()`, which
gives the whole mechanism monotonicity: API-reported counts, numstat-derived counts, and a
previous backfill all rank above a recount. It also makes the operation idempotent; a document
arriving twice (a manual reload after a prefetch, say) changes nothing the second time.

**3. The count is derived from parsed rows, not raw bytes.** Counting `DiffLineKind::Added` and
`Removed` rows in the parsed document reuses the parser's judgment about what is content versus
header (`+++`/`---` lines, hunk headers, meta rows never count), so the backfilled number is
computed by the same rules as every other count in the system.

The boolean return value feeds the caller's decision about whether anything visible changed.
Backfill runs at both arrival sites in the worker-result handler. The single-file arrival
(`WorkerEvent::PullRequestDiff`, `src/app.rs:3280-3282`) calls it unconditionally for the
arrived path. The batch arrival threads it through an accumulator, from `src/app.rs:3316-3333`:

```rust
                    Ok(documents) => {
                        self.pull_request_prefetch_retrying = false;
                        let mut arrived_visible = false;
                        let mut counts_changed = false;
                        for (path, document) in documents {
                            if !self.pull_request_documents.contains_key(&path) {
                                arrived_visible = arrived_visible
                                    || !self.preview_file_collapsed(&path.to_string_lossy());
                                counts_changed |=
                                    self.backfill_pull_request_counts(&path, &document);
                                self.cache_pull_request_document(path, document);
                            }
                        }
                        if (arrived_visible || counts_changed)
                            && self.pull_request_file_view == PullRequestFileView::AllFiles
                        {
                            self.rebuild_pull_request_all_files_document();
                        }
                        self.request_pull_request_prefetch(&mut effects);
                    }
```

The rebuild condition is the interesting line. A batch of patches for entirely collapsed files
would change nothing the reader can see, so rebuilding the combined document for it would be
wasted work; `arrived_visible` gates that. But a count backfill changes a header, and headers
are visible even for collapsed files, so `counts_changed` forces the rebuild in exactly the case
where the only visible delta is a `+·· -··` flipping to real numbers. Without that second flag,
a collapsed countless file would keep its skeleton until some unrelated visible file happened to
arrive, a subtle staleness the disjunction closes. After the merge, the handler immediately
requests the next batch, which is what turns the batch cycle into a self-sustaining stream.

## Folding as a loading control

Fold state and load state are independent axes in the assembler's four-way table, but from the
reader's side folding is also a control surface over loading, and on a huge pull request it is
the most powerful one available.

The visibility predicate passed to `document_with_visibility` comes from the app's fold state
(`visible_preview_paths` over the index paths), and the content pane advertises the bulk
controls in its title: "  [e Expand all]" or "  [e Collapse all]" whenever the current document
has collapsible files (`src/ui/mod.rs:3263-3271`). Three behaviors give folding its leverage:

**1. Collapsing is free and immediate.** A collapsed file contributes exactly one header row to
the combined document, whatever its load state. Collapse-all on a 2,188-file pull request
produces a headers-only document of a few thousand skeleton rows, which the row-layout pass
walks in microseconds; the unified row builder additionally fast-forwards past collapsed bodies
so they do not even appear in the row list (the test
`collapse_all_keeps_only_selectable_file_headers` pins a two-collapsed-file document to row
indices `vec![0, 4]`). For a reader triaging a rewrite, collapse-all first and expand
selectively is the intended workflow, and the assembler's count copy-back means the collapsed
headers still show real totals as patches arrive.

**2. Hidden work is never done.** Invariant 10's clause "hidden descendants never trigger diff
work" extends through the whole pipeline: a collapsed directory's files are absent from the
flattened tree, so they cannot anchor the prefetch; a collapsed file's arrived rows are not
cloned into the combined document; and a batch that delivers only collapsed files does not force
a document rebuild unless it also changed a header count. Folding a subtree is therefore a real
statement of disinterest that the scheduler and the assembler both respect.

**3. Expanding tells you its price, then pays it.** The two collapsed-state meta rows,
"Diff loaded · expand this file to display it" versus "Expand this file to load its diff",
distinguish a free expansion from one that will fetch. Expanding re-includes the path in the
visibility set; if the document is cached the body appears on the next frame, and if not the
file renders "Loading diff…" and becomes an ordinary candidate for the interactive load and the
background walk, both of which check the same document cache and therefore converge on one
fetch.

The net effect is that fold state acts as a reader-controlled admission filter in front of both
memory and layout cost, which is why the assembler was designed with visibility as an explicit
predicate parameter rather than pre-filtering the index: the same index, documents, and
machinery serve every fold configuration without recomputation of anything but the assembly
itself.

## Viewport-anchored wrap-around prefetch

With coherent partial rendering in place, the remaining lever is order: given that 2,188 patches
will take a while to arrive, which should arrive first? #55's answer is the file the reader is
currently looking at, then everything after it in index order, then everything before it,
wrapping around. This section covers the scheduler in detail; the mailbox and lane plumbing
underneath it is shared with [the prefetch page](../github/prefetch.md).

### Scheduling theory in one paragraph

Background fill is a single-server scheduling problem: one prefetch lane, many pending files,
choose the service order. Classic orderings optimize different objectives. FIFO in index order
is fair but attention-blind. Shortest-job-first (which #50 approximated with its smallest-first
sort) provably minimizes mean completion time, so it maximizes how fast the count of ready files
grows, but it says nothing about which files those are. What a reader actually experiences is
the latency of the specific files scrolled into view, which is a locality objective: serve
nearest-to-the-point-of-attention first. Disk schedulers faced the same tension decades ago and
settled on elevator-style sweeps for the same reason Quinjet settled on an anchored rotation:
pure priority orders can starve regions entirely, while a sweep from the point of interest
covers everything with bounded delay and still serves the hot region first. The wrap-around walk
is exactly a one-directional elevator over the index, re-aimed at every batch boundary.

### The anchor

`prefetch_anchor_index` (`src/app.rs:5909-5925`) computes where the walk starts:

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

The anchor is derived from render state, not selection state, and that distinction carries three
consequences:

**1. The anchor is what the reader sees, not what they clicked.** `sidebar_offset` is the tree's
scroll position: the number of tree rows scrolled above the viewport. Skipping that many rows and
taking the first `File` entry yields the topmost file currently on screen. Because PR #54
decoupled wheel scrolling from selection (wheel panning moves `sidebar_offset` without moving
the cursor), simply panning the tree down to a distant directory retargets the prefetch there,
before any click. The two features compose into "look at it and it starts loading".

**2. Directory rows are transparent.** The tree interleaves `Directory` and `File` entries; if
the top visible row is a directory heading, `find_map` continues to the first file below it.
Collapsed directories contribute nothing because their descendants are not in the flattened tree
at all (the tree builder skips children of collapsed directories, so hidden files cannot anchor
anything, consistent with invariant 10's "hidden descendants never trigger diff work").

**3. Outside the Files section the anchor is zero.** When the reader is on the Overview section
or a different view entirely, there is no viewport to chase, and the walk degrades to plain
index order from the top, which is the correct neutral policy.

The result is clamped by the caller (`.min(self.pull_request_files.len())`) so a stale offset
past the end of a shrunken index cannot make `split_at` panic.

### The batch builder

`request_pull_request_prefetch` (`src/app.rs:5927-5977`) turns the anchor into one bounded batch
per call:

```rust
    /// Walk the index in batches until every file has a patch. Each batch is one
    /// Git invocation and lands as soon as it is parsed, so the diff fills in
    /// progressively instead of a file at a time on demand.
    fn request_pull_request_prefetch(&mut self, effects: &mut Vec<AppEffect>) {
        if self.pull_request_prefetching {
            return;
        }
        let Some(workspace_generation) = self.pull_request_workspace_generation else {
            return;
        };
        if self.pull_request_prefetched_paths.len() >= MAX_PREFETCHED_PULL_REQUEST_FILES {
            return;
        }
        let remaining = MAX_PREFETCHED_PULL_REQUEST_FILES
            .saturating_sub(self.pull_request_prefetched_paths.len());
        let limit = PULL_REQUEST_PREFETCH_BATCH.min(remaining);
        let anchor = self
            .prefetch_anchor_index()
            .min(self.pull_request_files.len());
        let (before, from_anchor) = self.pull_request_files.split_at(anchor);
        let mut batch_bytes = 0_usize;
        let mut paths: Vec<PathBuf> = Vec::new();
        for file in from_anchor.iter().chain(before.iter()) {
            if paths.len() >= limit {
                break;
            }
            if !self.pull_request_file_needs_patch(&file.path)
                || self.pull_request_prefetched_paths.contains(&file.path)
            {
                continue;
            }
            let estimate = estimated_patch_bytes(file.counts);
            if !paths.is_empty()
                && batch_bytes.saturating_add(estimate) > PULL_REQUEST_PREFETCH_BYTE_BUDGET
            {
                break;
            }
            batch_bytes = batch_bytes.saturating_add(estimate);
            paths.push(file.path.clone());
        }
        if paths.is_empty() {
            return;
        }
        self.pull_request_prefetching = true;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFileBatch {
                workspace_generation,
                paths,
            },
        )));
    }
```

The rotation is the two-line idiom at the center: `split_at(anchor)` divides the index into the
part before the viewport and the part from the viewport onward, and
`from_anchor.iter().chain(before.iter())` walks the second part first. No allocation, no sort,
no auxiliary index: the entire ordering policy is an iterator adapter over the existing `Vec`.

The eligibility filter has two layers with different lifetimes. `pull_request_file_needs_patch`
(`src/app.rs:5871-5877`) is the current-truth check:

```rust
    /// A path still needs its patch unless it is already cached, already in
    /// flight, or currently occupying the single-file document.
    fn pull_request_file_needs_patch(&self, path: &Path) -> bool {
        !self.pull_request_documents.contains_key(path)
            && self.pull_request_loading_path.as_deref() != Some(path)
            && self.pull_request_single_file.as_deref() != Some(path)
    }
```

`pull_request_prefetched_paths` is the historical check: every path ever handed to a batch,
which is what the 4,096-file cap counts. The distinction matters at the memory boundary: a
document evicted by the 32 MiB budget would pass the first check again but not the second, so
eviction cannot put the scheduler into a refetch loop against its own memory ceiling.

### Byte budgeting and its worked example

Each candidate file contributes an estimated patch size, from `estimated_patch_bytes`
(`src/app.rs:7052-7060`):

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

The model: each changed line costs about `PULL_REQUEST_PATCH_LINE_ESTIMATE` (80) bytes on the
wire, plus a 4,096-byte fixed overhead per file for the `diff --git` header block, hunk headers,
and the context lines that `--unified=3` adds around changes. A file with no counts at all is
assumed to cost `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE` (512 KiB), a deliberately pessimistic
figure: an unknown file is, by construction of the 0/0 skip rule, likely one of the huge
generated files GitHub refused to count.

Worked through with the actual constants:

| File | Counts | Estimate |
| --- | --- | --- |
| `src/js_parser.zig` | +120 -30 | 150 x 80 + 4,096 = 16,096 bytes |
| `README.md` | +12 -3 | 15 x 80 + 4,096 = 5,296 bytes |
| `src/generated/h2_client.rs` | unknown (0/0 skipped) | 524,288 bytes |
| pure rename | +0 -0 kept | 0 x 80 + 4,096 = 4,096 bytes |

A batch accumulates estimates until adding the next file would cross
`PULL_REQUEST_PREFETCH_BYTE_BUDGET` (6 MiB), or until 32 files are queued, whichever comes
first. Small files pack densely: at the 5 KiB scale of typical source changes, the 32-file cap
binds long before the byte budget does. Unknown-count files dominate quickly: twelve of them at
512 KiB each fill the 6 MiB budget exactly, so a run of countless files travels in batches of
twelve. And one guard keeps the system live at the extreme: the budget check requires
`!paths.is_empty()`, so a single file whose estimate alone exceeds 6 MiB still travels, alone,
rather than deadlocking the walk.

The 6 MiB figure is headroom engineering against the hard cap downstream. The batch becomes one
`git diff` invocation whose stdout is capped at `MAX_DIFF_BYTES` (8 MiB) with a
kill-on-overflow pipe (see [the pipeline page](../diff/pipeline.md)). Estimates err in both
directions (long lines cost more than 80 bytes, context lines are uncounted), so the scheduler
aims 2 MiB under the truncation cap; an accurate-on-average batch then almost never truncates,
and when one does, the split-and-fallback logic in `diff_files` recovers file by file
(covered in [Failure modes and edge cases](#failure-modes-and-edge-cases)).

### Re-anchoring at every batch boundary

The scheduler holds no queue. Each call computes the anchor fresh, walks the index fresh, and
emits at most one batch; the arrival handler for that batch calls the scheduler again. The
consequence is that the fill order re-plans itself every batch: scroll somewhere else while
batch 7 is in flight, and batch 8 starts at the new viewport. The in-flight batch is never
cancelled (it is one Git invocation, already mostly paid for), so the cost of a retarget is at
most one batch of latency, bounded by the byte budget at roughly a second of Git work. A
persistent priority queue would need explicit rebalancing to do the same; deriving the plan from
current state each time gets retargeting for free, which is the same design instinct as the
derived `local_diff_load_progress` counter.

The test `prefetch_starts_at_the_files_viewport_and_wraps_around` (`src/app.rs:8972-9011`) pins
the whole policy in miniature: four files `a.rs` through `d.rs`, the Files section showing with
`sidebar_offset = 2`, and the asserted batch order is `[c.rs, d.rs, a.rs, b.rs]`, the visible
file first, wrap-around after.

### Interaction with the 4,096-file cap

`MAX_PREFETCHED_PULL_REQUEST_FILES` was raised by #55 from 400 to 4,096, chosen to cover real
huge pull requests entirely (bun#30412's 2,188 files fit with room to spare) while still capping
the pathological end (the index itself truncates at 16,384 paths). The cap is enforced
incrementally: `limit = PULL_REQUEST_PREFETCH_BATCH.min(remaining)` shrinks the final batch so
the cap lands exactly, not approximately. Past the cap, files are not lost, they merely return
to on-demand loading: selecting one issues a single-file `LoadPullRequestFile` through the
preview lane, same as before prefetch existed.

## From smallest-first to viewport-first

The wrap-around walk replaced an earlier, different answer to the ordering question, and the
replacement is worth documenting as an evolution step because both answers are defensible and
the reason one lost is instructive.

### What #50 shipped

PR #50 ("perf: prefetch smallest files first on huge pull requests", commit `133e28a`) kept
index order for ordinary pull requests but re-sorted the candidate list for huge ones. Two
thresholds defined "huge":

```rust
const HUGE_PULL_REQUEST_LINES: usize = 100_000;
const HUGE_PULL_REQUEST_FILES: usize = 1_000;
```

When a pull request's `additions + deletions` reached 100,000 or its file count reached 1,000,
the scheduler collected the files and sorted them ascending by the same
`estimated_patch_bytes` used for budgeting:

```rust
let mut candidates: Vec<&PullRequestFile> = self.pull_request_files.iter().collect();
if huge {
    candidates.sort_by_key(|file| estimated_patch_bytes(file.counts));
}
```

`sort_by_key` is stable, so equal-size files kept index order. The PR body states the objective
plainly: "Spend the prefetch budget on the smallest files first once a pull request crosses 100k
changed lines or 1,000 files, so most of the tree opens instantly." This is shortest-job-first:
with a fixed stop at 400 files and a 6 MiB budget per batch, filling batches smallest-first
maximizes how many files end up with a ready patch before the stop, which maximizes the fraction
of tree clicks that hit a warm document.

### Why it was the right call at 400 and the wrong call at 4,096

Smallest-first optimizes a coverage ratio, and coverage ratio is the right objective exactly
when coverage is scarce. At a 400-file stop against a 2,188-file index, 82 percent of files
would never prefetch, so making the covered 18 percent be the cheapest 18 percent (and therefore
the most numerous per byte) was a sound triage rule.

PR 55 changed the constraint that made triage necessary. With the stop raised to 4,096, every file
of a bun-scale pull request prefetches eventually; the question is no longer which files make
the cut but in what order they all arrive. Under that regime smallest-first has a real cost: the
files the reader is looking at load last precisely when they are large, and a reader who opens a
huge pull request usually navigates straight to the large, interesting rewrites, not to the
one-line version bumps that smallest-first serves first. Meanwhile the skeleton work in the same
PR (placeholders, count backfill, coherent partial documents) had lowered the price of an
unloaded file from "blocking spinner" to "header with a placeholder", which shrank the value of
raw coverage. Attention-locality won on both sides of the trade.

The removal was total: #55 deleted both `HUGE_` constants and the sort, so no size-tier code
path remains at HEAD, and the test `huge_pull_requests_prefetch_their_smallest_files_first` was
rewritten into `prefetch_starts_at_the_files_viewport_and_wraps_around`. The invariant text in
ARCHITECTURE.md was rewritten in the same commit, from "a pull request past 100,000 changed
lines or 1,000 files spends that budget on its smallest files first" to the wrap-around wording
quoted earlier. Smallest-first ordering exists only in history, between commits `133e28a` and
`1261472`; readers of the code at HEAD will find no trace of it, which is why this page records
it. Its lasting legacy is `estimated_patch_bytes` promoted from a batch-sizing detail into the
shared cost model, and the demonstration that the ordering policy is a one-place decision: both
the sort and the rotation were expressed entirely inside `request_pull_request_prefetch`, so the
replacement was a contained edit rather than a redesign.

The general catalog of both techniques, size-aware scheduling and viewport anchoring, lives in
[the techniques page](../techniques.md).

## Generation tags and stale replies

A progressive loader multiplies in-flight asynchrony: at any instant there may be a preview
request, a prefetch batch, a metadata refresh, and a conversation page all pending, and the user
can invalidate the premise of any of them with one keypress. Quinjet's defense is the generation
tag, applied end to end (ARCHITECTURE.md invariant 2: "Every preview, status, history, branch,
and pull-request request carries a generation; stale replies are ignored"). This section covers
the two generations that matter to the file view, and specifically why they are two rather than
one. The full threading model lives in [the concurrency page](./concurrency.md).

### The mechanism in general

A generation tag is a monotonically advancing counter attached to a request when it is issued
and echoed back verbatim in the reply. The state owner bumps the counter whenever the question
changes (a different file selected, a different workspace prepared), and the reply handler's
first act is an equality check: a reply tagged with anything but the current counter is dropped
on the floor, before any state is touched. The pattern costs one `u64` per request and one
comparison per reply, requires no cancellation machinery in the worker (workers may finish stale
work; it just lands in the bin), and turns "did the world change while this was in flight" into
a local, testable predicate. Quinjet's counters use `wrapping_add(1)`, so overflow is defined
behavior; a collision would require 2^64 intervening bumps, which is not a practical concern.

### Preview generation versus workspace generation

The file view uses two tags with deliberately different bump conditions:

**1. `diff_generation`, the preview tag.** Bumped every time the reader asks to look at
something different (`src/app.rs:5858` shows the bump as a single-file request is issued). The
single-file reply handler checks it first (`src/app.rs:3264-3267`):

```rust
            WorkerEvent::PullRequestDiff { generation, result } => {
                if generation != self.diff_generation {
                    return effects;
                }
```

A reader who selects `a.rs` and then immediately `b.rs` has issued two requests; when the `a.rs`
document arrives, its tag no longer matches and it is discarded, so the pane can never flash the
previous selection over the current one.

**2. `pull_request_workspace_generation`, the workspace tag.** Bumped only when a different
prepared workspace comes into existence, that is, when the pull request (or its head) changes.
Batch replies check this one instead (`src/app.rs:3307-3314`):

```rust
            WorkerEvent::PullRequestDiffBatch {
                workspace_generation,
                result,
            } => {
                if Some(workspace_generation) != self.pull_request_workspace_generation {
                    return effects;
                }
                self.pull_request_prefetching = false;
```

The asymmetry is the point, and the command type documents it, from `src/git/worker.rs:63-70`:

```rust
    /// Background fill for the rest of a prepared pull request. It carries no
    /// preview generation because it never replaces what the reader is looking
    /// at; the workspace it was prepared against is the only thing that can
    /// make its results stale.
    LoadPullRequestFileBatch {
        workspace_generation: u64,
        paths: Vec<PathBuf>,
    },
```

If batches carried the preview generation, every selection change would orphan the in-flight
batch: perfectly valid patches for the still-open pull request would be discarded because the
reader glanced at a different file while they were in transit. On a huge pull request, where
selection changes constantly and batches are always in flight, preview-tagging the batches would
starve the fill indefinitely. The correct staleness criterion for a batch is "is this still the
same base...head workspace", nothing finer. ARCHITECTURE.md invariant 10a states it as the
contract: batched background reads are "keyed to the prepared workspace rather than to a preview
generation, so they can never invalidate a reader's own request."

The two tags also compose with the correctness of the caches they guard. A stale batch reply
for a previous workspace would carry documents computed against a different merge base; merging
them into the current document map would be silent corruption, and the workspace check makes it
impossible. Conversely, documents for the current workspace are safe to merge no matter how the
reader has navigated meanwhile, because the cache is keyed by path within one immutable
base...head pair (see [the caching page](../github/caching.md)).

### Where the checks live

Both checks sit at the top of the single `match` arm that handles the reply, before any field is
mutated. That placement is a rule, not a convention: a generation check after a partial mutation
would leave torn state exactly in the race it exists to prevent. It also means dropped replies
cost nothing downstream; no rebuild, no invalidation, no render change, because no state
changed. The batch arm resets `pull_request_prefetching` only after the workspace check passes.
A stale batch reply must not clear the flag for a new workspace's in-flight batch; when a
workspace is torn down and rebuilt, the flag is reset along the workspace lifecycle instead, so
the scheduler of the new workspace starts clean rather than inheriting the old stream's state.

### The mailbox slot behind the preview

Generation tags reject stale answers; the mailbox arrangement prevents a subtler failure, a
pending batch displacing the answer the reader is actively waiting for. The worker's coalescing
mailbox has a dedicated `prefetch` slot, separate from the `preview` slot, from the routing in
`src/git/worker.rs:240-248`:

```rust
            command @ (WorkerCommand::PrepareLocalDiff { .. }
            | WorkerCommand::LoadLocalDiffFile { .. }
            | WorkerCommand::PreparePullRequest { .. }
            | WorkerCommand::LoadPullRequestFile { .. }) => {
                self.preview = Some(command);
            }
            command @ WorkerCommand::LoadPullRequestFileBatch { .. } => {
                self.prefetch = Some(command);
            }
```

Both slots drain onto the same `WorkerLane::PullRequestPreview` lane
(`src/git/worker.rs:315`), with the preview slot popped first, so an interactive selection
always jumps ahead of background fill, and a queued batch coalesces with (replaces) an older
queued batch rather than piling up. ARCHITECTURE.md invariant 3 names the guarantee:
"Background diff prefetch occupies its own mailbox slot behind the preview slot, so a queued
batch can never displace the preview a reader is waiting for." Progressive loading leans on
this daily: the fill stream is aggressive precisely because the mailbox structurally prevents
it from ever adding latency to the interaction it serves.

## Memory bounds while patches stream

A fill stream that walks 4,096 files must not accumulate 4,096 parsed documents. A parsed
`DiffDocument` is materially larger than its raw patch: every line is a struct, every syntax
span an owned `String`, so a million-line pull request parsed in full would occupy hundreds of
megabytes of heap. Quinjet bounds the resident set with an explicit byte-accounted cache and
FIFO eviction, sized at `MAX_PULL_REQUEST_DOCUMENT_BYTES` (32 MiB, `src/app.rs:38`).

### Accounting

Every insertion and eviction adjusts a running total computed by `diff_document_size`
(`src/app.rs:7062-7074`):

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

The estimate measures the parsed representation, structs plus string capacities, which is why
the in-memory budget (32 MiB) is deliberately larger than the raw patch read cap (8 MiB): the
same content costs more once decomposed into per-line, per-span allocations, and using
`capacity()` rather than `len()` counts what the allocator actually holds. It is an estimate,
not an audit (allocator overhead and map buckets are uncounted), but it is computed by the same
function on insert and evict, so the running total cannot drift even if it is uniformly biased.

### Insertion and eviction

`cache_pull_request_document` (`src/app.rs:5742-5757`) keeps three structures consistent: the
`HashMap` of documents, a `VecDeque` recording insertion order, and the byte total. Re-inserting
a path first subtracts and unlinks the previous entry, so replacement is not double-counted.
Insertion also records the path into `pull_request_prefetched_paths`, tying memory bookkeeping
to the scheduler's history set. Then the budget is enforced (`src/app.rs:5759-5772`):

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

Eviction is FIFO by insertion order: the oldest arrival goes first. That approximates
least-recently-useful under the wrap-around fill (documents arrive in attention order, so the
oldest arrivals are the ones the viewport left behind longest ago) without any per-access
bookkeeping on the render path, which must stay allocation- and mutation-free. The
`len() > 1` guard makes the newest document unevictable: a single patch larger than the whole
budget still renders rather than thrashing in and out of its own cache.

The interplay with the scheduler noted earlier completes the loop: an evicted path fails
`pull_request_documents.contains_key` but remains in `pull_request_prefetched_paths`, so the
background walk does not refetch it; selecting it interactively loads it on demand through the
preview lane. Memory pressure therefore converts the coldest prefetched files back into
on-demand files, silently and reversibly, which is the graceful degradation a bounded
progressive system needs. This was itself a review lesson: the pre-stack code had exactly this
eviction bookkeeping wrong (evicted paths were never re-loadable by prefetch and the budget
interacted badly with the 400-file stop), catalogued as failure mode 8 in the baseline
analysis.

## Stopping the poll on settled pull requests

Progressive loading governs how data arrives; the adaptive poll governs how often Quinjet asks
whether there is new data at all. #55 added one gate to that loop: a merged or closed pull
request stops being polled. The full polling design is documented in
[the API strategy page](../github/api-strategy.md); this section covers the #55 gate and its
placement.

### The cadence being gated

The poll constants, with their rationale doc comments, from `src/app.rs:39-52`:

```rust
/// Poll cadences for an open pull request. A run in progress changes state in
/// seconds and is worth watching closely; a settled pull request only needs to
/// notice new comments; a pull request nobody is looking at needs less again.
const PULL_REQUEST_ACTIVE_POLL: Duration = Duration::from_secs(5);
const PULL_REQUEST_IDLE_POLL: Duration = Duration::from_secs(20);
const PULL_REQUEST_BACKGROUND_POLL: Duration = Duration::from_secs(120);
/// Each live stream costs its own GitHub requests, so the tick cadence is a
/// ceiling rather than a schedule: check state is the only thing worth reading
/// as often as the tick fires. Metadata, the conversation and a growing log all
/// change on human or build timescales and hold their own floor.
const PULL_REQUEST_DETAIL_POLL: Duration = Duration::from_secs(20);
/// A running job's log grows continuously, so this is a tail interval rather
/// than a staleness bound.
const PULL_REQUEST_LOG_POLL: Duration = Duration::from_secs(8);
```

### The gate

`refresh_pull_request_live` (`src/app.rs:3013-3065`) runs each stream in a fixed order:
checks first, then the gate, then metadata and conversation, then the log tail. The gate itself
(`src/app.rs:3040-3046`):

```rust
        let settled = self
            .pull_request
            .as_ref()
            .is_some_and(|pull_request| matches!(pull_request.state.as_str(), "MERGED" | "CLOSED"));
        if settled && !force {
            return;
        }
```

Three properties fall out of the placement and the `force` flag:

**1. Settled state is judged from the already-loaded metadata.** No request is spent deciding
whether to spend requests. `MERGED` and `CLOSED` are terminal states in GitHub's model (a
reopened pull request transitions to `OPEN`, which is a new answer the next explicit read
observes), so the check is a string match on state Quinjet already holds.

**2. The checks read escapes the gate on purpose.** The gate sits after the checks-stream
dispatch, so a settled pull request's check list still refreshes on the tick cadence while the
metadata, conversation, and log streams stop. Checks are the one stream that can still move on
a settled pull request in a way a reader watching it cares about (re-runs against the merged
head), and the check list is a cheap 30-second-TTL read.

**3. `force` preserves the two legitimate refresh paths.** The flag is true for a forwarded
webhook delivery, which is by definition evidence that something changed and bypasses every
floor; and an explicit user reload takes a different path entirely and is unaffected by the
gate. ARCHITECTURE.md invariant 11 captures the contract in one sentence: "A merged or closed
pull request is not polled at all; a webhook delivery or an explicit reload still refreshes it."

### Why this belongs to the huge-PR story

The gate reads as an API-economy nicety, but it earns its place in #55 because of how these
pull requests are actually used. A million-line rewrite is studied after it lands: bun#30412
was merged when the whole optimization session benchmarked it, and reading a merged pull
request is the archaeology case, potentially hours in one view. Without the gate, that reading
session pays a metadata plus conversation read every 20 seconds indefinitely, against content
that provably cannot change, burning rate limit that the progressive fill (counts pages, compare
API, conversation pages) actually needs. Rate-limit pressure was not hypothetical: the working
session itself was interrupted twice by API 429 responses. Stopping the poll on settled pull
requests converts the long-read case from a steady request drain into zero background traffic,
while webhooks and manual reloads keep every legitimate refresh path open.

## The alternates borrow for squash-merged pull requests

The second checkpoint commit of #55 ("perf: borrow local objects in the PR workspace") attacks
a cost that viewport ordering cannot: the network round trip under each batch when the
disposable workspace is blob-less. It does so with one of Git's oldest sharing mechanisms, and
it exists because of a user report worth retelling.

### The mystery: everything is local, so why is it slow?

During the session, the user ran the TUI against a full local clone of bun at `~/Desktop/bun`
and still watched per-file "Loading diff…" crawls, asking (quoted in the session notes)
"Everything is local. Why is it taking so much time to load this for each of the files here?"
The diagnosis: bun squash-merged the rewrite, so the pull request's head commit `ed1a70f8`
exists on GitHub's `refs/pull/30412/head` and nowhere in the clone's `main` history. A squash
merge lands one new commit whose snapshot equals the branch's final tree but whose identity is
new; the branch's own commits never enter the target history (see
[the merge-bases page](../git-internals/merge-bases-and-history.md)). Quinjet only uses the
opened repository directly when both PR commits are locally present, because it refuses to
fetch into a repository it does not own (invariant 9: no ref, index, or worktree mutation). So
it fell back to the disposable blob-less workspace, and every batch of files became lazy blob
downloads from GitHub, in a directory sitting inches from a clone that already contained
nearly all of those blobs.

That last clause is the exploitable fact. A squash merge changes commit identity, not blob
identity: the merged tree's file contents are byte-identical to the PR head's, so the clone's
object store already holds most blobs the PR diff needs, just reachable from different commits.
Content addressing makes them findable by hash regardless of which ref led to them (see
[the object model page](../git-internals/object-model.md)).

### Git alternates in general

Git's object lookup consults, in order: loose objects under `objects/`, packfiles under
`objects/pack/`, and then every object database listed in `objects/info/alternates`, one path
per line, each treated as an additional read-only object store (the file is specified in
[gitrepository-layout](https://git-scm.com/docs/gitrepository-layout)). Alternates predate
partial clone by decades; they exist so related repositories can share one object store. Two
properties matter here. First, alternates are read-only by convention and by mechanism: the
borrowing repository writes its own new objects into its own store, never into the alternate.
Second, in a partial-clone repository the promisor fetch is the last resort: a missing object
is looked up through the entire local chain, alternates included, before any network request is
made (see [the shallow and partial clone page](../git-internals/shallow-and-partial-clone.md)).
That ordering is exactly the hook: give the blob-less workspace an alternate pointing at the
user's clone, and lazy blob reads that would have been HTTPS round trips become local file
reads whenever the clone has the bytes.

The classic operational hazard of alternates, that pruning the alternate store corrupts the
borrower, is structurally absent here: the borrower is a disposable workspace that lives for
one viewing session and is deleted on drop, while the alternate is the user's own repository,
which outlives it.

### The implementation

`TemporaryBareRepository::borrow_local_objects` (`src/git/github/mod.rs:1728-1745`):

```rust
    /// Let the disposable workspace read the opened repository's objects. A
    /// merged or locally built pull request usually already has most of its
    /// blobs on disk under other refs, so lazy blob reads resolve from the
    /// local store instead of the network. The opened repository is only read.
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
}
```

Small as it is, each line encodes a decision:

**1. The common directory, not the worktree.** `git_common_dir` runs
`git rev-parse --git-common-dir`, so when the opened repository is a linked worktree the borrow
resolves to the shared object store rather than the worktree's private administrative
directory (see [the refs and worktrees page](../git-internals/refs-index-and-worktrees.md)).

**2. Best effort, always.** Every failure path returns silently, and the write result is
explicitly discarded with `drop(...)`. The borrow is a pure optimization: without it the
workspace still works, just slower, and no error it could produce would be actionable for the
user. Optimizations that can fail loudly become reliability bugs; this one cannot.

**3. The opened repository is only read.** The alternates file is written inside the disposable
workspace's own `objects/info/`, not in the user's repository. Nothing in the user's clone
changes, preserving invariant 9's mutation guarantee. The direction of reference is from the
throwaway toward the durable, which is the safe direction.

The call site is `prepare_pull_request_diff` (`src/git/github/mod.rs:789`), immediately after
the temporary bare repository is created and before `fetch_pull_request` runs, so the borrow is
in place for the fetch negotiation itself as well as for later lazy blob reads: objects the
clone already has do not need to arrive in the fetch pack either.

### Effect on the progressive fill

The borrow multiplies the value of every mechanism above it. Batch cost in the disposable path
is dominated by lazy blob materialization; with the alternate in place, a squash-merged pull
request's batches hit the local store for almost every blob and the fill stream runs at local
`git diff` speed, network-touched only for the rare blob the clone lacks. The workspace-side
test `locally_available_pr_objects_avoid_disposable_fetches` (`src/git/github/mod.rs:2946`)
guards the adjacent contract (a PR whose commits are locally present completes preparation with
an unreachable network endpoint), and the mechanism was verified in session "end to end on
another merged bun PR whose head commit is absent from your clone". The session notes also
record the complementary manual escape hatch that predates the borrow: a one-time
`git fetch origin +refs/pull/30412/head:refs/remotes/origin/pr-30412` in the user's clone makes
both PR commits locally present, after which Quinjet takes the fully local, network-free path
with no workspace at all.

## Measured results

Every number in this section is quoted from the working session's notes, with its context. The
full benchmark methodology, including how to reproduce the setup, lives in
[the benchmarking page](../benchmarking.md); this section reports what the stack measured
against its target and what each figure means for the loading stages described above.

### The test bed

The benchmark clone was a shallow, blob-less clone of bun at `/tmp/bun-test`: 389 MB on disk,
`git rev-parse --is-shallow-repository` reporting true, with `remote.origin.promisor=true` and
`remote.origin.partialclonefilter=blob:none` in its config, fetching only `main`. Measurements
drove the CLI verbs (`quinjet pr view 30412`, `pr files`, `pr diff 30412 [path]`,
`pr conversation 30412`) with a release build of the top-of-stack branch (the build itself is
recorded as finishing "in 1m 01s"). Cold-cache runs were isolated with
`QUINJET_CACHE_DIR=$(mktemp -d) quinjet ...`, which points every cache, metadata, immutable
patch and counts entries, and the disposable `pr-*.git` workspaces, at a throwaway root; the
notes call this "exactly how I benchmarked the before/after numbers".

One honest caveat the session recorded: because bun#30412 is merged, its head is reachable in
`main`'s shallow history, so the pre-stack baseline also completed on this clone rather than
breaking outright. "Therefore correctness was not the differentiator on this exact clone,
timing was, and the baseline cold run was measured separately."

The CLI verbs measure the same machinery the TUI stages pay for: `pr view` is the stage-0
metadata read, `pr files` is workspace preparation plus the stage-2 index with counts, and
`pr diff <path>` is one stage-3/4 patch. What the numbers do not directly measure is the
TUI-only overlap, where these phases run concurrently on separate lanes and the reader
interacts throughout.

### First verification round

Measured at the top of the original five-PR stack, before the adversarial-review fixes, all
cold-cache unless stated:

- "Metadata in 1.7s" (`pr view` against bun#30412, cold). This is the stage-0 window: under two
  seconds after launch, title, state, counts, and refs exist, and the skeleton gives way to real
  chrome.
- "The rewrite PR enumerates all 2,188 files with real counts in 18.5s cold." This is `pr
  files`, including workspace preparation, and corresponds to the stage-2 moment where the full
  tree renders. Warm re-run of the index: 0.04s, the immutable `pr-files-v1` cache entry doing
  its job.
- Single-file patches: 0.1s, the stage-3 experience of selecting a file.
- "the 1,100-entry conversation in 21s with the newest activity preserved" (the conversation
  stream is its own progressive story, told in
  [the conversation page](../github/conversation-and-checks.md)).

The round's summary sentence ties them together: "the 1M-line 'Rewrite Bun in Rust' PR (#30412,
2,188 files) loads its full file index with real counts in 18.5s cold and 0.04s warm,
single-file patches in 0.1s, and the 1,100-entry conversation in 21s".

### Second verification round

Measured after all review fixes and the restack, on the final binary:

- "Final numbers on the bun PR: cold index 6.3s, warm 0.04s, conversation 26s with the honest
  truncation notice."
- Summary: "2,188-file/1M-line index in 6.3s cold, 0.04s warm, per-file patches instant,
  conversation newest-first in 26s."

Two deltas against the first round need explaining, and the notes explain both. The cold index
dropped from 18.5s to 6.3s with the review-fix round, which among other things rebased the
chain and included the counts-cache key fix. The conversation rose from 21s to 26s because the
fixed code degrades honestly rather than caching a gapped page-1 read: the earlier, faster
number was partly the product of a correctness bug found in review.

After a local install of the final build, a smoke test with warm metadata and the real cache:
"`q pr files 30412` lists all 2,188 files of the 1M-line rewrite PR in 1.4s."

### Reading the numbers as a user experience

Mapped back onto the stage sequence, the final figures say: a reader who opens the merged bun
rewrite cold sees identifying metadata in under two seconds, an explorable 2,188-file tree with
real counts in about six, and each selected patch effectively instantly, while the remainder of
the pull request streams in behind the viewport at 32 files per Git invocation. Reopening the
same pull request later costs 0.04 seconds for the index, because every expensive artifact was
cached under immutable OID-derived keys the first time. The pre-stack experience on the same
data was a single opaque wait covering fetch, blob storm, and enumeration before anything
useful rendered. The stack did not make the total data smaller; it reordered and bounded the
work so that the useful-frame timeline detached from the total-transfer timeline, which is the
entire thesis of progressive loading.

## Failure modes and edge cases

A progressive loader has more intermediate states than a monolithic one, and each is a place
for a subtle bug. This section catalogs the edges Quinjet handles deliberately, several of
which were found the hard way, by adversarial review of the stack before it merged.

**1. A batch whose patch truncates inside its first file section.** The 8 MiB read cap can cut
a combined patch anywhere, including inside the very first `diff --git` section when one file
is enormous. The pre-fix code dropped the partial section (correct for a middle-of-batch file),
found no sections for the other requested files, returned `Ok(vec![])`, cached nothing, and the
arrival handler immediately re-dispatched the identical batch: a tight worker loop re-running
the same 8 MiB `git diff` forever. The review notes give the trap's cleanest trigger: an added
minified bundle written as one 10 MB line has `additions = 1`, so its estimate is 80 + 4,096
bytes and the byte budget cannot see it coming. The fix defines the current contract in
`PreparedPullRequest::diff_files`: only the last section of a truncated combined patch can be
incomplete; a truncated last section in a multi-file batch is withheld from normal emission and
remembered as a fallback so a later request can retry it alone with the whole 8 MiB budget; and
if the batch produced nothing else, the truncated fallback document itself is returned, so a
single oversized file renders its truncated head instead of nothing, and the walk always makes
progress.

**2. A failed batch retries once, then yields.** From the arrival handler's error arms
(`src/app.rs:3336-3342`): the first failure sets `pull_request_prefetch_retrying` and
immediately re-requests; a second consecutive failure clears the flag and stops scheduling. The
stream is not dead, merely dormant: the next natural trigger (a single-file arrival, a
selection change, a workspace refresh) calls `request_pull_request_prefetch` again and the walk
resumes. The pre-stack version of this logic disabled background fill for the rest of the
session after a double failure, which the baseline analysis flagged; bounded retry with
re-triggering keeps transient Git failures from silently degrading the rest of a long reading
session.

**3. Truncated documents stay honestly incomplete.** A truncated per-file document is never
cached on disk (only complete patches enter the immutable per-file cache), is refused by count
backfill, and carries a visible trailing Meta row appended by the parser: "… diff truncated to
keep Quinjet responsive …". Every consumer therefore sees the same verdict, and a later
complete read of the same file can still upgrade it.

**4. Files past the API count horizon.** The pulls files endpoint is read for at most 64 pages
of 100 records, so a pull request past 6,400 files has a countless tail even though the index
itself holds up to 16,384 paths. Those files ride the pessimistic 512 KiB fallback estimate
(twelve to a batch) and resolve their headers through count backfill as their patches arrive.
The system degrades in estimate quality, never in correctness.

**5. The index truncates before the prefetch cap matters.** At 16,384 entries the name-status
parse stops and marks the index truncated; `total_files` then reports GitHub's `changedFiles`
figure so the tree's file count stays honest about what it is not showing. The prefetch cap of
4,096 is deliberately below the index cap: background fill covers what a reader will plausibly
browse, and everything else remains one selection away through the on-demand path.

**6. Binary files lose their label on the workspace path.** API-sourced counts hardcode
`binary: false` because the endpoint reports plain zeros for binary files, and count backfill
also writes `binary: false`. The "· binary" suffix in a file header, which derives from
`counts.binary`, therefore appears only where counts came from a local numstat. The review
recorded this as a known minor loss, accepted as part of the trade that removed the blob storm;
the diff body itself still renders Git's binary notice rows, so the information is delayed to
patch arrival rather than lost.

**7. Anchor arithmetic at the boundaries.** The anchor path is defensive at every step: a
viewport whose visible rows are all directories anchors on the first file below them; a
viewport scrolled past the last file (possible transiently, since #54's free scroll detaches
the window from the selection) yields no `File` entry and falls back to 0; an offset that
outlived a shrinking index is clamped with `.min(self.pull_request_files.len())` before
`split_at`, which would otherwise panic; and outside the Files section the anchor is defined to
be 0 rather than undefined. None of these edges can abort a batch; they only make its start
less clever.

**8. Replays and duplicates are absorbed, not amplified.** The batch merge skips paths already
in the document cache, backfill skips files with known counts and refuses truncated documents,
and cache insertion subtracts any prior entry before adding the new one. Any reply can
therefore be applied twice, or arrive after an equivalent single-file read already landed,
without corrupting counts, byte totals, or the document. In a system where the same file can be
requested by the preview lane and a batch simultaneously, idempotent merging is not a nicety;
it is what makes the two streams safe to run unsynchronized.

**9. The stages must not lie when the pull request is empty or fails.** A pull request with no
changed files renders the explicit empty document ("No file changes to display" or "This pull
request has no changed files") rather than an eternal stage 1, because the stage-1 message is
gated on work actually being in flight. A failed lookup surfaces an error card and a toast
rather than leaving skeleton rows up; the skeleton renders only while `pull_request_loading` is
true, and the failure path clears it. Loading states that outlive their loads were among the
review findings on the wider stack (a failed lookup once left "Fetching pull-request
metadata…" on screen indefinitely), and the merged code treats every loading indicator as
owned by exactly one in-flight operation.

## One batch end to end

The mechanisms above are each local; this section traces one healthy batch through all of them,
from scheduler to screen, naming every boundary it crosses. Suppose the reader has wheel-panned
the Files tree of bun#30412 so that `src/js_parser.zig` tops the viewport, the workspace
generation is 3, and none of the next files have patches yet.

**Step 1: the batch is planned.** A previous arrival calls `request_pull_request_prefetch`. The
anchor resolves to the index position of `src/js_parser.zig`; the walk chains from there and
wraps; eligible files accumulate under the 32-file and 6 MiB estimate limits. The scheduler
sets `pull_request_prefetching` and emits a single effect:

```text
WorkerCommand::LoadPullRequestFileBatch {
    workspace_generation: 3,
    paths: [src/js_parser.zig, src/js_lexer.zig, ..., docs/upgrading.md],
}
```

**Step 2: the worker routes, the session resolves.** The command lands in the mailbox's
`prefetch` slot, drains onto the `PullRequestPreview` lane behind any pending interactive
preview, and is executed through `cli::Session`, which resolves the prepared workspace by the
carried generation; a workspace that has since been dropped answers with a "workspace is no
longer available" error instead of touching anything (the session-ownership model is invariant
14, detailed in [the PR workspace page](../github/pr-workspace.md)).

**Step 3: cache partition, then one Git invocation.** `PreparedPullRequest::diff_files` checks
each path's immutable per-file cache entry (`pr-patch-v1\n{merge_base}\n{head}\n{path}`, 1 MiB
ceiling); hits skip Git entirely. The misses become one argv:

```bash
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 \
    <merge_base> <head> -- src/js_parser.zig src/js_lexer.zig ... docs/upgrading.md
```

run through the capped pipe machinery with an 8 MiB stdout limit. In the blob-less workspace
this is the moment lazy blob reads happen, resolving from the alternates link when the opened
repository has the bytes and from the promisor remote when it does not.

**Step 4: the combined patch is split at its section boundaries.** The output is one byte
buffer containing every file's patch back to back:

```text
diff --git a/src/js_parser.zig b/src/js_parser.zig   <- section 1 starts at offset 0
index 3f9c2a1..8b04e77 100644
--- a/src/js_parser.zig
+++ b/src/js_parser.zig
@@ -210,7 +210,9 @@ ...
 ...
diff --git a/src/js_lexer.zig b/src/js_lexer.zig     <- section 2 starts here
...
```

`split_patch_by_file` (`src/git/diff.rs:611-663`) scans for line starts matching
`diff --git `, `diff --cc `, or `diff --combined `, slicing the buffer into borrowed
`PatchSection { old_path, new_path, body }` values without copying; a section matches a
requested path when either its old or new path equals it, so renames answer under both names.
This split is what lets one Git process answer for 32 files while each file still becomes its
own document (invariant 10a; the format itself is dissected in
[the pipeline page](../diff/pipeline.md)).

**Step 5: each section parses into a document.** `pull_request_file_document` counts the raw
`+`/`-` lines, then `parse_diff` builds rows with two independent syntax-highlight states (old
and new side), under the 512 KiB per-patch and 32 KiB per-line grammar budgets described in
[the intraline and highlighting page](../diff/intraline-and-highlighting.md). Complete sections
under 1 MiB are written to the per-file disk cache on the way, so the next session's step 3
partition will classify them as hits.

**Step 6: the reply crosses back and merges.** The worker wraps the documents in
`WorkerEvent::PullRequestDiffBatch { workspace_generation: 3, result }`. The handler checks the
workspace generation, clears `pull_request_prefetching`, and merges: new documents enter the
byte-accounted cache, count backfill fills any `+·· -··` headers among them, `arrived_visible`
and `counts_changed` decide whether the combined document rebuilds, and
`document_with_visibility` reassembles it if so. `set_document` bumps the layout generation;
the next frame rebuilds its row list once and draws the new rows that fall inside the viewport.

**Step 7: the stream sustains itself.** The same handler's last act is to call
`request_pull_request_prefetch` again, which re-derives the anchor from wherever the viewport
is now and plans the next batch. Steps 1 through 7 repeat until the eligible set is empty or
the 4,096-file cap is reached, with the reader free to scroll, fold, and select throughout,
every interaction jumping the preview slot ahead of the stream.

## The single-file path beside the stream

Progressive fill runs beside, not instead of, the interactive path, and the two share their
caches. Selecting a file in the tree issues `WorkerCommand::LoadPullRequestFile` carrying both
tags (`generation` for the preview, `workspace_generation` for the workspace), visible at its
dispatch site (`src/app.rs:5858-5868`):

```rust
        self.diff_generation = self.diff_generation.wrapping_add(1);
        self.pull_request_loading_path = Some(path.clone());
        self.document_loading = show_loading;
        self.pull_request_progress = None;
        effects.push(AppEffect::Git(Box::new(
            WorkerCommand::LoadPullRequestFile {
                generation: self.diff_generation,
                workspace_generation,
                path,
            },
        )));
```

Setting `pull_request_loading_path` is what excludes the in-flight file from the next batch
plan (through `pull_request_file_needs_patch`), so the two lanes never race Git for the same
path from opposite directions. When the single-file document arrives, the handler's behavior
splits on the current file view (`src/app.rs:3283-3301`): in all-files view the document goes
into the shared cache and the combined document rebuilds around it; in single-file view the
previous document is first moved back into the cache under its path
(`cache_current_pull_request_single_document`), the new one takes the document slot, and
scrolls reset. The companion `take_pull_request_document` performs the reverse move with the
same byte accounting, so documents can shuttle between "the thing on screen" and "one entry in
the cache" indefinitely without the 32 MiB ledger drifting.

Two consequences make the shared cache the quiet workhorse of the whole design. Returning to
any file the stream or a previous selection already loaded is free, no Git, no parse, no
network, just a map move. And the single-file arrival handler ends the same way the batch
handler does, with `request_pull_request_prefetch(&mut effects)` (`src/app.rs:3305`): every
interactive load re-kindles the background stream if it had gone dormant, which is also the
recovery path noted under failure mode 2.

## Design alternatives that lost

The merged design is one point in a space the session explicitly explored. The alternatives
below were each considered, attempted, or shipped-then-replaced; recording why they lost is as
useful as recording what won.

**1. Load the whole pull request in one invocation.** The null alternative: run one
`git diff <merge_base> <head>` and parse it all. It fails on every axis at bun scale: the
combined patch dwarfs the 8 MiB read cap, so it would truncate arbitrarily; parsing it would
blow the in-memory budget in one allocation storm; and one uninterruptible child process would
occupy the lane for the whole duration, exactly the freeze the stack set out to remove. The cap
architecture (invariants 5 and 6) exists precisely to make this shape of work impossible to
express.

**2. Paginate the file list.** A "first 300 files, load more" tree would have bounded the
initial render without any skeleton machinery. It lost because the index is simply not the
expensive part: paths, statuses, and counts for 2,188 files cost one bounded read measured at
6.3 seconds cold and 0.04 seconds warm, while pagination would have taxed navigation forever
(fold state, search, and the wrap-around walk all want the whole index addressable). Quinjet
bounds the index at 16,384 entries and renders it through viewport virtualization instead,
which delivers the same bounded frame cost without making the reader page through their own
pull request. Invariant 10 encodes the decision: "there are no changed-file pages."

**3. One Git process per file.** The maximally incremental alternative: fetch each file's patch
in its own invocation, gaining perfect per-file addressing and trivially fine scheduling. The
doc comment on `diff_files` records why not: "Spawning one Git process per file dominates the
cost of a wide pull request, so batching is what lets the whole diff arrive while the reader is
still reading the first file." At 2,188 files, per-file spawning means 2,188 process startups,
repository opens, and object-store walks; batching 32 paths per invocation amortizes all three
while `split_patch_by_file` restores the per-file boundaries afterward.

**4. A persistent priority queue with in-flight cancellation.** A scheduler that maintains a
priority-ordered work queue, re-prioritizes on every scroll, and cancels in-flight batches
would react to retargeting a batch sooner. It lost to the stateless re-plan: deriving each
batch from current state gets retargeting within one batch boundary for free, has no queue to
keep consistent with the index, the fold state, and the eviction set, and needs no cancellation
protocol with the worker (whose lane discipline would make cancellation awkward anyway). The
in-flight batch is at most a second of already-spent work; letting it land and merge is cheaper
than any machinery to abort it.

**5. Smallest-first ordering.** Shipped as #50, replaced by #55, and analyzed in
[its own section](#from-smallest-first-to-viewport-first): right objective under a scarce
400-file cap, wrong objective once the cap covered the whole pull request.

**6. Fetching the head with blobs for mid-size pull requests.** Proposed during the session and
recorded as not built: skip partial-clone lazy fetching entirely for pull requests small enough
that eagerly downloading all blobs up front would be cheaper than many lazy round trips. The
alternates borrow took most of its value for the common squash-merge case (the blobs are
already local), and the size threshold where eager wins is workload-dependent enough that the
session deferred it rather than guessing.

**7. Detecting a missing-but-fetchable PR head and offering the fetch hint.** Also proposed and
not built: when the opened clone lacks `refs/pull/N/head` but could fetch it, surface the
one-time `git fetch` command that would make the fully local path apply. It remains a manual
remedy documented in the session notes; automating it would sit right at the edge of invariant
9's promise never to mutate the opened repository, so it stayed a suggestion rather than a
feature.

**8. Chunking the index past 16,384 entries.** Planned as work package 6 (a continuation-cursor
enumeration for pathological pull requests beyond the index cap) and consciously deferred:
bun#30412's 2,188 files did not need it, and the cap plus honest truncation handles the tail
today. The deferral is recorded so a future pull request that actually exceeds the cap finds
the design already sketched rather than a silent limit.

## Testing the progressive path

Progressive loading is guarded by tests at every layer, and the test names double as a summary
of the contracts this page has described. After #55's final commit the suite stood at "282
tests, clippy wall, comment check" in the session's gate log. The ones that pin this page's
behavior:

| Test | Location | Contract pinned |
| --- | --- | --- |
| `prefetch_starts_at_the_files_viewport_and_wraps_around` | `src/app.rs:8972-9011` | batch order is anchor-first with wrap-around (`c, d, a, b` under `sidebar_offset = 2`) |
| `lazy_index_keeps_all_headers_while_merging_one_loaded_file` | `src/git/diff.rs:987-1060` | skeletons, loaded-file splice, collapsed-state messages, count copy-back |
| `indexed_counts_render_before_any_patch_is_loaded` | `src/git/diff.rs:1122-1173` | known counts render with no placeholders before any patch exists |
| `indexed_totals_do_not_depend_on_loaded_or_visible_patches` | `src/git/diff.rs:1175-1219` | aggregate totals come from the index, not from materialized rows |
| `api_file_counts_parse_and_skip_malformed_records` | `src/git/github/mod.rs:3177-3203` | malformed and non-rename 0/0 records skipped, pure renames kept at 0/0 |
| `splits_a_batched_patch_into_one_section_per_file` | `src/git/diff.rs:1097-1120` | section boundaries and rename matching in the batch split |
| `bounded_runner_kills_oversized_git_output` | `src/git/github/mod.rs:3090-3105` | the capped pipe retains exactly the limit and kills the child |
| `locally_available_pr_objects_avoid_disposable_fetches` | `src/git/github/mod.rs:2946-2986` | locally present PR commits prepare with zero network reachability |
| `disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source` | `src/git/github/mod.rs:2989-3077` | full index, per-file batch answers, workspace cleanup, source repo untouched |
| `pull_request_file_tree_virtualizes_a_thousand_files` | `src/ui/mod.rs:7410-7456` | the tree renders a 1,000-file index through a viewport window |
| `sidebar_wheel_scroll_pans_without_moving_the_selection` | `src/app.rs:8880-8908` | free scroll pans without effects, feeding the prefetch anchor |

The pattern worth imitating: each mechanism's most surprising decision (the wrap-around order,
the collapsed-header count copy, the rename exemption, the exactly-at-limit pipe kill) has a
test whose name states the decision in prose, so the contract survives refactors that would
outlive any comment.

## Related pages

- [Rendering group hub](./README.md): how this page fits the rendering group.
- [Viewport rendering](./viewport.md): the frame-cost economics progressive loading depends on,
  including the row-layout cache the rebuilds flow through.
- [Concurrency](./concurrency.md): the worker, lanes, mailboxes, and generations end to end.
- [Prefetch](../github/prefetch.md): the mailbox slot and batching machinery shared with this
  page, from the GitHub-layer perspective.
- [PR workspace](../github/pr-workspace.md): the disposable bare workspace the batches run in.
- [API strategy](../github/api-strategy.md): the pulls files endpoint, the compare API, and the
  adaptive poll this page's gate modifies.
- [Caching](../github/caching.md): why every artifact in this pipeline caches immutably.
- [Diff pipeline](../diff/pipeline.md): patch bytes to document model, including the batch
  split.
- [Object model](../git-internals/object-model.md): content addressing, which underwrites both
  the immutable caches and the alternates borrow.
- [Shallow and partial clone](../git-internals/shallow-and-partial-clone.md): blob-less
  fetching and promisor lazy reads.
- [Merge bases and history](../git-internals/merge-bases-and-history.md): why a squash merge
  strands the PR head, and how the merge base is resolved.
- [Benchmarking](../benchmarking.md): the full bun#30412 methodology behind the measured
  results.
- [Techniques](../techniques.md): progressive loading, generation tagging, byte-budgeted
  batching, and viewport-scoped computation as general catalog entries.

## Planning a batch by hand

Start with the first visible file, rotate the bounded index at that anchor, and discard paths whose
patches are already loaded or already attempted. Price each remaining file as changed lines times
80 bytes plus 4,096 bytes of structural overhead, using 512 KiB when counts are unknown. Admit no
more than 32 paths and stop before adding a later path would cross six MiB. The first path always
travels even when its estimate exceeds the budget, which prevents an oversized file from blocking
the walk permanently.

## Launching straight into the stream

The `--pr` launch option changes initial focus, not the loading architecture. Repository identity
is still discovered on demand, metadata still establishes immutable endpoint OIDs, the workspace
still emits its bounded index before patches, and every reply still carries the generation that
requested it. Direct launch therefore removes navigation steps without creating a second eager
path or weakening stale-reply rejection.

## Optimization review matrix

Use this matrix during performance reviews. Each row combines a cost lens, repository context, and observable signal without claiming that every combination needs a standalone benchmark.

| ID | Review condition | Evidence to capture |
| ---: | --- | --- |
| 1 | Check latency for Progressive viewport-first loading in a small local repository | Record time to first useful rows |
| 2 | Check latency for Progressive viewport-first loading in a small local repository | Record steady frame cost |
| 3 | Check latency for Progressive viewport-first loading in a small local repository | Record bytes accepted from child output |
| 4 | Check latency for Progressive viewport-first loading in a small local repository | Record Git and gh process count |
| 5 | Check latency for Progressive viewport-first loading in a small local repository | Record maximum retained document bytes |
| 6 | Check latency for Progressive viewport-first loading in a small local repository | Record cache disposition and complete key |
| 7 | Check latency for Progressive viewport-first loading in a small local repository | Record stale reply rejection |
| 8 | Check latency for Progressive viewport-first loading in a small local repository | Record visible state after failure |
| 9 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record time to first useful rows |
| 10 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record steady frame cost |
| 11 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record bytes accepted from child output |
| 12 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record Git and gh process count |
| 13 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record maximum retained document bytes |
| 14 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record cache disposition and complete key |
| 15 | Check latency for Progressive viewport-first loading in a monorepo with many changed paths | Record stale reply rejection |
