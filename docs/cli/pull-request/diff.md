# `quinjet pr diff`

Prints a pull request's patch, or one path's patch out of it.

Usage:

```bash
quinjet pr diff <number> [<path>] [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |
| `[PATH]` | path | unset | Limits the patch to one file. Must match a path the pull request changes, exactly as [`quinjet pr files`](./files.md) prints it. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for the metadata. Patches are keyed by commits and are never stale, so this only matters when the head has moved. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`pr diff` prepares the same workspace [`quinjet pr files`](./files.md) does,
takes the resulting list of paths, and reads their patches in batches of 16.
Each batch is one Git process:

```text
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 \
  <merge-base> <head> -- <path> <path> ...
```

Batching is the point. Spawning one Git process per file dominates the cost of a
wide pull request, so sixteen paths at a time keeps a 200 file diff to thirteen
processes rather than two hundred. The combined output is then split back apart
by file header, and each file's section is cached forever under
`pr-patch-v1\n<merge base>\n<head>\n<path>`. A second `pr diff` for the same
pair of commits asks Git for nothing at all, and a batch whose files are all
cached skips the Git call entirely.

That cache is also why there is no whole-file option here, no counterpart to the
`--expanded` flag `quinjet diff` and `quinjet show` take. The cache key is the
merge base, the head and the path, and the width baked into every entry is the
three lines of context above. A second width would mean a second key, a second
copy of every patch in the store and a second `git diff` for anything already
read. When you need the whole file, run `git diff` against the two commits
yourself.

Only patches of at most 1 MiB are cached, so a single generated file cannot
crowd the rest of the pull request out of the store. A patch read that crosses
the global 8 MiB cap has its partial final line trimmed, is not cached, and sets
`truncated`, which prints as:

```json
[output reached Quinjet's size cap and was truncated]
```

With a `<path>`, the path is checked against the prepared index before any patch
is read. A path the pull request does not touch is a name error, not an empty
result:

```console
$ quinjet pr diff 8 nope.md
error: `nope.md` is not part of this pull request
hint: run `quinjet pr files <number>` for the files it changes
```

That exits 3. Matching is exact and literal: no globbing, no prefix matching, no
directory expansion. `quinjet pr diff 8 src/` does not mean "everything under
`src`", it means a file called `src`, and it exits 3. To filter, read
`quinjet pr files 8 --json` and loop.

Because each `git diff` is restricted to the file's new path, rename detection
cannot see the old path and a renamed file prints as a whole-file addition. The
header still says `renamed from <old>`, because that comes from the index, but
the hunk is `@@ -0,0 +1,N @@`. [`quinjet pr files`](./files.md) reports the true
`+n -m` for such a file.

`--json` shape, one object holding the flattened document the terminal interface
renders. `lines` is the whole patch as typed rows rather than raw text, so a
consumer never has to re-parse a diff. `kind` is one of `file-header`,
`file-footer`, `hunk-header`, `context`, `added`, `removed`, `meta`. `oldLine`
and `newLine` are the line numbers on each side and are `null` on headers.
`spans` carries the syntax highlighting: `foreground` is a semantic color name
or `null`, and concatenating every `text` gives the line. `commitDetails` and
`pullRequestDetails` are both `null` for this verb, because the aggregated
document describes a range rather than a single commit:

```json
{
  "title": "PR #8",
  "lines": [
    {
      "kind": "file-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "README.md  · modified", "foreground": null, "bold": false, "italic": false },
        { "text": "+42", "foreground": null, "bold": false, "italic": false },
        { "text": "-6", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "hunk-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "@@ -19,8 +19,10 @@ Quinjet discovers the containing Git repository", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "added",
      "oldLine": null,
      "newLine": 21,
      "spans": [
        { "text": "- Foldable GitHub Actions logs per check run", "foreground": "green", "bold": false, "italic": false }
      ]
    }
  ],
  "truncated": false,
  "commitDetails": null,
  "pullRequestDetails": null
}
```

Syntax highlighting is capped at 512 KiB per patch and 32 KiB per line; beyond
that every span is plain with a `null` foreground.

In the text form a file header is the spans joined with spaces, a hunk header
and a meta line print as they are, added lines get a leading `+`, removed lines
a leading `-`, context lines a leading space, and file footers are dropped. A
blank line separates one file from the next.

Examples:

```bash
quinjet pr diff 8
quinjet pr diff 8 README.md
quinjet pr diff 8 src/git/mod.rs --repo pulkitxm/quinjet
quinjet pr diff 8 --json | jq '[.lines[] | select(.kind == "added")] | length'
quinjet pr diff 8 > pr-8.patch
```

```console
$ quinjet pr diff 8 src/git/diff.rs
src/git/diff.rs  · modified +260 -2
@@ -76,14 +76,34 @@ pub struct CommitDetails {
     pub committed_at: String,
 }
 
+#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
+pub struct DiffLineCounts {
+    pub additions: usize,
+    pub deletions: usize,
+    pub binary: bool,
+}
+
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct DiffFileIndexEntry {
     pub path: PathBuf,
     pub old_path: Option<PathBuf>,
     pub status: String,
+    pub counts: Option<DiffLineCounts>,
 }
```

The output is not a valid `git apply` input. It is a rendering: file headers are
Quinjet's own summary line rather than `diff --git`, and the `index` and
`--- / +++` lines are folded away. Use `git diff` against the two commits if you
need a patch to apply.

## Where to go next

- [`quinjet pr`](./README.md), the rest of this group and the workspace this
  verb prepares
- [`quinjet pr files`](./files.md) for the paths this verb accepts
- [All `quinjet` commands](../README.md)
