# `quinjet branch compare`

Prints the patch between another branch and the one you are on, without
checking anything out.

Usage:

```bash
quinjet branch compare <REFERENCE> [--expanded] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<REFERENCE>` | branch name or full ref | required | The branch to compare against. Either the short name (`main`, `origin/main`) or the full ref (`refs/heads/main`, `refs/remotes/origin/main`). |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--expanded` | flag | off | Print whole files instead of three lines of context around each change. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Any directory inside the worktree works. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the patch. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

**This verb never checks anything out.** It does not move HEAD, does not create
or delete a ref, does not write to the index, does not touch the working tree,
and does not stash anything. That is the entire reason it exists: looking at
what another branch contains should not cost you the state you are in the
middle of. The only Git commands it runs are reads.

`<REFERENCE>` is resolved against the same listing
[`quinjet branch list --all`](./list.md) prints, and it matches on either the
short name or the full ref, whichever you give. It is not a revision: tags,
commit ids, `HEAD` and `main~2` all fail, because none of them is a row in that
listing. A miss exits **3** and says what to run instead:

```console
$ quinjet branch compare v0.0.6
error: `v0.0.6` does not name a branch in this repository
hint: run `quinjet branch list --all` for the branches that exist
```

The listing is scanned in its printed order, which is the current branch, then
local branches by date, then remote-tracking branches by date. The first row
whose name or ref matches wins, so a local branch always beats a
remote-tracking branch that happens to answer to the same string.

The comparison itself is `<REFERENCE>` on the left and `HEAD` on the right:

```text
git diff --numstat     -z --find-renames <REFERENCE> HEAD --
git diff --name-status -z --find-renames <REFERENCE> HEAD --
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 \
  <REFERENCE> HEAD -- <path>
```

The first two build the file index, one for totals and one for statuses, so
every file's `+n -n` is known before any patch is read. The third runs once per
changed file, with `--unified=1000000` in place of `--unified=3` when
`--expanded` is given, and with the old path added ahead of the new one for a
rename.

Two endpoints, not a merge base. `git diff A B` is the two-dot comparison, so
this answers "what would turning `<REFERENCE>` into HEAD change", not "what has
happened on my branch since we diverged". If the other branch has commits of
its own that HEAD does not have, they appear here as *removals*. For the
one-sided view, compare against the merge base with Git directly, or read the
pull request with [`quinjet pr diff`](../pull-request/README.md), which is
merge-base based.

Direction follows from that. Files your branch added are additions, files it
deleted are deletions, and comparing against a branch that is strictly ahead of
you shows its work as removals.

When the two sides are identical, including comparing the current branch with
itself, the document holds one line and the exit code is 0:

```console
$ quinjet branch compare feat/cli-command-surface
No file changes to display
```

Edge cases worth knowing:

- On a detached HEAD the comparison still works. The abbreviated commit id
  stands in for the current branch in the document's title.
- On an unborn branch that has siblings, an orphan checkout before its first
  commit, `HEAD` names nothing and Git refuses:
  `error: Git command failed: fatal: bad revision 'HEAD'`, exit 1. In a
  repository with no refs at all the failure comes earlier, as exit 3, because
  there is nothing to match `<REFERENCE>` against.
- Uncommitted changes are not part of this. The right-hand side is `HEAD`, the
  commit, so anything you have not committed is invisible here. Use
  [`quinjet diff`](../repository/README.md) for that.
- One Git process runs per changed file, serially, so a comparison across 200
  files is 200 processes. The command line always loads every file, because it
  prints a whole document.
- Patches obey the caps in [conventions](../conventions.md#size-caps): 8 MiB per
  file patch, 8 MiB and 16,384 paths for the index. A truncated read ends with
  `[output reached Quinjet's size cap and was truncated]` and sets `truncated`
  in the JSON.
- Because the reads are separate processes, a commit or rebase happening
  elsewhere while `compare` runs can produce a document that mixes two states.
  Nothing detects that.
- The output can be very long. A reader that stops early, `head` for instance,
  closes the pipe under Quinjet, and that is treated as an ordinary end rather
  than a failure: the run stops writing, prints nothing on stderr and exits 0.
  Redirect to a file, or pipe to a pager that drains its input, when you want
  the whole document.

`--json` shape, one object, the same `DiffDocument` every patch-printing verb
in Quinjet emits:

```json
{
  "title": "main -> feat/cli-command-surface -- branch comparison",
  "lines": [
    {
      "kind": "file-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "Cargo.toml  · modified", "foreground": null, "bold": false, "italic": false },
        { "text": "+2", "foreground": null, "bold": false, "italic": false },
        { "text": "-0", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "hunk-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "@@ -20,6 +20,8 @@ crossbeam-channel = \"0.5\"", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "added",
      "oldLine": null,
      "newLine": 24,
      "spans": [
        { "text": "serde_json", "foreground": [191, 97, 106], "bold": false, "italic": false },
        { "text": " ", "foreground": [192, 197, 206], "bold": false, "italic": false },
        { "text": "=", "foreground": [192, 197, 206], "bold": false, "italic": false },
        { "text": " ", "foreground": [192, 197, 206], "bold": false, "italic": false },
        { "text": "\"", "foreground": [192, 197, 206], "bold": false, "italic": false },
        { "text": "1.0", "foreground": [163, 190, 140], "bold": false, "italic": false },
        { "text": "\"", "foreground": [192, 197, 206], "bold": false, "italic": false }
      ]
    }
  ],
  "truncated": false,
  "commitDetails": null,
  "pullRequestDetails": null
}
```

The keys that are not obvious:

- `title` is the compared branch, an arrow, the current branch, a separator and
  the words `branch comparison`. The real output uses a right arrow and a long
  dash; this page writes them as `->` and `--` because it avoids those
  characters. Nothing else prints the title: the human output starts at the
  first file header.
- `kind` is one of `file-header`, `file-footer`, `hunk-header`, `context`,
  `added`, `removed`, `meta`. `meta` is what carries
  `No file changes to display`.
- `spans` exist because Quinjet syntax-highlights patches. A line's text is its
  spans concatenated, except a `file-header`, whose spans are joined with a
  space to make `path  · modified +2 -0`. `foreground` is `[r, g, b]` or `null`.
  Highlighting is skipped above 512 KiB per patch or 32 KiB per line, and then
  every span is plain.
- `oldLine` and `newLine` are the line numbers on each side, `null` where a
  side has none.
- `commitDetails` and `pullRequestDetails` are always `null` for a branch
  comparison. They exist because the same type carries
  [`quinjet show`](../repository/README.md) and
  [`quinjet pr diff`](../pull-request/README.md).

Examples:

```bash
quinjet branch compare main
quinjet branch compare origin/main --expanded
quinjet branch compare refs/remotes/origin/main
quinjet branch compare main --json | jq '[.lines[] | select(.kind == "file-header")] | length'
quinjet branch compare main -C ~/code/project > /tmp/branch.patch
```

```console
$ quinjet branch compare main
.github/labeler.yml  · modified +3 -0
@@ -13,3 +13,6 @@ ui:
 git:
   - changed-files:
       - any-glob-to-any-file: ['src/git/**', 'src/watch.rs']
+cli:
+  - changed-files:
+      - any-glob-to-any-file: ['src/cli/**', 'src/main.rs', 'docs/cli/**']

.github/workflows/wiki.yml  · added +51 -0
@@ -0,0 +1,51 @@
+name: Wiki
```

Each file starts with a header line carrying its path, its status and its
totals, then Git's hunk headers, then the patch body with the usual space, `+`
and `-` prefixes. A blank line separates files. There is no color on the
command line even when stdout is a terminal.

## Where to go next

- [`quinjet branch`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
