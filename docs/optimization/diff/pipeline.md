# The diff pipeline: from patch bytes to the document model

This page follows a diff through Quinjet end to end: the exact bytes Git writes when asked for a
unified patch, the document model in `src/git/diff.rs` that those bytes become, the three bounded
reads that build a local diff before any patch exists, the parser that turns one patch into rows,
the merge step that assembles collapsed headers and loaded bodies into a single scrollable
document, the splitter that lets one Git invocation answer for a whole batch of files, the
synthesizer that fakes a patch for files Git has never seen, and every cap that keeps the pipeline
bounded no matter how large the repository or the pull request. The companion pages cover what
happens on either side of this pipeline: [diff algorithms](./algorithms.md) explains how Git
computes the hunks Quinjet consumes, and
[intraline emphasis and highlighting](./intraline-and-highlighting.md) explains what happens to
the rows once they exist.

## Contents

- [Pipeline overview](#pipeline-overview)
- [The unified diff format on the wire](#the-unified-diff-format-on-the-wire)
- [The document model](#the-document-model)
- [The three-read local diff](#the-three-read-local-diff)
- [Parsing patch bytes with parse_diff](#parsing-patch-bytes-with-parse_diff)
- [Merging skeleton and bodies with document_with_visibility](#merging-skeleton-and-bodies-with-document_with_visibility)
- [Splitting batched patches with split_patch_by_file](#splitting-batched-patches-with-split_patch_by_file)
- [Untracked-file patch synthesis](#untracked-file-patch-synthesis)
- [Caps and budgets end to end](#caps-and-budgets-end-to-end)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [The behavioral contract in tests](#the-behavioral-contract-in-tests)
- [Where to go next](#where-to-go-next)

## Pipeline overview

Quinjet never computes a diff itself. Git is the authority for hunk computation, rename
detection, and context selection; Quinjet's job is to ask Git the smallest possible questions,
read the answers through capped pipes, and turn the resulting bytes into an in-memory document
the renderer can draw one viewport at a time. The pipeline has five stages, and each stage exists
to keep some cost bounded:

1. Index first. Before any patch is read, a diff source (working tree, commit, branch
   comparison, stash, or pull request) is reduced to a `DiffIndex`: a title, a list of
   `DiffFileIndexEntry` records, and a truncation flag. The index comes from
   `git diff --name-status -z` plus a `--numstat` pass over the same range, so every file header
   can render its path, status, and real `+n -n` totals before a single patch byte exists.
2. Patch on demand. A file's patch is produced only when its body must be shown or when the
   background prefetcher decides to warm it. Each read is one path-scoped `git diff` capped at
   8 MiB, parsed by `parse_diff` into a per-file `DiffDocument`.
3. Batch when many are needed. For pull requests, the prefetcher requests up to 32 files in one
   Git invocation; `split_patch_by_file` cuts the combined output back apart at its
   `diff --git` boundaries so each file still parses and caches as its own document.
4. Merge for display. `DiffIndex::document_with_visibility` assembles the on-screen document
   from the index and whatever per-file documents have loaded: loaded visible files contribute
   their full rows, everything else contributes a header plus a one-line placeholder.
5. Evict under a budget. Parsed pull-request documents live in an in-memory cache bounded to
   32 MiB of estimated retained size, evicted oldest-first, with the guarantee that the newest
   document always survives.

The stages map onto two responsiveness invariants from `ARCHITECTURE.md`. Invariant 8:

> Working-tree groups, commits, branch comparisons, and stashes use one coalesced local
> workspace. A bounded name/status index produces collapsed headers first; the first path is
> prefetched silently, and later expansions run path-scoped Git commands whose documents remain
> cached for that workspace. Periodic status snapshots do not rebuild an unchanged comparison.

And invariant 8a, which is the reason the pipeline reads counts separately from patches:

> Every index also reads `git diff --numstat` over the same range, so a header shows its real
> `+n -n` before that file has a patch. A file's totals never depend on whether its patch has
> loaded.

Everything below is the machinery that makes those two sentences true.

## The unified diff format on the wire

Quinjet's parser consumes the unified diff format exactly as `git diff --patch` emits it, so the
first thing worth knowing is what those bytes look like. The format is line-oriented: every
record is one line terminated by `\n`, and the meaning of a line is determined entirely by its
prefix. That prefix-dispatch property is what makes a single forward pass sufficient to parse an
arbitrarily large patch.

### Origins and general shape

The unified format predates Git. It comes from GNU diff's `-u` mode, which replaced the older
"context format" by interleaving removed and added lines in one block instead of printing the old
and new regions separately. Git adopted the format for `git diff`, `git show`, and
`git format-patch`, and extended it with a per-file header block (the `diff --git` line and the
extended header lines after it) that carries Git-specific metadata: blob object names, file
modes, rename and copy detection results, and binary-file notices. The authoritative reference
for the Git extensions is the [git-diff manual page](https://git-scm.com/docs/git-diff).

A multi-file patch is a concatenation of per-file sections. Each section has this shape, in this
order, with every part after the first optional:

```text
diff --git a/<old path> b/<new path>      one line, starts the section
<extended header lines>                    zero or more: modes, index, renames, similarity
--- <old file label>                       old side label, or /dev/null
+++ <new file label>                       new side label, or /dev/null
@@ -<old start>,<old count> +<new start>,<new count> @@ <heading>
<hunk body lines>                          prefixed with ' ', '+', '-', or '\'
@@ ... @@                                  further hunks, each with its body
```

Three structural facts follow from this shape, and all three are load-bearing for Quinjet:

**1. Sections are self-delimiting.** A new section begins exactly where a line starting with
`diff --git` (or `diff --cc` / `diff --combined` for merges) begins. Nothing inside a hunk
body can be confused with that prefix, because hunk body lines always start with a space, `+`,
`-`, or `\`. This is what makes `split_patch_by_file` in `src/git/diff.rs` correct: it can cut a
combined multi-file patch at those line starts without understanding anything else about the
content.

**2. Lines are cheap to classify.** Every line's role is decided by at most the first few bytes.
`parse_diff` is a single loop whose body is an ordered chain of `strip_prefix` and `starts_with`
tests; there is no lookahead, no backtracking, and no grammar beyond the prefix table.

**3. Truncation at a line boundary is safe.** Because no record spans lines, a patch cut at any
`\n` is a valid (shorter) patch. Quinjet exploits this every time a capped read kills Git
mid-stream: `truncate_to_complete_line` in `src/git/mod.rs` pops bytes until the buffer ends
with `\n`, and the parser never sees a half record.

### The file header line

The section opener names both sides of the file:

```text
diff --git a/src/main.rs b/src/main.rs
```

The `a/` and `b/` prefixes are conventional labels for the pre-image and post-image trees. For a
rename the two paths differ; for an ordinary edit they are equal. The line is emitted even for
files with no textual hunks (pure mode changes, pure renames, binary files), which is why
Quinjet's parser starts a new `FileBuilder` on this line rather than on the first hunk.

Two details complicate parsing this line in the general case:

- A path may contain spaces. `diff --git a/old name.rs b/new name.rs` is legal, and nothing
  escapes the interior space. Quinjet resolves the ambiguity the same way Git's own tools do:
  `diff_header_paths` in `src/git/diff.rs` splits at the last occurrence of `" b/"` (or
  `" "b/` for a quoted new path), because the new path cannot itself contain that separator
  sequence at its own boundary.
- A path with bytes outside the printable ASCII range is C-quoted by default: wrapped in double
  quotes with backslash escapes. Quinjet runs every Git command with `-c core.quotepath=false`,
  which makes Git emit raw bytes instead, but quoting can still appear (a path containing an
  actual double quote or backslash is quoted regardless), so the decoder handles both forms.

Here is the splitting function, from `src/git/diff.rs`:

```rust
fn diff_header_paths(header: &str) -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(separator) = header.rfind(" b/").or_else(|| header.rfind(" \"b/")) else {
        return (None, None);
    };
    let old = patch_path(header.get(..separator).unwrap_or_default(), "a/");
    let new = patch_path(header.get(separator + 1..).unwrap_or_default(), "b/");
    (old, new)
}
```

The `rfind` is the important choice: searching from the right means an old path that happens to
contain the substring `" b/"` (for example `a/misc b/old.txt`) still splits at the true
boundary, because the true `b/` side is always the rightmost occurrence.

### Extended header lines

Between the `diff --git` line and the `---` label, Git emits zero or more extended header lines.
Each is a fixed keyword prefix followed by a value:

| Header | Example | Meaning |
| --- | --- | --- |
| `old mode` | `old mode 100644` | Pre-image file mode when the mode changed |
| `new mode` | `new mode 100755` | Post-image file mode when the mode changed |
| `new file mode` | `new file mode 100644` | The file was created; mode of the new file |
| `deleted file mode` | `deleted file mode 100644` | The file was deleted; mode it had |
| `copy from` / `copy to` | `copy from lib.rs` | Copy detection result (requires `-C`) |
| `rename from` | `rename from old name.rs` | Rename detection: pre-image path |
| `rename to` | `rename to new name.rs` | Rename detection: post-image path |
| `similarity index` | `similarity index 90%` | Content similarity that justified the rename |
| `dissimilarity index` | `dissimilarity index 40%` | Rewrite score (requires `-B`) |
| `index` | `index 3f9c1a2..b7d20e4 100644` | Pre and post blob object names, shared mode |

The `index` line deserves a note because it connects the patch to the
[object model](../git-internals/object-model.md): the two abbreviated hashes are the blob OIDs
of the file's old and new contents. A tool that wanted to verify or reapply the patch could use
them; a viewer cannot do anything with them. Quinjet drops `index` and `similarity index` lines
on the floor as transport noise, keeps `old mode` and `new mode` lines as visible meta rows
(a mode flip is user-relevant), and folds `rename from` / `rename to` into the file's paths and
status instead of showing them as rows. From `src/git/diff.rs`:

```rust
if raw_line.starts_with("index ") || raw_line.starts_with("similarity index ") {
    continue;
}
if raw_line.starts_with("new file mode ") {
    file_mut(&mut current_file, path_hint).status = Some("added");
    continue;
}
if raw_line.starts_with("deleted file mode ") {
    file_mut(&mut current_file, path_hint).status = Some("deleted");
    continue;
}
```

Because Quinjet passes `--find-renames` but not `-C`, `copy from` / `copy to` headers do not
occur in the patches it generates for itself; if a foreign patch contained them they would fall
through to the parser's final arm and render as plain meta rows, which is a graceful degradation
rather than a failure.

### The old and new file labels

The `---` and `+++` lines repeat the two sides with their tree prefixes:

```text
--- a/src/main.rs
+++ b/src/main.rs
```

Creation and deletion are encoded with the special label `/dev/null`:

```text
--- /dev/null
+++ b/docs/new-page.md
```

`patch_path` in `src/git/diff.rs` maps `/dev/null` to `None`, trims a trailing tab (Git appends
one when a path ends in whitespace, so the true end of the path is unambiguous), decodes
quoting, and strips the `a/` or `b/` prefix:

```rust
fn patch_path(value: &str, prefix: &str) -> Option<PathBuf> {
    let value = value.trim_end_matches('\t');
    if value == "/dev/null" {
        return None;
    }
    let decoded = decode_git_path(value);
    let path = decoded.strip_prefix(prefix).unwrap_or(&decoded);
    Some(PathBuf::from(path))
}
```

The `+++` line is also the parser's last chance to learn the file's real path before content
arrives, so it is where Quinjet re-derives the path used to choose a syntax-highlighting
grammar. The `---` and `+++` labels win over the `diff --git` header when they disagree, because
they are emitted per side and are not subject to the space-splitting ambiguity.

### Hunk headers

Each hunk starts with a range header:

```text
@@ -10,3 +10,4 @@ fn main() {
```

The grammar of the two range fields:

| Field | Form | Meaning |
| --- | --- | --- |
| `-10,3` | `-<start>,<count>` | Old file: the hunk covers 3 lines starting at line 10 |
| `+10,4` | `+<start>,<count>` | New file: the hunk covers 4 lines starting at line 10 |

Two format wrinkles matter for a correct parser:

- When a count is exactly 1 it may be omitted: `@@ -5 +5 @@` means one line on each side.
- When a side is empty (a pure insertion into an empty region, or a file creation), the start is
  the line before the insertion point and the count is 0: `@@ -0,0 +1,7 @@` is the canonical
  header for a new 7-line file.

The text after the closing `@@` is the "function context" heading: the nearest preceding line
that matches Git's per-language `xfuncname` pattern. It is a reading aid, not data; Quinjet
keeps the whole header line as a `HunkHeader` row and separately extracts only the two start
numbers to seed its line counters. From `src/git/diff.rs`:

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

fn parse_range_start(field: &str, prefix: char) -> Option<usize> {
    field.strip_prefix(prefix)?.split(',').next()?.parse().ok()
}
```

Whitespace splitting plus "take the part before the first comma" handles both the full and the
count-omitted forms in four lines. The counts themselves are deliberately ignored: Quinjet does
not need them because it tracks line numbers by walking the body, and ignoring them makes the
parser robust against patches whose counts are wrong (hand-edited patches are a real input via
the clipboard-adjacent paths that feed `parse_diff` a buffer and a path hint).

### Line prefixes inside a hunk

Every line of a hunk body begins with a one-byte marker:

| First byte | Role | Old line number | New line number |
| --- | --- | --- | --- |
| space | Context line, present on both sides | advances | advances |
| `-` | Removed line, present only in the old file | advances | unchanged |
| `+` | Added line, present only in the new file | unchanged | advances |
| `\` | Metadata about the preceding line | unchanged | unchanged |

The marker is part of the line, not a separator: the content of `+new();` is `new();`, obtained
by stripping exactly one byte. A context line representing an empty source line is a single
space character; some non-Git tools emit a truly empty line instead, and Quinjet's parser
tolerates that by simply dropping empty lines that match no other rule.

The ordering convention inside a hunk is that a run of `-` lines followed by a run of `+` lines
represents a replacement; Git never interleaves them within one logical change. Quinjet's
intraline emphasis leans on that convention to pair removed and added lines positionally, which
is covered in [intraline emphasis and highlighting](./intraline-and-highlighting.md).

### The missing newline marker

A file that does not end with a newline cannot be represented naively, because the format is
line-oriented. The escape hatch is a pseudo-line immediately after the affected line:

```text
\ No newline at end of file
```

It starts with a backslash and a space, so it cannot collide with any content prefix. If the old
file lacked a trailing newline and the new one has it, the marker follows the last `-` line; the
reverse follows the last `+` line; if both lack it, it appears after both. Quinjet does not
special-case the marker: it starts with `\`, matches none of the parser's prefix rules, and
lands in the fallback arm that turns any unrecognized non-empty line into a `Meta` row, which is
exactly the right rendering for it. The same marker is emitted by Quinjet's own untracked-file
synthesizer when the file on disk lacks a final newline, so the synthetic patch round-trips
through the same parser arm.

### Combined diff headers

Merge conflicts have more than one pre-image, and Git represents their diffs in a combined
format that starts with a different header:

```text
diff --cc src/app.rs
```

(or `diff --combined` for more than two parents). Combined hunks use `@@@` markers with one
range per parent, and body lines carry one marker column per parent, so `++` means "added
relative to both parents" and `+` with a trailing space means "added relative to the first parent only". Quinjet
requests this format in exactly one place: `raw_diff_for_change` in `src/git/mod.rs` passes
`--cc` when a change sits in the conflict area, so a merge-conflicted file previews as its
combined diff.

The parser handles combined output with deliberately single-column semantics. The `diff --cc`
and `diff --combined` prefixes are recognized as section starts (both in `parse_diff` and in
`split_patch_by_file`), and the single path they carry serves as both the old and the new path.
An `@@@` hunk header still matches the `@@` prefix test and renders as a hunk-header row, but
its second field is another `-` range, so the new-side counter is not seeded and the extra
marker column stays visible inside the content. That is a conscious trade: conflict previews are
short-lived working states, and rendering the combined markers verbatim is more honest than
pretending a two-parent diff is a two-column diff.

### Binary file notices

When either side of a file is binary (or when `core.bigFileThreshold` style heuristics decide
text processing is off), Git emits no hunks. With default options the section body is a single
notice line:

```text
Binary files a/assets/logo.png and b/assets/logo.png differ
```

With `--binary` it is a literal `GIT binary patch` block containing base85-encoded delta data.
Quinjet never passes `--binary`, but it recognizes both forms. From `src/git/diff.rs`:

```rust
if raw_line.starts_with("Binary files ") || raw_line == "GIT binary patch" {
    let file = file_mut(&mut current_file, path_hint);
    file.binary = true;
    file.lines.push(meta_line(DiffLineKind::Meta, raw_line));
    continue;
}
if current_file.as_ref().is_some_and(|file| file.binary) {
    file_mut(&mut current_file, path_hint)
        .lines
        .push(meta_line(DiffLineKind::Meta, raw_line));
    continue;
}
```

The second arm is a containment rule: once a file is marked binary, every subsequent line of
that section becomes a meta row, so even if a base85 payload did arrive it could never be
misread as hunks. The index side has its own binary signal, described with `parse_numstat`
below, so a binary file's header can say `binary` before any patch is read.

### A worked example patch, byte by byte

The unit test `parses_hunks_and_tracks_line_numbers` in `src/git/diff.rs` feeds the parser this
154-byte patch (shown here with visible line breaks):

```diff
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,3 +10,4 @@ fn main() {
 let value = 1;
-old();
+new();
+more();
 end();
```

Its exact byte layout:

| Offset | Length | Bytes (as text) | Role |
| --- | --- | --- | --- |
| 0 | 39 | `diff --git a/src/main.rs b/src/main.rs` + `\n` | Section start, both paths |
| 39 | 18 | `--- a/src/main.rs` + `\n` | Old label |
| 57 | 18 | `+++ b/src/main.rs` + `\n` | New label |
| 75 | 30 | `@@ -10,3 +10,4 @@ fn main() {` + `\n` | Hunk header, seeds counters 10/10 |
| 105 | 16 | space + `let value = 1;` + `\n` | Context, old 10 / new 10 |
| 121 | 8 | `-old();` + `\n` | Removed, old 11 |
| 129 | 8 | `+new();` + `\n` | Added, new 11 |
| 137 | 9 | `+more();` + `\n` | Added, new 12 |
| 146 | 8 | space + `end();` + `\n` | Context, old 12 / new 13 |

Walking the counters makes the numbering rule concrete. The hunk header sets `old_line = 10` and
`new_line = 10`. The context line is stamped `(10, 10)` and advances both counters to 11. The
removed line is stamped old 11, advancing only the old counter to 12. The two added lines are
stamped new 11 and new 12, advancing only the new counter to 13. The final context line is
stamped `(12, 13)`. The test pins exactly those five pairs, and they are what the renderer
prints in the gutter: two 4-column line numbers per row, an old one and a new one, with the
absent side blank for added and removed rows.

Note what is absent from the parsed output: the `diff --git` line, the `---` and `+++` labels.
They are steering metadata, consumed to configure the file section and then dropped, which the
test `highlights_typescript_and_hides_git_transport_headers` pins by asserting a one-hunk patch
produces exactly five rows: file header, hunk header, removed, added, file footer.

## The document model

Everything the diff pane draws is a `DiffDocument`, and everything a `DiffDocument` contains is a
flat `Vec<DiffLine>`. There is no tree of files containing hunks containing lines; there is one
row list, in display order, where file boundaries and hunk boundaries are themselves rows. That
flatness is a rendering decision as much as a modeling one: the viewport renderer in
[rendering/viewport](../rendering/viewport.md) scrolls a row list, hit-tests a row list, and
slices a row list, and a flat vector makes every one of those operations an index computation
instead of a tree walk.

### DiffLineKind

The row vocabulary is a seven-variant enum at the top of `src/git/diff.rs`:

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

Each variant carries a distinct rendering and behavioral contract:

- `FileHeader` is the anchored, clickable row that names a file, shows its status, and carries
  its `+n -n` counts. It is the fold toggle target and the sticky header the renderer pins to
  the top of the pane while the file's body scrolls under it.
- `FileFooter` is an empty spacer row closing a file section. It exists so collapsed and
  expanded files have symmetric boundaries and so the side-by-side layout can end a file's
  two-column region cleanly.
- `HunkHeader` preserves the `@@` line. It is stored but not displayed: both the unified and the
  side-by-side row builders skip `HunkHeader` rows when producing visual rows, because the
  gutter line numbers already communicate position and the raw range text is noise on screen.
  Keeping the row in the document anyway means the information is never lost and serialized
  documents (the `--json` output of the diff subcommands) remain complete.
- `Context`, `Added`, and `Removed` are the content rows. They are the only kinds with line
  numbers, and the only kinds eligible for intraline emphasis.
- `Meta` is everything else worth showing: mode-change lines, binary notices, the missing
  newline marker, placeholder messages in skeleton documents, and truncation notices.

The enum is `Copy` and seven variants fit in one byte, so kind checks in render loops are free.
The serde attribute matters because documents cross the process boundary: the CLI subcommands
(`quinjet pr diff --json` and friends) serialize the same structs the terminal renders, so the
row kinds appear as `"file-header"`, `"hunk-header"`, and so on in machine-readable output.

### HighlightSpan and DiffLine

A row's text is not a `String`; it is a vector of styled spans:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HighlightSpan {
    pub text: String,
    pub foreground: Option<SyntaxColor>,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub spans: Vec<HighlightSpan>,
}
```

Two design choices here carry most of the weight.

**1. Foreground is an optional semantic color, not an RGB value.** `SyntaxColor` (defined in
`src/theme.rs`) is a small enum of semantic roles: text, comment, red, orange, yellow, green,
cyan, blue, purple, brown. The parser maps syntect's concrete base16-ocean RGB output into those
roles at parse time, and the renderer maps roles back to whatever the active theme says at draw
time. A `None` foreground means "use the diff-kind default at render time": added rows get the
added color, removed rows the removed color, and so on. The consequence is that a parsed
document is theme-independent: switching between the light and dark palettes recolors every
cached document instantly, with zero re-parsing, because no concrete color was ever baked into
the stored spans.

**2. Line numbers are per-side options, not a single index.** `old_line` and `new_line` are
1-based positions in the old and new files respectively. A context row has both, an added row
has only `new_line`, a removed row only `old_line`, and header, footer, hunk, and meta rows have
neither. This is precisely the information the two-column gutter needs, and it is also what
lets the side-by-side layout pair rows without re-deriving anything: a `Split` row references an
old-side row and a new-side row by document index, and each side prints its own number.

`DiffLine::text()` reassembles a row's plain text, with one subtlety:

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

File headers join their spans with single spaces because a header's three spans are separate
fields (the label, the `+n` string, the `-n` string) that the renderer positions independently;
content rows concatenate because their spans are contiguous slices of one source line, split
only where the highlighter changed style. The distinction shows up in tests as headers reading
`docs/two.md  · added +1 -0` while a highlighted code row reads back as its exact source text.

### DiffLineCounts and DiffFileIndexEntry

The index side of the model is two small structs:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffLineCounts {
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffFileIndexEntry {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: String,
    /// Exact per-file totals read from `git diff --numstat` while the index is
    /// built. Known counts let a file header render its real `+n -n` before the
    /// patch for that file has been produced.
    pub counts: Option<DiffLineCounts>,
}
```

An entry is a file's identity in a diff: its post-image path (the key everything else uses), its
pre-image path when the file was renamed or copied, a lowercase human status label ("added",
"modified", "renamed", "type changed", ...), and optionally its exact line totals. The doc
comment on `counts` states the purpose plainly, and the `Option` is honest: counts come from a
separate best-effort read, so a file can exist in the index without them, and the UI must be
able to say so.

Two methods turn an entry into header content. `label()` builds the display string:

```rust
fn label(&self) -> String {
    let mut label = self.path.display().to_string();
    if let Some(old_path) = self.old_path.as_ref().filter(|old| *old != &self.path) {
        label.push_str("  · renamed from ");
        label.push_str(&old_path.display().to_string());
    } else if !self.status.is_empty() {
        label.push_str("  · ");
        label.push_str(&self.status);
    }
    if self.counts.is_some_and(|counts| counts.binary) {
        label.push_str("  · binary");
    }
    label
}
```

A rename annotation wins over the plain status (saying both "renamed from x" and "renamed" would
be redundant), and the binary marker is appended independently, so a renamed binary file reads
`assets/logo.png  · renamed from old/logo.png  · binary`. `count_spans()` produces the two count
strings, and it is where the placeholder glyphs live:

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

The pair `+··` / `-··` (two middle dots per side) is the visual signal for "totals not yet
known". It appears only when a file has no numstat counts and no loaded patch, and it is
distinct from `+0 -0` because zero is a real answer (a pure rename genuinely changes zero
lines) while the dots are the absence of an answer. The merge step described later replaces the
placeholders the moment better information exists, from either direction: numstat counts fill
them at index time, and a loaded patch's real header strings overwrite them at merge time.

### DiffIndex

The index is the bounded file list plus its framing:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffIndex {
    pub title: String,
    pub files: Vec<DiffFileIndexEntry>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
}
```

`truncated` records that the file list itself is incomplete: the name-status read hit its 8 MiB
cap or the 16,384-entry ceiling, so files beyond the cut simply are not listed. That flag is
sticky through every downstream document so the UI can always say the view is partial.

`line_counts()` folds the per-file counts into a whole-diff total:

```rust
pub(crate) fn line_counts(&self) -> DiffLineCounts {
    self.files.iter().filter_map(|file| file.counts).fold(
        DiffLineCounts::default(),
        |total, counts| DiffLineCounts {
            additions: total.additions.saturating_add(counts.additions),
            deletions: total.deletions.saturating_add(counts.deletions),
            binary: total.binary || counts.binary,
        },
    )
}
```

Files with `counts: None` are skipped rather than estimated, so the totals a diff title shows
come purely from numstat data and never from loaded patches. The test
`indexed_totals_do_not_depend_on_loaded_or_visible_patches` makes the separation explicit: an
index whose files carry counts of 12/2 and 3/7 reports `line_counts()` of 15 additions and 9
deletions while the fully collapsed document built from the same index reports an
`addition_count()` of 0, because no `Added` rows were materialized. Two different questions, two
different answers, both correct.

### DiffDocument and its counters

The document is the index's rendered counterpart:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffDocument {
    pub title: String,
    pub lines: Vec<DiffLine>,
    pub truncated: bool,
    pub commit_details: Option<CommitDetails>,
    pub pull_request_details: Option<PullRequestDetails>,
}
```

`DiffDocument::empty(title, message)` builds the degenerate one-row document used for every
"nothing to show" state: an empty diff, an error placeholder, a loading placeholder. Uniformity
here is worth more than it looks: because even the empty states are ordinary documents with one
`Meta` row, the renderer has no special cases, and every state transition is "replace the
document", which is the single choke point the layout cache invalidation hangs off.

The three counters walk the row list:

```rust
pub(crate) fn file_count(&self) -> usize {
    self.lines
        .iter()
        .filter(|line| line.kind == DiffLineKind::FileHeader)
        .count()
}
```

with `addition_count()` and `deletion_count()` identical over `Added` and `Removed`. These
deliberately count materialized rows, not logical changes. A collapsed file contributes its
header and nothing else; an unloaded file contributes a header and a placeholder meta row. The
counters therefore answer "what is in this document" rather than "what is in this diff", and the
latter question is always answered by `DiffIndex::line_counts()`. Keeping the two questions on
two types is what lets a skeleton document render instantly with honest totals in its title
while its body is still one placeholder per file.

### CommitDetails and PullRequestDetails

Two optional attachments ride along with a document so the header area above the rows can be
drawn without consulting any other state.

`CommitDetails` carries the identity of a commit-sourced diff: id, subject, author and committer
names, emails, and both timestamps, all as the strings Git printed. It is populated by
`commit_details` in `src/git/mod.rs` from the already-parsed `Commit` that the history pane
holds, so previewing a commit costs no extra Git invocation for its metadata.

`PullRequestDetails` is the pull-request equivalent and considerably wider: number, title,
description, author, state, draft flag, update timestamp, URL, base and head repository labels
(with enterprise hosts prefixed onto the head repository name so a fork on another GitHub host
is never mistaken for a github.com repository), remote lists for both sides, the
cross-repository flag, the PR-level `changed_files` / `additions` / `deletions` totals from
GitHub metadata, and, for single-file documents, the selected file's path and its own counts.
The per-file counts in that last group are computed by scanning the raw patch bytes with
`count_patch_lines` in `src/git/github/mod.rs`:

```rust
fn count_patch_lines(output: &[u8], marker: u8) -> usize {
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| {
            line.first() == Some(&marker)
                && !line.starts_with(if marker == b'+' { b"+++ " } else { b"--- " })
        })
        .count()
}
```

The exclusion of `+++` and `---` label lines is the classic unified-diff counting bug turned
into two explicit conditions: the labels start with the same marker bytes as content lines, and
a naive prefix count would credit every file with one phantom addition and one phantom deletion.

The document model, summarized as a data-flow contract: `DiffFileIndexEntry` is what exists
before patches, `DiffLine` is what a patch becomes, `DiffIndex` answers questions about the
whole diff, `DiffDocument` answers questions about what is on screen, and the two attachment
structs carry the surrounding metadata so a document is self-sufficient for rendering.

## The three-read local diff

Opening any local diff source in Quinjet triggers at most three kinds of Git reads, in a strict
order: a name-status listing, a numstat listing over the same range, and then per-file patch
reads only as files actually need bodies. This section walks each read, its exact argv, its
parser, and its failure behavior. The general plumbing conventions these commands rely on
(NUL-terminated output, `LC_ALL=C`, argv-direct spawning, `GIT_OPTIONAL_LOCKS=0`) are covered in
[plumbing and porcelain](../git-internals/plumbing-and-porcelain.md); this page takes them as
given and focuses on the diff-specific parts.

### Why an index comes before any patch

The naive way to preview a commit is `git show <id>` and render the output. For a small commit
that works; for a 2,000-file merge it means waiting for Git to compute and emit every hunk of
every file before the first pixel, holding the entire patch in memory, and re-highlighting all
of it. The measured pathology that drove the optimization stack was exactly this shape: the Bun
rewrite pull request (oven-sh/bun#30412) weighs in at 2,188 changed files and over a million
added lines, and no single-read design survives contact with it.

The index-first design inverts the cost. A `--name-status` listing is tiny (a status byte and a
path per file, no content), so it arrives nearly instantly at any file count that fits the
16,384-entry cap. That listing is enough to render the complete file tree and a collapsed
document with every header present. The expensive part, patch content, then loads per file: on
demand for the file the reader expands, and in the background for everything else. The reader's
time-to-first-content stops depending on the size of the whole diff.

The workspace that holds this state is `PreparedLocalDiff` in `src/git/mod.rs`:

```rust
pub(crate) fn prepare_local_diff(
    &self,
    request: &LocalDiffRequest,
) -> Result<PreparedLocalDiff> {
    let index = self.local_diff_index(request)?;
    Ok(PreparedLocalDiff {
        repository: self.clone_for_worker(),
        request: request.clone(),
        index,
    })
}
```

The prepared workspace captures the repository handle, the original request, and the built
index. Every later per-file read resolves against that same captured request and file list, so
a workspace answers consistently even if the repository moves on underneath it; a new request
builds a new workspace under a new generation, and the session-level workspace matching
described in [rendering/concurrency](../rendering/concurrency.md) guarantees a file read can
never cross into a workspace it was not prepared for.

`LocalDiffRequest` names the four local sources, and each carries an `expanded` flag that
selects the context width for every later patch read:

```rust
pub(crate) enum LocalDiffRequest {
    Changes { changes: Vec<Change>, version: u64, expanded: bool },
    Commit { commit: Box<Commit>, expanded: bool },
    Branch { branch: Box<HistoryBranch>, current: String, current_oid: Option<String>, expanded: bool },
    Stash { stash: Box<Stash>, expanded: bool },
}
```

`expanded` maps to `--unified=1000000` versus `--unified=3`: a million lines of context is
effectively "show the whole file around every hunk", which is how the expanded view shows full
files without any separate file-content read.

### Read one, the name-status listing

For revision-based sources the index listing argv is built by `diff_index_args` in
`src/git/mod.rs`:

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

Flag by flag:

- `--name-status` emits one status record and one or two path records per file, no content.
- `-z` terminates records with NUL instead of newline and disables path quoting, so a path
  containing a newline, a tab, or any other hostile byte parses unambiguously.
- `--find-renames` turns on rename detection, which changes the record shape for renamed files
  (an `R<score>` status followed by two path records) and is what makes `old_path` populated.
- The trailing `--` closes the revision list so nothing that follows could be parsed as an
  option, part of the argv hygiene discipline applied to every Git invocation.

Each source variant instantiates the read differently. A commit with a parent diffs
`parent..commit`; a root commit has no parent, so `local_diff_index` switches to
`diff-tree --root --no-commit-id --name-status -z -r --find-renames <id> --`, which diffs the
commit against the empty tree. A branch comparison first validates that the reference starts
with `refs/heads/` or `refs/remotes/` and then diffs `reference..HEAD`. A stash uses
`stash show --name-status -z --include-untracked <ref> --`, where `--include-untracked` folds
the stash's third-parent untracked commit into the listing. The working-tree variant is special:
it runs no listing at all, because the status snapshot the app already holds is the listing, and
its entries convert directly into `DiffFileIndexEntry` values.

The record stream for a mixed diff looks like this (NUL bytes shown as `␀`):

```text
M␀src/app.rs␀A␀docs/new.md␀R087␀old/name.rs␀new/name.rs␀D␀gone.rs␀
```

`diff_index_files` walks it with a cursor:

```rust
let status_code = status.first().copied().unwrap_or_default();
let rename_or_copy = matches!(status_code, b'R' | b'C');
let Some(first_path) = records.get(cursor) else {
    truncated = true;
    break;
};
cursor += 1;
let first_path = PathBuf::from(String::from_utf8_lossy(first_path).into_owned());
let (old_path, path) = if rename_or_copy {
    let Some(new_path) = records.get(cursor) else {
        truncated = true;
        break;
    };
    cursor += 1;
    (
        Some(first_path),
        PathBuf::from(String::from_utf8_lossy(new_path).into_owned()),
    )
} else {
    (None, first_path)
};
```

Only the first byte of the status record matters (`R087` matches on `R`; the score digits are
ignored), and rename or copy records consume two path records: pre-image first, post-image
second, keyed by the post-image. A record run that ends mid-file (possible when the read was
capped) sets `truncated` and stops rather than guessing. The status byte becomes a human label
through a total mapping:

```rust
const fn diff_status_label(status: u8) -> &'static str {
    match status {
        b'A' => "added",
        b'M' => "modified",
        b'D' => "deleted",
        b'R' => "renamed",
        b'C' => "copied",
        b'T' => "type changed",
        b'U' => "unmerged",
        _ => "changed",
    }
}
```

The catch-all arm means an unrecognized future status code degrades to a generic label instead
of a parse failure.

Two caps guard the listing. The read itself runs through `checked_bounded` with
`MAX_DIFF_INDEX_BYTES` (8 MiB), and the entry loop stops at `MAX_DIFF_INDEX_FILES` (16,384
files), setting `truncated` in both cases. When the byte cap fires the buffer is additionally
cut back to the byte after its last NUL so only whole records parse:

```rust
fn truncate_diff_index(output: &mut Vec<u8>) -> bool {
    if output.len() <= MAX_DIFF_INDEX_BYTES {
        return false;
    }
    let boundary = output
        .get(..MAX_DIFF_INDEX_BYTES)
        .unwrap_or_default()
        .iter()
        .rposition(|byte| *byte == 0)
        .map_or(0, |index| index + 1);
    output.truncate(boundary);
    true
}
```

### Read two, the numstat totals

Before the name-status command even runs, `diff_index_files` derives and executes its numstat
twin. The derivation is a token swap, not a second command definition:

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

This is a small function doing correctness work. The two reads must describe the same revision
range with the same rename detection, or counts would attach to the wrong files; deriving one
argv from the other makes divergence impossible by construction, including for the `diff-tree`
root-commit form and the `stash show` form (both contain `--name-status`, so both get numstat
twins automatically). The swap returns `None` for an argv without `--name-status`, in which
case the index simply has no counts.

The read itself is deliberately failure-tolerant:

```rust
/// Counts are a rendering enhancement, never a correctness requirement, so a
/// failed or bounded read simply leaves the affected headers unresolved.
fn numstat_counts(&self, args: Vec<OsString>) -> HashMap<PathBuf, DiffLineCounts> {
    self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)
        .map(|(output, _)| parse_numstat(&output))
        .unwrap_or_default()
}
```

Any error collapses to an empty map, every file's `counts` stays `None`, and the headers show
the `+··` placeholders until patches arrive and backfill them. Nothing downstream distinguishes
"numstat failed" from "numstat had no entry for this path"; both are just absent counts.

### parse_numstat and its rename and binary handling

`git diff --numstat -z` emits one record per file of the form `additions TAB deletions TAB
path`, NUL-terminated. Two encodings inside that simple shape require care, and both live in
`parse_numstat` in `src/git/diff.rs`.

**1. Renames split into three records.** With `-z`, a renamed file's record has an empty path
field, and the two paths follow as separate NUL-terminated records: pre-image, then post-image.
The doc comment on the function states the rule: renames "emit an empty path field followed by
the pre-image and post-image records, so the scanner has to consume those two extra records
instead of assuming one." The scanner keys the entry by the post-image path and advances its
cursor past both:

```rust
if path.is_empty() {
    let Some(new_path) = records.get(cursor + 1) else {
        break;
    };
    cursor += 2;
    let _ = counts.insert(record_path(new_path), entry);
} else {
    let _ = counts.insert(record_path(path), entry);
}
```

Keying by the post-image path only is deliberate: the index entry for a rename is also keyed by
its post-image path, so the join in `diff_index_files` (`counts.get(&path).copied()`) lines up,
and the pre-image path never creates a phantom entry.

**2. Binary files report dashes.** A binary file's record reads `-\t-\tpath`: both count fields
are a single `-` byte because line counts are meaningless for binary content. The parser turns
that into `binary: true` with zero counts:

```rust
let binary = additions == b"-" || deletions == b"-";
let entry = DiffLineCounts {
    additions: parse_count(additions),
    deletions: parse_count(deletions),
    binary,
};
```

`parse_count` maps any non-numeric field to 0, so the dash rows parse cleanly and the `binary`
flag is what surfaces in the header label (the `· binary` suffix from
`DiffFileIndexEntry::label`).

There is a third, quieter subtlety: the field split is `record.splitn(3, |byte| *byte == b'\t')`,
which splits on at most the first two tabs. A path that itself contains tab characters (legal in
Git, and raw under `-z`) therefore survives intact inside the third field. The test
`reads_numstat_totals_for_plain_renamed_and_binary_paths` pins all three behaviors with one
input buffer containing a plain file, a rename triple, a binary dash record, and a
tab-containing path:

```text
1␉1␉src/keep.rs␀1␉0␉␀old/name.rs␀new/name.rs␀-␉-␉assets/logo.png␀4␉2␉path␉with␉tabs.rs␀
```

(tabs shown as `␉`, NULs as `␀`). The assertions: four entries parse, the rename keys under
`new/name.rs` only, `assets/logo.png` is binary, and `path\twith\ttabs.rs` keeps its counts
4 and 2.

### Working-tree counts with two calls at most

The working-tree source cannot reuse `numstat_args`, because its file list came from the status
snapshot rather than from a diff command. It gets its own count filler,
`apply_worktree_counts` in `src/git/mod.rs`, whose doc comment states the budget: "Working-tree
changes are already known from the status snapshot, so the index needs only their totals. One
`--numstat` read per populated area keeps that to at most two extra Git calls regardless of
file count."

The mechanism: staged changes are counted by `git diff --numstat --cached -z --find-renames --`
and unstaged ones by the same command without `--cached`, each run only if any change actually
belongs to that area. Then each index entry picks its counts from the map matching its area:

```rust
for (file, change) in files.iter_mut().zip(changes) {
    let counts = if change.area == ChangeArea::Staged {
        &staged
    } else {
        &unstaged
    };
    file.counts = counts.get(&file.path).copied();
}
```

A file modified in both areas appears twice in the change list (porcelain v2 `MM` produces a
staged entry and an unstaged entry), and each of its two index entries resolves against the
correct area's map, so the staged row shows the staged totals and the unstaged row the unstaged
totals. Untracked files appear in neither numstat output and simply keep `counts: None` until
their synthesized patch loads, which the test suite pins explicitly.

### Read three, the per-file patch

A file's body loads through `local_diff_file`, which first re-resolves the path against the
stored index (an unknown path is the error `"{path} is not part of this diff"`) and then
dispatches per source. The revision-based path builder is representative:

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

The flags again earn their places. `--no-color` guards against ANSI escapes from any
`color.diff` configuration reaching the parser. `--no-ext-diff` disables external diff drivers,
which could otherwise run arbitrary user-configured programs and emit arbitrary formats.
`--find-renames` keeps rename detection consistent with the index reads. And the path list
after `--` is built by `append_diff_file_paths`, which pushes the pre-image path before the
post-image path for renames:

```rust
fn append_diff_file_paths(args: &mut Vec<OsString>, file: &DiffFileIndexEntry) {
    if let Some(old_path) = &file.old_path {
        args.push(old_path.as_os_str().to_owned());
    }
    args.push(file.path.as_os_str().to_owned());
}
```

Without the old path, a path-limited diff of a renamed file would see only the post-image side
and report the file as wholly added; naming both paths lets Git pair them and emit a real
rename section with its content hunks.

Every per-file read funnels through one bounded runner:

```rust
fn diff_document_from_args<I, S>(
    &self,
    args: I,
    title: &str,
    path: &Path,
) -> Result<DiffDocument>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let (mut output, truncated) = self.checked_bounded(args, MAX_DIFF_BYTES)?;
    if truncated {
        truncate_to_complete_line(&mut output);
    }
    Ok(parse_diff(&output, title, Some(path), truncated))
}
```

Cap, repair to a line boundary, parse, and carry the truncation flag into the document so the
renderer can append its notice row. `MAX_DIFF_BYTES` is 8 MiB; the enforcement mechanics (the
pipe reader that kills the child at the cap) are covered in the caps section below.

The working-tree per-file read, `raw_diff_for_change`, follows the same shape with two area
modifiers: `--cached` selects the index-versus-HEAD diff for staged changes, and `--cc` selects
the combined format for conflicted files. Untracked files never reach Git at all; they take the
synthesis path described in its own section.

### The stash special case

A stash is not one commit; `git stash` records the working tree as a commit with the stash base
as `{ref}^1`, and, when untracked files were included, a third parent `{ref}^3` holding only
those untracked files. A complete per-file stash preview therefore needs two reads, and
`stash_diff_file` in `src/git/mod.rs` runs them under one shared budget:

1. The tracked half: `git diff ... {ref}^1 {ref} -- <paths>`, capped at `MAX_DIFF_BYTES`.
2. A probe: `rev-parse --verify --quiet {ref}^3` to learn whether an untracked commit exists at
   all. A stash created without `--include-untracked` has no third parent, and the probe failing
   is a normal outcome, not an error.
3. Only if the probe succeeded and the first read was not truncated, the untracked half:
   `git show --format= ... {ref}^3 -- <paths>`, capped at `MAX_DIFF_BYTES` minus the bytes the
   first half already consumed:

```rust
let (untracked, untracked_truncated) =
    self.checked_bounded(untracked_args, MAX_DIFF_BYTES.saturating_sub(output.len()))?;
output.extend(untracked);
truncated |= untracked_truncated;
```

The two outputs concatenate into one patch buffer, truncation flags OR together, the buffer is
repaired to a complete line, and the combined bytes parse as one document. The shared budget is
the point: a stash preview can never cost more than one patch read's worth of memory no matter
how the content splits between its tracked and untracked halves. A regression test pins that a
tracked-only stash (no `^3`) still previews, so the probe's failure path stays exercised.

### What the app receives

The index and the per-file documents travel to the app as separate worker events:
`WorkerEvent::LocalDiffIndex` carries the `DiffIndex` and establishes the workspace generation,
and each `WorkerEvent::LocalDiffFile` carries one `(path, DiffDocument)` under both the preview
generation and the workspace generation. The app stores per-file documents in a map keyed by
path and rebuilds the display document through the merge step described next. The strictness of
the staleness guards on these events (four simultaneous conditions for a file document to be
accepted) belongs to [rendering/concurrency](../rendering/concurrency.md); what matters here is
the data shape: an index arrives once, documents arrive one at a time, and the merge is
recomputed from whatever subset has landed.

## Parsing patch bytes with parse_diff

`parse_diff` in `src/git/diff.rs` is the single entry point that turns raw patch bytes into a
`DiffDocument`. Every patch in the system passes through it: working-tree diffs, commit and
branch and stash previews, pull-request files (single and batched), and the synthesized
untracked patches. Its signature carries the whole contract:

```rust
pub(crate) fn parse_diff(
    raw: &[u8],
    title: impl Into<String>,
    path_hint: Option<&Path>,
    truncated: bool,
) -> DiffDocument {
```

`raw` is the patch bytes, possibly already truncated to a line boundary. `title` becomes the
document title verbatim. `path_hint` is the caller's knowledge of which file this patch should
concern, used both as a fallback identity for headerless patches and as the initial grammar
selector for syntax highlighting. `truncated` is the caller's report that the bytes are
incomplete, which the parser turns into a visible notice row and a sticky document flag.

### Admission control before the first line

The first two statements decide how much work the whole parse is allowed to do:

```rust
if raw.is_empty() {
    return DiffDocument::empty(title, "No textual diff to display");
}

let assets = (raw.len() <= MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES).then(highlight_assets);
```

An empty input short-circuits to the explanatory one-row document. The second line is the
512 KiB syntax budget (`MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES = 512 * 1024`): when the whole patch
exceeds it, `assets` is `None`, and every downstream highlighting call degrades to plain spans.
The `.then(highlight_assets)` ordering matters: the syntect syntax set and theme are loaded
lazily through a `OnceLock`, so a process that only ever meets oversized patches never pays the
grammar-loading cost at all. This is admission control rather than mid-flight cancellation:
the decision is made once, up front, from a single cheap length check.

The per-line companion budget is `MAX_SYNTAX_HIGHLIGHT_LINE_BYTES` (32 KiB): a single content
line over that size sets the affected side's highlighter to `None` for the rest of the file
section, on the theory that a file containing one 32 KiB line (minified bundles, generated
data) will contain more, and grammar state is already unsalvageable mid-file. The full
highlighting story, including why two highlighter states exist, is the subject of
[intraline emphasis and highlighting](./intraline-and-highlighting.md); this page only marks
where the budgets sit in the parse.

### The prefix dispatch order

The loop body is an ordered chain of prefix tests, and the order is semantically significant.
In match order:

1. `diff --git` flushes the current `FileBuilder` and starts a new one from the header paths.
2. `diff --cc` / `diff --combined` do the same with one path for both sides.
3. If no file has started yet, the line is dropped. This single rule absorbs every commit
   preamble: `git show` output starts with `commit <oid>`, author, date, and message lines, and
   none of them survive because they precede the first `diff --git`. The test
   `groups_commit_patch_into_named_file_sections_and_drops_preamble` pins that no `commit` or
   `Author:` text reaches any row.
4. `index` and `similarity index` lines are dropped as transport noise.
5. `new file mode` and `deleted file mode` set the builder's status without producing rows.
6. `rename from` / `rename to` set the builder's paths and status without producing rows.
7. `---` sets the old path; `+++` sets the new path and re-derives the highlighting grammar.
8. `old mode` / `new mode` become visible `Meta` rows.
9. `Binary files` / `GIT binary patch` mark the file binary; every later line of a binary file
   becomes a `Meta` row.
10. `@@` seeds the line counters and emits a `HunkHeader` row.
11. `+`, `-`, and space produce `Added`, `Removed`, and `Context` rows with numbers and spans.
12. Anything else non-empty becomes a `Meta` row; empty lines are dropped.

The order encodes precedence facts about the format: `deleted file mode` must be tested before
the general `-` content arm would ever see it (it starts with `d`, so it would actually fall to
the meta arm, but the explicit test communicates intent and future-proofs the chain);
`similarity index` must be dropped before the bare `index` test could half-match it, which is
why both prefixes appear in one condition; and the binary containment arm sits before the hunk
and content arms so base85 payload bytes can never masquerade as hunks.

The three content arms are where the row model gets its numbers. The added arm is
representative:

```rust
if let Some(content) = raw_line.strip_prefix('+') {
    let number = new_line;
    new_line = new_line.map(|line| line + 1);
    let content = expand_tabs(content);
    let spans = highlight_optional(&mut new_highlighter, &content, assets);
    let file = file_mut(&mut current_file, path_hint);
    file.additions += 1;
    file.lines.push(DiffLine {
        kind: DiffLineKind::Added,
        old_line: None,
        new_line: number,
        spans,
    });
}
```

The number captured is the counter's value before the increment, matching the 1-based semantics
seeded from the hunk header. The counters are `Option<usize>`: before any hunk header has been
seen they are `None`, and content rows arriving in that state (malformed input) carry no
numbers rather than wrong ones. `expand_tabs` normalizes tabs to 4-column tab stops at parse
time, measuring position in display-width columns so tabs after wide characters land on the
correct stop; doing this once at parse time means the renderer, the intraline matcher, and the
horizontal scroller all see a tab-free string and never need per-frame width special cases.

The context arm advances both counters and both highlighter states:

```rust
let spans = highlight_optional(&mut new_highlighter, &content, assets);
advance_highlighter(&mut old_highlighter, &content, assets);
```

A context line exists in both files, so both grammar states must consume it to stay
synchronized; only the new-side output is kept, since one rendering of an unchanged line
suffices. This dual-state bookkeeping is the mechanism behind the function's doc comment:
"Keeping two parser states avoids additions corrupting the old-file syntax state and removals
corrupting the new-file state." A removed line that opens a block comment must poison the
old-side state only; an added line that closes one must affect the new side only.

### FileBuilder and the flush

Rows accumulate per file in a `FileBuilder`, a small mutable record of both paths, the detected
status, the row list, running addition and deletion tallies, and the binary flag. The accessor
that every arm uses hides one recovery behavior:

```rust
fn file_mut<'a>(
    current: &'a mut Option<FileBuilder>,
    path_hint: Option<&Path>,
) -> &'a mut FileBuilder {
    current.get_or_insert_with(|| FileBuilder::new(None, None, path_hint))
}
```

If content arrives before any `diff --git` header, a builder is conjured from the `path_hint`.
This is not defensive paranoia; it is a supported input shape. A raw single-file patch that
starts at `--- a/...` (or even at `@@`) parses correctly when the caller knows which file it
belongs to, which is exactly the situation for per-file reads where the caller always passes
the path. The rule interacts with the preamble rule above: preamble lines are dropped only
because the `current_file.is_none()` check precedes the content arms, and the first
recognizable diff structure after it establishes the file.

When the input ends, the builders flush through `flush_file`, which finalizes each file
section:

```rust
fn flush_file(mut file: FileBuilder, output: &mut Vec<DiffLine>) {
    if file.lines.is_empty() {
        file.lines.push(meta_line(
            DiffLineKind::Meta,
            if file.status == Some("renamed") {
                "File renamed without content changes"
            } else {
                "No textual changes to display"
            },
        ));
    }

    let status = file
        .status
        .map(|status| format!("  · {status}"))
        .unwrap_or_default();
    output.push(DiffLine {
        kind: DiffLineKind::FileHeader,
        old_line: None,
        new_line: None,
        spans: vec![
            HighlightSpan::plain(format!("{}{}", file.display_path(), status)),
            HighlightSpan::plain(format!("+{}", file.additions)),
            HighlightSpan::plain(format!("-{}", file.deletions)),
        ],
    });
    output.append(&mut file.lines);
    output.push(meta_line(DiffLineKind::FileFooter, ""));
}
```

The empty-body substitution handles the two legitimate ways a file section can contain no
content rows: a pure rename (header lines only) and a pure mode change. Each gets an honest
explanatory row instead of a header floating over nothing. The header's three spans mirror the
index-side header exactly (label, `+n`, `-n`), which is what makes the merge step's span-level
surgery possible: both header producers agree on the span layout, so the merge can copy span 1
and span 2 between them blindly.

### Sorting files inside one patch

Between the flush loop and the output, one line does ordering work:

```rust
files.sort_by_cached_key(FileBuilder::sort_path);
```

`sort_path` is the post-image path (falling back to the pre-image), rendered lossily with
backslashes normalized to `/`. The sort is case-sensitive byte order on the full repository
path, which matches the order `git diff` itself lists files in and, more importantly, matches
the order of the name-status index. A multi-file patch (a commit preview rendered whole, or a
batched PR read) therefore interleaves correctly with index-derived documents: the same file
order everywhere means the file tree, the collapsed skeleton, and a fully parsed multi-file
document never disagree about position. The test
`sorts_files_by_case_sensitive_full_repository_path` pins the exact order for a mixed set:
`.github/ISSUE_TEMPLATE/bug.yml` before `.github/labeler.yml` before `CODE_OF_CONDUCT.md`
before `Cargo.toml` before `README.md` before `src/app.rs`, demonstrating that dotfiles sort
first and uppercase sorts before lowercase in byte order.

### Decoding quoted paths

`decode_git_path` handles the C-quoted path form. When the value is wrapped in double quotes it
decodes byte-wise: `\n`, `\r`, `\t`, and `\"` map to their bytes, an octal escape of up to
three digits accumulates with saturating arithmetic, any other escaped byte passes through
unchanged, and a trailing lone backslash is preserved rather than dropped:

```rust
Some(first @ b'0'..=b'7') => {
    let mut value = first - b'0';
    for _ in 0..2 {
        let Some(next @ b'0'..=b'7') = bytes.peek().copied() else {
            break;
        };
        let _ = bytes.next();
        value = value.saturating_mul(8).saturating_add(next - b'0');
    }
    output.push(value);
}
```

The result goes through `String::from_utf8_lossy`, so a path with genuinely invalid UTF-8
degrades to replacement characters in display while remaining stable as a key. Because Quinjet
runs Git with `core.quotepath=false`, most non-ASCII paths arrive raw and skip this decoder
entirely; it exists for the residual cases where Git quotes regardless (embedded quotes,
control bytes) and for foreign patches produced under default configuration.

### Edge cases the parser absorbs

A catalog of inputs that would break a naive parser and how this one handles them:

- A patch cut mid-line by the 8 MiB cap: never reaches the parser, because every capped caller
  repairs to a line boundary first. The parser additionally appends its own notice when the
  `truncated` argument is set: a `Meta` row reading `… diff truncated to keep Quinjet
  responsive …`.
- A file with tabs in its name: survives numstat (splitn 3), survives name-status (`-z`), and
  survives the patch header (trailing-tab trim only removes the label-terminating tab).
- A patch for a file with no newline at end: the `\ No newline at end of file` marker renders
  as a meta row in exactly the position Git emitted it.
- Windows-style paths in foreign patches: normalized to `/` only for sorting, preserved for
  display.
- An empty patch for a real request (file unchanged between the requested revisions): the
  empty-input arm answers `No textual diff to display` instead of an empty screen.
- Invalid UTF-8 anywhere: the whole buffer goes through `String::from_utf8_lossy` once at the
  top, so the parser operates on `str` and pays the lossy conversion exactly once.

## Merging skeleton and bodies with document_with_visibility

`DiffIndex::document_with_visibility` is where the index-first design becomes a single
scrollable document. It takes the index, a map of loaded per-file documents, and a visibility
predicate, and produces the merged document in one pass over the file list:

```rust
pub(crate) fn document_with_visibility(
    &self,
    loaded: &HashMap<PathBuf, DiffDocument>,
    mut visible: impl FnMut(&Path) -> bool,
) -> DiffDocument {
```

The visibility predicate is the fold state: the app passes a closure over its set of expanded
paths, so a collapsed file is "not visible" regardless of whether its patch has loaded. For
pull requests the closure also encodes the viewport-first policy described in
[progressive loading](../rendering/progressive-loading.md), where only files near the viewport
contribute bodies to the combined document.

### The skeleton shape

An empty index short-circuits to the one-row empty document with the index's commit details
attached. Otherwise the output vector is pre-sized to three rows per file:

```rust
let mut lines = Vec::with_capacity(self.files.len().saturating_mul(3));
```

Three is the collapsed-file row count: header, one placeholder meta row, footer. That is the
shape of the initial skeleton for any freshly opened diff, and it is why a 2,188-file pull
request renders its complete file list as roughly 6,500 cheap rows before any patch exists.
The capacity hint means the common case (mostly collapsed) allocates once.

The document's `truncated` flag starts from the index's own flag and absorbs each loaded
file's flag as it merges, so a truncation anywhere in the pipeline (index cut, capped patch)
is visible on the final document.

### The four merge cases per file

For each index entry the merge inspects two booleans: is the file visible, and is a loaded
document with a real `FileHeader` available. The four combinations produce four different row
contributions:

**1. Visible and loaded.** The fast path clones the loaded document's rows wholesale and then
performs one span replacement: the first span of its `FileHeader` is overwritten with
`file.label()`.

```rust
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
```

The label overwrite makes the index authoritative for identity: the index knows the file's
status from name-status and its binary flag from numstat, while a path-scoped patch parse can
have a less complete view (a patch section alone does not know its diff-wide status context).
The loaded document's own count spans (spans 1 and 2) survive untouched, because they were
computed from the real patch and are exact.

**2. Not visible (loaded or not), and 3. visible but not loaded.** All remaining cases start
from a synthetic header built by `index_file_header`, which renders `file.label()` plus the
`count_spans()` pair, placeholders included. Then one more piece of surgery runs: if a loaded
document exists (a collapsed-but-loaded file), its header's exact count strings are copied
into spans 1 and 2 of the synthetic header:

```rust
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
```

This is the placeholder-resolution ladder in miniature. A header's counts come from, in
descending priority: the loaded patch's real tallies, then numstat counts baked into
`count_spans`, then the `+··` / `-··` placeholders. A file whose numstat was missing but whose
patch has arrived shows real counts even while collapsed.

The body row for these cases is a single meta line chosen by state:

```rust
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
```

The `show_body && loaded` branch inside this block is case 4: a loaded document that contains
no `FileHeader` of its own (an unusual parse, possible for degenerate inputs) still
contributes its rows under the synthetic header rather than being dropped. Every path through
the four cases ends with the same `FileFooter` row, keeping the section geometry uniform.

The three placeholder strings are deliberate user communication, not internal states leaking:
"Loading diff…" says work is in flight for a file the reader is looking at; "Expand this file
to load its diff" says no work will happen until asked; "Diff loaded · expand this file to
display it" says the data is already local and expanding is free. The distinction between the
last two is the visible face of the prefetcher: as background batches land, collapsed files
flip from the third message to the second, telling the reader that expansion has become
instant.

### Totals that never depend on patches

Note what the merge never does: it never recomputes counts from rows, and it never lets a
missing patch subtract from anything. The test
`lazy_index_keeps_all_headers_while_merging_one_loaded_file` runs the full ladder: a two-file
skeleton shows `+··` and two "Loading diff…" rows; after one file's document loads, the merged
document has both headers, one real addition row, one real removal row, and the unloaded file
still shows its placeholder; with everything collapsed, the loaded file's header still shows
its real `+1` counts and the two collapsed-state messages appear once each. The header layer
and the body layer update independently, which is precisely invariant 8a's "a file's totals
never depend on whether its patch has loaded."

### The pull request adapter

Pull requests carry their own index type (`PullRequestDiffIndex` in `src/git/github/mod.rs`,
with typed statuses and API-sourced counts), but the merge is the same code. When the app
rebuilds the all-files PR document, it converts each `PullRequestFile` into a
`DiffFileIndexEntry` (mapping the typed status to its lowercase label and copying counts), then
calls the same `document_with_visibility` with the PR document cache as the loaded map, and
finally attaches `pull_request_details` to the result (`src/app.rs:5703-5727`). One merge
implementation serves the working tree, commits, branches, stashes, and pull requests, which
is why every one of those views folds, scrolls, and fills identically.

## Splitting batched patches with split_patch_by_file

The per-file model has one cost the index-first design cannot hide: process spawns. If every
one of 2,188 files needed its own `git diff` invocation, the fixed overhead per process
(fork/exec, repository discovery, object store setup) would dominate the actual diff work. The
answer is stated in the doc comment on `PreparedPullRequest::diff_files` in
`src/git/github/mod.rs`: "Produce many file documents from a single `git diff`. Spawning one
Git process per file dominates the cost of a wide pull request, so batching is what lets the
whole diff arrive while the reader is still reading the first file."

Batching only works because the combined output can be cut back apart losslessly, and that is
`split_patch_by_file`'s job, per its own doc comment in `src/git/diff.rs`: "Cut a multi-file
patch at its `diff --git` boundaries and key each section by the paths in that header. One Git
invocation can then answer for many files while each file still parses and renders as its own
document." Invariant 10a in `ARCHITECTURE.md` promotes the pattern to an architectural rule:

> Remaining patches arrive through batched background reads keyed to the prepared workspace
> rather than to a preview generation, so they can never invalidate a reader's own request. One
> Git invocation answers for a batch of paths and the combined patch is split back apart at its
> `diff --git` boundaries.

### The offset scanner

The splitter works on bytes, not strings, and allocates nothing for the scan:

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
```

The first pass records the byte offset of every line that starts a section. The second pass
slices the buffer between consecutive starts (the last section runs to the end of the patch)
and parses only each section's first line to extract its paths, reusing `diff_header_paths`
for the `diff --git` form and the single-path decode for the combined forms. The output is a
vector of borrowed sections:

```rust
pub(crate) struct PatchSection<'a> {
    pub old_path: Option<PathBuf>,
    pub new_path: Option<PathBuf>,
    pub body: &'a [u8],
}

impl PatchSection<'_> {
    pub(crate) fn matches(&self, path: &Path) -> bool {
        self.new_path.as_deref() == Some(path) || self.old_path.as_deref() == Some(path)
    }
}
```

`body` is a slice into the original buffer, so an 8 MiB combined patch is never copied during
splitting; only the sections that survive to caching or parsing are materialized. `matches`
accepting either path is the rename accommodation: a batch may request a file by its post-image
path while the section header leads with both names, and a section must also be findable by
the pre-image path because the index entry for a rename knows both. The test
`splits_a_batched_patch_into_one_section_per_file` pins this with a three-section patch whose
middle section is a pure rename with spaces in both names: the section matches under
`new name.rs` and under `old name.rs`, sections do not cross-match, and a section's body parses
standalone into a one-file document.

Correctness of the byte-prefix cut rests on the self-delimiting property established in the
format section: hunk body lines always begin with a space, `+`, `-`, or `\`, so the literal
bytes `diff --git` at a line start cannot occur inside any file's content region. A patch
whose content contains the text `diff --git` mid-line (this very documentation, for example)
is safe because the scanner only tests line starts.

### diff_files from cache partition to documents

`PreparedPullRequest::diff_files` orchestrates a batch end to end. Its phases:

**1. Resolve and partition.** Requested paths resolve against the workspace index (unknown
paths are silently dropped; the caller's index may be newer than the workspace's). Each
resolved file checks the immutable per-file patch cache, keyed
`pr-patch-v1\n{merge_base}\n{head}\n{path}`:

```rust
let mut cached: HashMap<PathBuf, Vec<u8>> = HashMap::new();
let mut requested: Vec<PathBuf> = Vec::new();
for file in &files {
    let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
    match cache_read_bounded(&key, CacheLife::Immutable, MAX_CACHED_PATCH_BYTES) {
        Some(patch) => {
            drop(cached.insert(file.path.clone(), patch));
        }
        None => requested.push(file.path.clone()),
    }
}
```

Because the key names two immutable commit OIDs plus the path, a hit can never be stale, only
absent; the reasoning behind that cache taxonomy is [github/caching](../github/caching.md).
Reopening a pull request therefore replays most of its patches from disk without any Git work,
which is what the warm-cache numbers in the evidence below are measuring.

**2. One Git call for the misses.** All cache misses go into a single
`diff_selected_paths` invocation: `git diff --no-color --no-ext-diff --find-renames --patch
--unified=3 <merge_base> <head> -- <paths...>`, stdout capped at `MAX_DIFF_BYTES` (8 MiB) and
repaired to a line boundary on truncation. The revision pair is the workspace's pinned merge
base and head, so a batch is byte-for-byte reproducible; how those two OIDs were obtained is
the story of [merge bases and history](../git-internals/merge-bases-and-history.md) and the
[PR workspace](../github/pr-workspace.md).

**3. Split and assign.** `split_patch_by_file` cuts the combined output, and each requested
file finds its section by `matches`. A file with no section (unchanged between the pinned
commits, or beyond a truncation cut) is skipped rather than given an empty document, leaving it
eligible for a later request.

**4. Cache and emit.** Complete sections are written to the per-file cache (bounded at
`MAX_CACHED_PATCH_BYTES`, 1 MiB, so one enormous file cannot crowd the cache) and parsed into
documents via `pull_request_file_document`, which wraps `parse_diff` with the PR title format
and attaches `PullRequestDetails`.

### The truncated last section and its retry

The delicate case is a batch whose combined output hit the 8 MiB cap. Only one section can be
damaged: the last one, because truncation cuts the stream at a single point and every earlier
section ended before it. The handling in `diff_files`:

```rust
let section_truncated = truncated && index == sections.len().saturating_sub(1);
if section_truncated && requested.len() > 1 {
    if truncated_fallback.is_none() {
        truncated_fallback = Some((
            file.path.clone(),
            pull_request_file_document(section.body, &self.pull_request, file, true),
        ));
    }
    continue;
}
if !section_truncated {
    let key = patch_cache_key(&self.merge_base, &self.head, &file.path);
    cache_write_bounded(&key, section.body, MAX_CACHED_PATCH_BYTES);
}
```

Three rules compose here:

**1. A cut section in a multi-file batch is withheld.** Emitting it would present a half patch
as the file's document and, worse, caching it would make the damage permanent under an
immutable key. Instead the file is simply not answered: it stays absent from the app's document
map, `pull_request_file_needs_patch` keeps returning true for it, and the next prefetch round
requests it again, now in a smaller batch or alone with the full 8 MiB budget to itself.

**2. Complete sections always land.** The files before the cut are unaffected; they cache and
emit normally, so a truncated batch still makes real progress.

**3. Starvation is impossible.** If every requested section was cut (a single-file batch whose
one file exceeds 8 MiB), the withheld document is emitted anyway, flagged truncated:

```rust
if documents.is_empty()
    && let Some(fallback) = truncated_fallback
{
    documents.push(fallback);
}
```

A file that can never fit the cap thus renders its truncated head with the truncation notice
rather than looping forever, and because a truncated document is never cached, a future larger
budget (none exists today, but the invariant holds regardless) would re-read it cleanly.

The single-file read path, `PreparedPullRequest::diff_file`, obeys the same cache discipline
from the other direction: a cache hit builds the document with `truncated: false` and no Git
call; a miss runs `diff_selected_paths` for the one path and writes the cache only when the
output was complete.

### Backfilling counts from arrived patches

The last piece of the header-counts ladder lives on the consumer side. A pull-request file can
reach the app with `counts: None` (the API count read failed, or GitHub had no numbers for it),
and the fix arrives with its patch. `backfill_pull_request_counts` in `src/app.rs`:

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
```

The two guards are the whole design: a truncated document's tallies would undercount, so it
never backfills; and a file that already has counts keeps them, so API numbers are never
overwritten by locally derived ones. When both guards pass, the function counts the document's
`Added` and `Removed` rows and installs the result. The return value feeds a render decision:
in the batch-arrival handler, `counts_changed` ORs across the batch, and the all-files document
rebuilds only when a newly counted or newly visible file actually changed what the screen
would show. This is the mechanism behind the tail of invariant 5's prefetch sentence: the
background walk "backfills a header's counts from its arrived patch when GitHub could not
report them."

### The measured effect

The batch-and-split design, together with the API-sourced counts and merge base it depends on,
is what the PR #47 evidence comment measured on the Bun rewrite pull request. Quoting the
comment's transcript verbatim (cold cache means the on-disk patch and listing caches were
empty; warm means a second run):

```console
$ quinjet pr view 30412
#30412  Rewrite Bun in Rust
MERGED · @Jarred-Sumner · opened Sat May 9 3:44 AM · updated Tue Aug 11 3:34 PM
Changes      2188 files, +1009257 -4024
$ time quinjet pr files 30412 | wc -l
2188                                # every file with its real +n -n counts
real  0m6.30s                       # cold cache
real  0m0.04s                       # warm
$ time quinjet pr diff 30412 .buildkite/ci.mjs | head -2
.buildkite/ci.mjs  · modified +49 -28
@@ -368,15 +368,19 @@ function getLinkBunAgent(platform, options) {
real  0m0.10s
```

The shape of those numbers is the pipeline in miniature: listing 2,188 files with real counts
costs 6.30 seconds cold and 0.04 seconds warm, and a single file's patch costs 0.10 seconds
because it is one path-scoped `git diff` against two pinned commits, independent of the other
2,187 files. The full benchmark narrative lives in [benchmarking](../benchmarking.md).

## Untracked-file patch synthesis

An untracked file defeats the entire pipeline premise, because Git cannot diff what it has
never seen: `git diff` compares trees, indexes, and tracked worktree files, and an untracked
path exists in none of those. Quinjet still owes the reader a preview, so `untracked_patch` in
`src/git/mod.rs` manufactures one, producing bytes indistinguishable from a real Git patch so
that everything downstream (the parser, the highlighter, the fold logic, the caps) applies
unchanged.

### The path guard

The function's first act is defensive:

```rust
fn untracked_patch(&self, change: &Change) -> Result<(Vec<u8>, bool)> {
    let path = safe_worktree_path(&self.root, &change.path)?;
    let metadata = fs::symlink_metadata(&path)
        .with_context(|| format!("failed to read {}", change.display_path()))?;
```

This is the only place in the local diff pipeline that reads file contents directly from the
filesystem rather than through Git, so it is the only place path traversal is even possible.
`safe_worktree_path` rejects absolute paths and any path containing parent-directory, root, or
prefix components before joining onto the repository root, with the error "refusing to access
path outside the repository". The status snapshot is the only source of these paths, so the
guard should never fire, and that is exactly why it exists: the property is enforced where the
filesystem is touched, not assumed from provenance. `symlink_metadata` (rather than `metadata`)
is the second quiet choice: it does not follow symlinks, so a symlink to a huge or sensitive
file classifies as a non-regular file instead of being opened.

### Binary stubs

Non-regular files (directories recorded as untracked, symlinks, sockets, FIFOs) and files
containing NUL bytes get a synthesized binary notice instead of content:

```rust
let binary_patch = || {
    format!(
        "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\nBinary files /dev/null and b/{display_path} differ\n"
    )
    .into_bytes()
};
if !metadata.is_file() {
    return Ok((binary_patch(), false));
}
```

The stub mimics Git's own output for a new binary file: a `diff --git` header, a
`new file mode` line (which the parser turns into the "added" status), and the
`Binary files ... differ` notice (which sets the binary flag and renders as a meta row). The
NUL heuristic mirrors Git's own binary detection closely enough for preview purposes: after
reading the content, `contents.contains(&0)` routes to the same stub, carrying the input
truncation flag in case the NUL appeared beyond an 8 MiB prefix of a text-looking file.

### Synthesizing the text patch

For a regular text file the function builds a creation patch by hand:

```rust
let body = String::from_utf8_lossy(&contents);
let line_count = body.lines().count();
let mut patch = format!(
    "diff --git a/{display_path} b/{display_path}\nnew file mode 100644\n--- /dev/null\n+++ b/{display_path}\n@@ -0,0 +1,{line_count} @@\n"
);
for line in body.split_inclusive('\n') {
    patch.push('+');
    patch.push_str(line);
}
if !body.is_empty() && !body.ends_with('\n') {
    patch.push('\n');
    patch.push_str("\\ No newline at end of file\n");
}
```

Every element matches the format specification from the top of this page: `/dev/null` as the
old label marks creation, `@@ -0,0 +1,{n} @@` is the canonical new-file hunk header (old side
starts at 0 with count 0, new side starts at 1 with the real line count), each content line is
prefixed with a single `+`, and a file lacking a final newline gets the backslash marker in the
position Git would emit it. `split_inclusive('\n')` keeps each line's own newline attached, so
the `+` prefix insertion cannot merge or split lines. The parser then treats the synthetic
patch exactly like Git output: the header establishes the file with status "added", the hunk
header seeds `new_line = 1`, and every `+` row numbers sequentially, giving the untracked
preview real gutter numbers.

### Truncation of synthesized patches

Boundedness holds even without Git in the loop. The read takes `MAX_DIFF_BYTES + 1` bytes,
uses the extra byte purely to detect that the file was larger, and then truncates back:

```rust
let _ = fs::File::open(&path)
    .with_context(|| format!("failed to read {}", change.display_path()))?
    .take(MAX_DIFF_BYTES as u64 + 1)
    .read_to_end(&mut contents)
    .with_context(|| format!("failed to read {}", change.display_path()))?;
let input_truncated = contents.len() > MAX_DIFF_BYTES;
contents.truncate(MAX_DIFF_BYTES);
```

And because the `+` prefixes and header inflate the output beyond the input size, the
assembled patch is capped a second time through `truncate`, the cap-then-repair helper that
truncates to the limit and pops back to a complete line. The returned flag ORs both
truncations, so the document's notice row appears whether the file itself or only the
synthesized encoding crossed the limit. A 100 MiB accidental artifact in the working tree
costs one bounded read and renders as an 8 MiB truncated preview, never an out-of-memory
condition.

## Caps and budgets end to end

Every stage of the pipeline runs under an explicit numeric bound, and the bounds compose: a cap
on raw bytes feeds a cap on parsed size feeds a cap on what the prefetcher may request next.
This section collects all of them, with the enforcement code for each. The philosophy is
invariant 6 from `ARCHITECTURE.md`:

> Potentially large local and PR subprocess output is read through capped pipes. Crossing a cap
> kills the child rather than first allocating all output and truncating afterward. Syntax
> grammar parsing stops at 512 KiB per patch or 32 KiB per row, parsed PR patches use a 32 MiB
> in-memory budget, and collapsed cached patches are not cloned into the combined document, so
> post-processing remains bounded after Git returns.

### The complete cap table

| Constant | Value | Defined in | Bounds |
| --- | --- | --- | --- |
| `MAX_DIFF_BYTES` | 8 MiB | `src/git/mod.rs` | Any single patch read, local or PR, single or batched |
| `MAX_DIFF_INDEX_BYTES` | 8 MiB | `src/git/mod.rs` | Local name-status and numstat listings |
| `MAX_DIFF_INDEX_FILES` | 16,384 | `src/git/mod.rs` | Entries in a local diff index |
| `MAX_GIT_ERROR_BYTES` | 128 KiB | `src/git/mod.rs` | stderr retained from a bounded Git call |
| `MAX_PR_PATH_BYTES` | 8 MiB | `src/git/github/mod.rs` | PR name-status and numstat listings and their cache entries |
| `MAX_PR_PATHS` | 16,384 | `src/git/github/mod.rs` | Entries in a PR changed-file index |
| `MAX_CACHED_PATCH_BYTES` | 1 MiB | `src/git/github/mod.rs` | One file's patch in the on-disk cache |
| `MAX_SYNTAX_HIGHLIGHT_PATCH_BYTES` | 512 KiB | `src/git/diff.rs` | Whole-patch syntax highlighting admission |
| `MAX_SYNTAX_HIGHLIGHT_LINE_BYTES` | 32 KiB | `src/git/diff.rs` | Per-line highlighting; crossing it drops the file's highlighter |
| `PULL_REQUEST_PREFETCH_BATCH` | 32 | `src/app.rs` | Files per background batch |
| `PULL_REQUEST_PREFETCH_BYTE_BUDGET` | 6 MiB | `src/app.rs` | Estimated patch bytes per batch |
| `PULL_REQUEST_PATCH_LINE_ESTIMATE` | 80 | `src/app.rs` | Estimated bytes per changed line |
| `PULL_REQUEST_PATCH_FALLBACK_ESTIMATE` | 512 KiB | `src/app.rs` | Estimate for a file without counts |
| `MAX_PREFETCHED_PULL_REQUEST_FILES` | 4,096 | `src/app.rs` | Files the background fill will ever request |
| `MAX_PULL_REQUEST_DOCUMENT_BYTES` | 32 MiB | `src/app.rs` | Parsed PR documents held in memory |

### The kill-on-cap pipe

The primitive under every byte cap is `run_bounded_command` in `src/git/github/mod.rs` (shared
with the local side through `checked_bounded`). It reads the child's stdout in 64 KiB chunks
and enforces the limit at the moment of crossing:

```rust
let remaining = stdout_limit.saturating_sub(collected.len());
if read > remaining {
    collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
    truncated = true;
    drop(child.kill());
    break;
}
```

Killing the child at the cap is the difference between bounding memory and bounding work: a
`git diff` that would produce multi-gigabyte output stops paying CPU and I/O the moment its
first 8 MiB are in hand, so the cap costs at most the limit plus one read buffer of transfer.
A companion thread drains stderr to EOF while retaining at most its own limit, so a chatty
child can never deadlock on a full stderr pipe while the stdout reader waits. The test
`bounded_runner_kills_oversized_git_output` pins the semantics: a 256 KiB blob read under a
1,024-byte cap returns exactly 1,024 bytes with the truncation flag set.

One consequence needs explicit handling upstream: a killed child exits non-zero. The wrapper
`checked_bounded` therefore treats non-zero exit as an error only when stdout was not
truncated; a truncated read is the answer, not a failure. Without that carve-out every capped
read would be reported as a Git error.

### Truncation repair rules

Every cap crossing is followed by a repair that restores the format invariant the next parser
expects:

- Patch text repairs to a complete line: `truncate_to_complete_line` pops bytes until the
  buffer ends with `\n`, because `parse_diff` classifies whole lines by prefix.
- NUL-record output repairs to a complete record: `diff_index_files` and its PR counterpart
  cut back to the byte after the last NUL, because the record walkers pair status and path
  records positionally and a half record would shift every subsequent pairing.
- Batched patches repair structurally: only the final section of a truncated combined patch is
  suspect, and the withheld-and-retry logic in `diff_files` quarantines exactly that section.

The pairing of every cap with its repair is what makes truncation a rendering condition ("this
view is partial, and says so") instead of a correctness condition.

### The 32 MiB parsed-document budget

Raw bytes are only half the memory story. A parsed `DiffDocument` is larger than its patch: a
100-byte source line becomes a `DiffLine` struct, a span vector, and one or more `String`
allocations, each with capacity slack and per-allocation overhead. The in-memory cache of
parsed PR documents is therefore budgeted separately, at
`MAX_PULL_REQUEST_DOCUMENT_BYTES = 32 * 1024 * 1024`, four times the raw patch cap, and the
size of a document is estimated by walking exactly the allocations it retains:

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

Using `capacity()` rather than `len()` counts the memory actually held, including allocator
slack from string growth, so the estimate tracks the real resident cost of a document rather
than its logical text length.

The cache is a map plus an insertion-order queue plus a running byte total, maintained by
`cache_pull_request_document` in `src/app.rs`: inserting a document first subtracts any prior
entry for the same path (so re-parses do not double-count), adds the new estimate, pushes the
path to the back of the order queue, and then prunes:

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

Eviction is FIFO: the oldest cached document goes first, on the theory that the reader's
attention moves forward through a pull request and the documents behind them are the cheapest
to lose (each is one cached-patch re-parse away if revisited, and usually one disk read since
the raw patch stays in the on-disk cache). The `len() > 1` guard is the important boundary
condition: the loop never evicts the last remaining document, so the newest document always
survives even when it alone exceeds the whole 32 MiB budget. A single pathological file can
therefore always be viewed; it just cannot keep neighbors in memory while it is.

`take_pull_request_document` is the third maintenance path: when the single-file view adopts a
cached document as the live document, it is removed from the cache with its bytes and queue
entry, keeping the accounting exact in both directions. The inverse move,
`cache_current_pull_request_single_document`, returns the live document to the cache when the
reader leaves the file, so bouncing between two files re-parses nothing.

### Sizing the prefetch to the caps

The background prefetcher is the component that has to respect all the byte caps
prospectively: it chooses which files to put in a batch before knowing their real patch sizes.
It estimates from the counts the index already carries:

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

The model: each changed line costs about 80 bytes on the wire (the line itself plus its
prefix), plus 4,096 bytes of fixed overhead per file (headers, hunk headers, context lines). A
file without counts assumes 512 KiB, a deliberately pessimistic default that keeps unknown
quantities from stuffing a batch. Batch assembly in `request_pull_request_prefetch` walks
candidate files, accumulating estimates until the next file would push the batch past
`PULL_REQUEST_PREFETCH_BYTE_BUDGET` (6 MiB) or the batch reaches 32 paths:

```rust
let estimate = estimated_patch_bytes(file.counts);
if !paths.is_empty()
    && batch_bytes.saturating_add(estimate) > PULL_REQUEST_PREFETCH_BYTE_BUDGET
{
    break;
}
batch_bytes = batch_bytes.saturating_add(estimate);
paths.push(file.path.clone());
```

The 6 MiB budget sits deliberately under the 8 MiB pipe cap, leaving 2 MiB of headroom for
estimate error, so a well-estimated batch essentially never triggers the truncated-section
retry. And the `!paths.is_empty()` condition encodes the forward-progress guarantee: a single
file whose estimate alone exceeds the budget still travels, alone in its own batch, rather
than stalling the walk. If its real patch then exceeds even 8 MiB, the single-file fallback in
`diff_files` renders its truncated head, closing the last gap in the progress argument.

One retry policy caps the failure side: a batch that errors triggers exactly one immediate
retry (`pull_request_prefetch_retrying` in the batch-arrival handler), and a second
consecutive failure stops the prefetch loop rather than hammering a broken workspace.

### From smallest-first to viewport-first

The prefetch walk order changed twice during the optimization stack, and the history explains
the current shape.

PR #47 introduced byte-budgeted batches themselves, replacing a fixed batch of 12 paths taken
in file-list order; batch size became 32 paths under the 6 MiB estimate budget, and the
prefetch file cap at that point was 400 files. PR #50 then addressed huge pull requests
specifically: past 100,000 total changed lines or 1,000 files, candidates were sorted by
estimated patch bytes ascending, so the byte budget filled with the smallest files first. The
reasoning was throughput: many small files per batch means more headers resolve per Git
invocation, and the huge tail files stop blocking everything behind them.

PR #55 replaced that ordering entirely. Smallest-first optimizes global completion, but the
reader is not global: they are looking at one region of the Files tree, and the files that
matter most are the ones on screen. The current walk starts at the first file visible in the
Files tree and wraps around the rest of the index in order:

```rust
/// Where background fill should start: the first file visible in the
/// Files tree, so patches land where the reader is looking and then wrap
/// around the rest of the index in order.
fn prefetch_anchor_index(&self) -> usize {
```

with the batch loop iterating `from_anchor.iter().chain(before.iter())`, the wrap-around
expressed as slice concatenation. The same PR raised `MAX_PREFETCHED_PULL_REQUEST_FILES` from
400 to 4,096, so the walk now covers even the Bun-scale index completely. The size tiers and
their two constants were removed; the current tree has viewport-anchored ordering only. What
survives from #50 is the estimator itself and the byte budget, which the viewport-first walk
still uses to size every batch. Invariant 5 records the current behavior: background prefetch
"walks the whole index up to 4,096 files, starting at the file the Files tree is showing and
wrapping around the rest in order, sizes each batch by per-file count estimates to stay under
the 8 MiB patch read." The rendering-side effects of this ordering, including how partial
documents draw coherently while the walk is mid-flight, are the subject of
[progressive loading](../rendering/progressive-loading.md), and the mailbox slot that keeps
these batches from ever displacing a foreground preview is described in
[github/prefetch](../github/prefetch.md).

## Design alternatives and why they lost

The pipeline's shape was chosen against real alternatives, several of which are the standard
approach elsewhere. Recording why they lost is as useful as documenting what won.

**1. A Git library instead of subprocesses.** Linking libgit2 or gix would remove process
spawn overhead and give structured diff output without any parsing. It lost on authority and
on surface area. Git's diff behavior is a moving target of heuristics (rename scoring, the
indent heuristic, driver configuration), and a library reimplementation is permanently almost
compatible; delegating to the `git` binary makes Quinjet's previews byte-identical to what the
user's own `git diff` would say, which is the correctness bar a Git tool is held to. The
performance argument also inverts at scale: the expensive diffs are large, and for those the
subprocess overhead is noise while the streaming, kill-on-cap pipe is a feature no in-process
library call offers as naturally. The batching design then eliminates the spawn overhead where
it actually bites (many small reads), keeping the best of both.

**2. One big patch per view.** Rendering `git show` or `git diff` output directly, as a single
parse, is simpler than index-plus-merge and is exactly what earlier terminal tools do. It lost
on the stress benchmark's shape: a million-line patch cannot be read, parsed, or highlighted
inside a responsive frame budget, and a single 8 MiB cap applied to the whole view means large
diffs silently lose their tails. The index-first design renders completeness (every header,
every count) immediately and spends the byte budget per file, so the cap degrades one file's
body instead of the whole view.

**3. One process per file, no batching.** Pure lazy loading without batched prefetch keeps the
code simpler (no splitting, no truncated-section protocol). It lost arithmetically: at 2,188
files, per-file spawns serialize thousands of process startups behind one worker lane, and the
reader who expands file after file pays a spawn each time. One invocation answering 32 files
amortizes the startup while the split keeps the per-file document model intact.

**4. Streaming parse into the renderer.** Parsing incrementally and drawing rows as bytes
arrive would minimize latency to the first row of a single huge patch. It lost on model
complexity: every downstream consumer (folding, the layout caches, intraline pairing,
side-by-side row building) assumes a complete, immutable row list per document, and generation
tagging assumes a document is one atomic answer. The pipeline gets the same perceived effect
at a coarser grain: documents stream per file through the prefetch loop, and each arrival is
an atomic, cacheable unit.

**5. Caching raw patches in memory instead of parsed documents.** Raw bytes are 3 to 4 times
smaller than parsed documents, so a raw cache could hold more files per megabyte. It lost on
the frame budget: the point of the in-memory cache is that re-displaying a file (fold toggle,
view switch, scroll-back) costs no re-parse and no re-highlight, and a raw cache would put a
parse on every one of those interactions. The system uses both tiers deliberately: raw
patches persist in the on-disk cache (1 MiB per file, immutable keys), parsed documents in
the 32 MiB memory budget, and each tier evicts independently.

**6. Counting totals from patches instead of numstat.** Deriving `+n -n` from parsed documents
would avoid the second listing read entirely. It lost because it inverts the availability
order: counts would exist only after the expensive work, so a fresh view would show
placeholder headers precisely when the reader most wants orientation. The numstat read is one
cheap extra process per index against Git's already-computed diff statistics, and it makes
every header exact before any patch exists. The API-count variant of the same decision (for
blob-less PR workspaces, where local numstat would download every blob) is covered in
[api-strategy](../github/api-strategy.md).

## The behavioral contract in tests

The pipeline's edge cases are pinned by a test suite in `src/git/diff.rs` that reads as a
specification. The load-bearing ones, and what each guarantees:

- `parses_hunks_and_tracks_line_numbers`: the counter semantics of the worked example above;
  five content rows with exact old/new number pairs.
- `returns_explanatory_line_for_empty_diff`: empty input produces the explanatory document,
  never an empty row list.
- `lazy_index_keeps_all_headers_while_merging_one_loaded_file`: the full merge ladder;
  placeholders before load, merged bodies after, real counts on collapsed loaded files, and
  the exact placeholder strings for each state.
- `reads_numstat_totals_for_plain_renamed_and_binary_paths`: the three-record rename form,
  dash-marked binaries, and tab-containing paths in one buffer.
- `splits_a_batched_patch_into_one_section_per_file`: section boundaries, rename matching
  under both paths (with spaces), and standalone parseability of a section body.
- `indexed_counts_render_before_any_patch_is_loaded`: headers with API or numstat counts show
  real numbers with no placeholders anywhere in the skeleton.
- `indexed_totals_do_not_depend_on_loaded_or_visible_patches`: `line_counts()` versus
  `addition_count()`, the two-questions separation.
- `highlights_typescript_and_hides_git_transport_headers`: transport lines produce no rows;
  a one-hunk patch is exactly five rows; highlighted rows carry multiple foreground colors.
- `skips_syntax_grammar_work_for_large_patches` and
  `skips_syntax_grammar_work_for_very_long_lines`: both syntax budgets produce plain spans,
  never partial styling.
- `preserves_space_indentation_and_expands_tabs_to_tab_stops`: tab expansion to 4-column
  stops, including tabs after existing indentation.
- `groups_commit_patch_into_named_file_sections_and_drops_preamble`: commit preambles vanish;
  files sort into path order; header text joins as label plus counts.
- `sorts_files_by_case_sensitive_full_repository_path`: the exact byte-order sort.

On the GitHub side, `disposable_pr_workspace_indexes_all_files_and_does_not_mutate_the_source`
in `src/git/github/mod.rs` exercises the batched path end to end against a real local bare
remote: a 21-file pull request indexes completely, `diff_files` returns all 21 documents in
request order, unknown paths yield an empty result, and the source repository's refs and
status are byte-identical before and after, pinning the pipeline's read-only guarantee. And
`bounded_runner_kills_oversized_git_output` pins the pipe cap primitive everything else
stands on.

Together the tests encode the pipeline's three promises: headers are always complete and
honest, bodies arrive incrementally and boundedly, and no input, however hostile or huge, can
make the diff view allocate or compute without limit.

## Where to go next

- [Diff algorithms](./algorithms.md): how Git computes the hunks this pipeline consumes, and
  why Quinjet treats Git as the diff authority instead of reimplementing Myers.
- [Intraline emphasis and highlighting](./intraline-and-highlighting.md): what happens to
  `DiffLine` rows after this pipeline, from syntect grammar states to viewport-scoped
  emphasis.
- [The PR workspace](../github/pr-workspace.md): where the pinned merge-base and head OIDs
  that key every PR patch come from.
- [Prefetch](../github/prefetch.md): the mailbox lane and scheduling policy that drives
  `diff_files` batches.
- [Caching](../github/caching.md): the immutable-key on-disk store that makes warm PR reads
  nearly free.
- [Progressive loading](../rendering/progressive-loading.md): the viewport-first loading
  behavior built on this pipeline in PR #55.
- [Concurrency](../rendering/concurrency.md): the generations and workspace tags that keep
  stale documents off the screen.
- [Benchmarking](../benchmarking.md): the full measurement story on oven-sh/bun#30412.
- [Techniques](../techniques.md): the catalog view of the patterns this page details.
