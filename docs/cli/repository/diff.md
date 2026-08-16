# `quinjet diff`

Prints the working-tree patch as one document, optionally limited to the index,
to the worktree, or to a set of path prefixes.

Usage:

```bash
quinjet diff [PATHS]... [--staged | --unstaged] [--expanded] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATHS]...` | zero or more paths | all changes | Keeps only changes whose path has one of these as a leading path prefix. Repeatable; a change matching any one of them is kept. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--staged` | flag | off | Keeps only changes in the index, which is the patch a plain `quinjet commit` would record. |
| `--unstaged` | flag | off | Keeps only changes in the worktree, including untracked files. Cannot be combined with `--staged`. |
| `--expanded` | flag | off | Prints whole files instead of three lines of context around each change. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of text. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`diff` starts from a status read, not from `git diff`. It runs the same
`git status --porcelain=v2` that [`quinjet status`](./status.md) runs, filters
the resulting change list, and only then asks Git for patches. That is why its
selection is expressed in Quinjet's three areas rather than in Git's revision
syntax, and why an untracked file has a patch here at all.

The filter runs in two passes over the change list. First the area: `--staged`
keeps only `staged` changes, `--unstaged` keeps only `unstaged` ones, and
neither flag keeps everything, conflicts included. Note the consequence, since
conflicts live in their own area: a conflicted path is dropped by `--staged` and
dropped by `--unstaged` alike, and appears only when neither is given. Passing
both is refused by clap with `error: the argument '--staged' cannot be used with
'--unstaged'` and exit 2.

Then the paths. Each surviving change is kept if its path starts with any of the
given filters, tested component by component with `Path::starts_with`, never as
a substring and never as a glob. `docs` keeps `docs/cli/README.md`; `doc` keeps
nothing; `./docs` keeps nothing, because `.` is a real path component; an
absolute path keeps nothing, because change paths are relative to the worktree
root. Those paths are relative to the worktree root regardless of where you
invoke Quinjet, so `quinjet diff sync_wiki.py` inside `scripts/` matches nothing
and `quinjet diff scripts/sync_wiki.py` matches from anywhere. If nothing
survives either pass, `diff` prints `No changes match` and exits 0.

What survives becomes a workspace. Quinjet builds an index of the selected files
first, filling each file's `+n -n` from at most two `git diff --numstat -z
--find-renames` reads, one with `--cached` and one without, then loads every
file's patch one at a time and composes a single document from the index plus
the loaded parts. Each tracked file is one process:

```bash
git diff --no-color --no-ext-diff --find-renames --unified=3 [--cached] [--cc] -- <path>
```

`--cached` is added for a staged change and `--cc` for a conflicted one, so a
conflict is shown as a combined diff against both merge parents.

An untracked file is different, because `git diff` prints nothing at all for a
path Git does not track. For those Quinjet synthesizes the patch itself, in
Rust, from the filesystem: it reads the file, emits a
`diff --git a/<path> b/<path>` header, a `new file mode 100644` line, one
`@@ -0,0 +1,<line count> @@` hunk, and every line prefixed with `+`, adding
`\ No newline at end of file` when the file does not end in one. If the file
contains a NUL byte in its first 8 MiB, or is not a regular file (a symlink, a
fifo, a socket), it emits the `Binary files /dev/null and b/<path> differ` form
instead. The read is capped at 8 MiB like everything else, and a file larger
than that is marked truncated.

`--expanded` is `--unified=1000000` on every one of those commands. It is the
`t` key of the terminal interface.

The composed text is Quinjet's line model printed back out, not Git's patch.
Each file gets one label line (`scripts/sync_wiki.py  · modified +3 -2`), a
blank line before it if it is not the first, then hunk headers verbatim,
additions prefixed `+`, deletions `-`, context with a leading space. Tabs are
expanded to four-column stops. `diff --git`, `index`, `---` and `+++` lines are
gone. `git apply` will not accept the result.

Cost scales with file count, one Git process per file plus the status read plus
the numstat reads, so a diff of a thousand untracked files starts a thousand
child processes. The 16,384-path index cap does not apply here, because the file
list comes from the status parse rather than from `--name-status`; the 8 MiB cap
applies per file, and any file crossing it sets `truncated` on the whole
document and appends `[output reached Quinjet's size cap and was truncated]`.

`--json` shape, one object, the diff document:

```json
{
  "title": "scripts/sync_wiki.py \u2014 Changes Modified",
  "lines": [
    {
      "kind": "file-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "scripts/sync_wiki.py  · modified", "foreground": null, "bold": false, "italic": false },
        { "text": "+3", "foreground": null, "bold": false, "italic": false },
        { "text": "-2", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "hunk-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "@@ -18,6 +18,7 @@ from __future__ import annotations", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "context",
      "oldLine": 18,
      "newLine": 18,
      "spans": [
        { "text": "import", "foreground": [180, 142, 173], "bold": false, "italic": false },
        { "text": " ", "foreground": [192, 197, 206], "bold": false, "italic": false },
        { "text": "argparse", "foreground": [192, 197, 206], "bold": false, "italic": false }
      ]
    }
  ],
  "truncated": false,
  "commitDetails": null,
  "pullRequestDetails": null
}
```

The separator in `title` is U+2014, written escaped above; the real output
carries the character itself. `title` is `<path> <separator> <area> <status>`
when exactly one change was selected, and `<area label>  <n> files` when more
than one was, where the area is the first selected change's, so a mixed
selection is labeled by whichever area sorts first. `kind` is one of
`"file-header"`, `"file-footer"`, `"hunk-header"`, `"context"`, `"added"`,
`"removed"`, `"meta"`. `oldLine` and `newLine` are 1-based line numbers and are
`null` on every row that is not a body row: an added row has only `newLine`, a
removed row only `oldLine`, a context row both. `spans` carry syntax
highlighting, so the row's plain text is the concatenation of `spans[].text`,
except on a `file-header` row, whose three spans (label, additions, deletions)
join with a single space. `foreground` is an RGB triple or `null`.
`commitDetails` and `pullRequestDetails` are always `null` here; they are filled
by [`quinjet show`](./show.md) and `quinjet pr diff`. `truncated` is true if the
index or any file crossed a cap.

When nothing matches, the document is not a diff document at all but the
standard message object:

```json
{
  "message": "No changes match"
}
```

Examples:

```bash
quinjet diff
quinjet diff --staged
quinjet diff --unstaged src docs
quinjet diff --expanded src/cli/mod.rs
quinjet diff --json | jq -r '.lines[] | select(.kind == "added") | .spans[].text'
```

```console
$ quinjet diff .github/labeler.yml
.github/labeler.yml  · modified +3 -0
@@ -13,3 +13,6 @@ ui:
 git:
   - changed-files:
       - any-glob-to-any-file: ['src/git/**', 'src/watch.rs']
+cli:
+  - changed-files:
+      - any-glob-to-any-file: ['src/cli/**', 'src/main.rs', 'docs/cli/**']
```

```console
$ quinjet diff sub/c.txt
sub/c.txt  · untracked +1 -0
@@ -0,0 +1,1 @@
+x
```

```console
$ quinjet diff b.bin
b.bin  · untracked +0 -0
Binary files /dev/null and b/b.bin differ
```

```console
$ quinjet diff src/main.rs
No changes match
```

## Where to go next

- [`quinjet status`](./status.md) for the change list this filters
- [`quinjet show`](./show.md) for the same document built from a commit
- [`quinjet status`, `diff`, `log`, `show`](./README.md), the rest of this group
  and the caps, the path-filter rules and the exit codes
- [All `quinjet` commands](../README.md)
