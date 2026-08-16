# `quinjet show`

Prints one commit's metadata and the patch it introduced.

Usage:

```bash
quinjet show [REVISION] [--expanded] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[REVISION]` | branch, tag, commit, or any expression Git can resolve to a commit | `HEAD` | The commit to show. A branch or tag shows the commit it points at. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--expanded` | flag | off | Prints whole files instead of three lines of context around each change. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of text. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`show` does not run `git show` to find its commit. It resolves the revision
exactly as [`quinjet log`](./log.md) does, refusing anything empty or starting
with `-` before Git sees it, then reads history at that revision with
`--skip=0 --max-count=1`. The single commit that comes back is the answer, and
it arrives fully parsed, with its parents, its author and committer, its ISO
timestamps, its relative date and its decorations. If resolution fails the verb
exits 3 with `` `<revision>` does not name a commit in this repository `` and
the hint `` run `quinjet log` or `quinjet branch list --all` for what this
repository holds ``. In the narrow case where resolution succeeded but that
one-entry read came back empty, the same sentence is produced as a not-found
failure, again with exit code 3, and without the hint.

That commit is then diffed. For an ordinary commit, and for a merge, the range
is the **first parent** to the commit:

```text
git diff --name-status -z --find-renames <first parent> <commit> --
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 \
    <first parent> <commit> -- [<old path>] <path>
```

A merge therefore prints a `Merge:` line naming every parent but shows only what
the merge changed relative to its first parent, which for a clean merge is
usually the whole of the branch being merged in. There is no combined-diff form
here; `--cc` is used for a conflicted working-tree file, not for a merge commit.

A root commit has no parent, so it takes the only other path:

```text
git diff-tree --root --no-commit-id --name-status -z -r --find-renames <commit> --
git show --format= --no-color --no-ext-diff --find-renames --patch --unified=3 \
    <commit> -- [<old path>] <path>
```

`--root` is what makes `diff-tree` emit the whole tree as additions instead of
nothing, and `--format=` on `git show` suppresses the commit header so that only
the patch reaches the parser. This is one of only two places in Quinjet where
`git show` runs at all; the other is
[`quinjet stash show`](../stash/show.md), which reads the untracked half of a
stash with `git show --format= ... <stash ref>^3 -- <paths>`.

Either way the file list is read first and the totals for its `+n -n` come from
the same command with `--name-status` swapped for `--numstat`, so the two reads
describe exactly the same range. When a file was renamed, both the pre-image and
the post-image path are passed after the `--`, so the patch for a rename is
found even though only one of the two paths exists on each side. Then every file
is loaded, one Git process each, and composed into a single document.

The two caps that bite here are the ones on the index: 8 MiB of `--name-status`
output and 16,384 paths, whichever comes first. A commit that touches more paths
than that has its list cut and the document marked truncated. Each file's patch
is separately capped at 8 MiB.

`--expanded` is `--unified=1000000` on the per-file command, so the whole of
every touched file is printed with the changes in place. It is the same flag,
with the same meaning, as on `diff`, `branch compare` and `stash show`.

The text form is the commit header, then the diff. The header is `commit <full
id>`, a `Merge:` line listing every parent as a full object id when there is
more than one, `Author: <name> <<email>>`, `Date:   <authored timestamp, ISO
8601 with its original offset>`, then the subject indented by four spaces
between blank lines. Only the subject is shown: the record format carries `%s`
and no body, so a commit's extended message is not available from this verb in
either form. The patch that follows is Quinjet's line model, not a Git patch;
see [`quinjet diff`](./diff.md) for what that changes.

`--json` shape, one object with two keys, the commit and its diff document:

```json
{
  "commit": {
    "id": "6ce4acd7ae455e8783860945f574a3d329ff663e",
    "shortId": "6ce4acd",
    "parentIds": [
      "58eae9b05e60ae5cb0d89a7acfad126c58e24931"
    ],
    "author": "github-actions[bot]",
    "authorEmail": "41898282+github-actions[bot]@users.noreply.github.com",
    "authoredAt": "2026-08-15T13:20:42+00:00",
    "committer": "github-actions[bot]",
    "committerEmail": "41898282+github-actions[bot]@users.noreply.github.com",
    "committedAt": "2026-08-15T13:20:42+00:00",
    "relativeDate": "5 hours ago",
    "subject": "chore: release v0.0.6",
    "decorations": ["tag: v0.0.6", "origin/main", "main"]
  },
  "diff": {
    "title": "6ce4acd \u2014 chore: release v0.0.6",
    "lines": [
      {
        "kind": "file-header",
        "oldLine": null,
        "newLine": null,
        "spans": [
          { "text": "Cargo.lock  · modified", "foreground": null, "bold": false, "italic": false },
          { "text": "+1", "foreground": null, "bold": false, "italic": false },
          { "text": "-1", "foreground": null, "bold": false, "italic": false }
        ]
      }
    ],
    "truncated": false,
    "commitDetails": {
      "id": "6ce4acd7ae455e8783860945f574a3d329ff663e",
      "subject": "chore: release v0.0.6",
      "author": "github-actions[bot]",
      "authorEmail": "41898282+github-actions[bot]@users.noreply.github.com",
      "authoredAt": "2026-08-15T13:20:42+00:00",
      "committer": "github-actions[bot]",
      "committerEmail": "41898282+github-actions[bot]@users.noreply.github.com",
      "committedAt": "2026-08-15T13:20:42+00:00"
    },
    "pullRequestDetails": null
  }
}
```

`commit` is the same object [`quinjet log`](./log.md) emits, so the two verbs
can be joined on `id`. `diff` is the same document
[`quinjet diff`](./diff.md) emits, with the same `kind`, `oldLine`, `newLine`
and `spans` rules, except that `commitDetails` is filled here rather than
`null`. It is a deliberate subset of `commit`: the same identity, timestamps and
subject, without `parentIds`, `shortId`, `relativeDate` or `decorations`, and it
exists so the diff document is self-describing when it travels alone. The
separator in `diff.title` is U+2014, written escaped above; the real output
carries the character itself. A commit that touches nothing produces a single
`meta` line reading `No file changes to display` rather than an empty `lines`
array.

Examples:

```bash
quinjet show
quinjet show 6ce4acd
quinjet show v0.0.6 --expanded
quinjet show origin/main --json
quinjet show HEAD~3 --json | jq -r '.commit.subject'
```

```console
$ quinjet show 6ce4acd
commit 6ce4acd7ae455e8783860945f574a3d329ff663e
Author: github-actions[bot] <41898282+github-actions[bot]@users.noreply.github.com>
Date:   2026-08-15T13:20:42+00:00

    chore: release v0.0.6

Cargo.lock  · modified +1 -1
@@ -658,7 +658,7 @@ dependencies = [
 
 [[package]]
 name = "quinjet"
-version = "0.0.5"
+version = "0.0.6"
 dependencies = [
  "anyhow",
  "clap",

Cargo.toml  · modified +1 -1
@@ -1,6 +1,6 @@
 [package]
 name = "quinjet"
-version = "0.0.5"
+version = "0.0.6"
 edition = "2024"
 description = "A fast, live, keyboard-first Git source-control interface for the terminal"
 repository = "https://github.com/pulkitxm/quinjet"
```

The root-commit path, where every line is an addition:

```console
$ quinjet show ba0bcc7a8ca9eaee45380392b2548af179b8123c
commit ba0bcc7a8ca9eaee45380392b2548af179b8123c
Author: T <t@e.com>
Date:   2026-08-16T00:19:11+05:30

    first

a.txt  · added +2 -0
@@ -0,0 +1,2 @@
+hello
+two
```

A merge, showing every parent in the header and the first-parent patch below it:

````console
$ quinjet show 58eae9b
commit 58eae9b05e60ae5cb0d89a7acfad126c58e24931
Merge:  5451c8cc4376a6ea6d8f54043aef5749e262f193 df8b3a85ed92b0b1b8f11daf2e67ce0431a22d44
Author: Pulkit <kpulkit15234@gmail.com>
Date:   2026-08-15T18:49:41+05:30

    Merge pull request #8 from pulkitxm/feat/pr-conversation-live-checks

ARCHITECTURE.md  · modified +15 -5
@@ -20,32 +20,42 @@ Git worker ── Git CLI ── parsed events ──┘
       ▲
       │
 filesystem watcher (coalesced signal)
+webhook listener (loopback, opt-in)
````

## Where to go next

- [`quinjet log`](./log.md) for finding the commit to show
- [`quinjet diff`](./diff.md) for the same document built from the working tree
- [`quinjet status`, `diff`, `log`, `show`](./README.md), the rest of this group
  and revision resolution, the caps and the exit codes
- [All `quinjet` commands](../README.md)
