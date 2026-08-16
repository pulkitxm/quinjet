# `quinjet stash show`

Prints one stash as a patch, without applying it.

Usage:

```bash
quinjet stash show <REFERENCE> [--expanded] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<REFERENCE>` | `stash@{N}` | required | The stash to print. It has to appear in [`stash list`](./list.md); anything else exits 3. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--expanded` | flag | off | Prints whole files instead of three lines of context, by asking Git for `--unified=1000000` rather than `--unified=3`. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the patch. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

There is no path argument. Unlike [`quinjet pr diff`](../pull-request/README.md),
`stash show` cannot be limited to one file: it always prints the whole stash.

The reference is resolved against the list before anything else happens.
Quinjet runs `stash list`, looks for an entry whose `reference` matches the
string exactly, and stops if there is none:

```console
$ quinjet stash show 'stash@{9}'
error: `stash@{9}` does not name a stash in this repository
hint: run `quinjet stash list` for the stashes that exist
```

That is exit 3, and it is the same answer for a reference of the wrong shape
entirely, such as `garbage`, because the list can never contain one. This is
why the `refusing to use an invalid stash reference` failure that
[`apply`](./apply.md) and [`drop`](./drop.md) can produce is unreachable from
`show`.

## What runs underneath

A stash is a commit with two or three parents: `^1` is the HEAD it was taken
from, `^2` is the index at that moment, and `^3`, present only when
`--include-untracked` was used, is a root commit holding the untracked files.
Quinjet reads it in two halves.

The file list comes first, from

```bash
git stash show --name-status -z --include-untracked stash@{1} --
```

with the same call repeated as `--numstat` to fill in the per-file `+n -n`
counts. The numstat read is best-effort: if it fails, the counts render as
`+? -?` and the patch is still printed.

Then, once per file in that list, the tracked half:

```text
git diff --no-color --no-ext-diff --find-renames --patch --unified=3 \
  stash@{1}^1 stash@{1} -- <path>
```

Because the base is `^1`, the patch is the stash's entire change against the
commit it was taken from. Work that was staged and work that was not appear
together, with nothing to distinguish them. A stash taken with `--staged` is
the exception: its tree only ever held the index, so its patch is the staged
change alone.

Untracked files are not in that diff, because they are not in `^1` or in the
stash commit's own tree. `git stash show` can list them with
`--include-untracked` but older Git cannot path-filter it, so Quinjet reads
them from the third parent directly, once per file, and appends the result to
the same document:

```text
git show --format= --no-color --no-ext-diff --find-renames --patch --unified=3 \
  stash@{1}^3 -- <path>
```

A path that is not in the untracked commit simply produces nothing, so the two
halves concatenate cleanly. The untracked read is given whatever is left of the
8 MiB patch cap after the tracked half, and is skipped entirely if the tracked
half already hit it.

That is roughly three Git calls per file plus two for the listing, so a stash
touching twenty files is around sixty invocations. It is not one
`git stash show -p`.

## The third-parent probe, and when this verb fails

Before reading the untracked half, Quinjet asks whether there is one, with
`git rev-parse --verify "stash@{1}^3^{commit}"`, and only runs `git show` when
that succeeds. The nested braces defeat it. On Git 2.43, `stash@{1}^3^{commit}`
is read as a reflog lookup whose selector is everything up to the final `}`,
which is not a number, so Git falls back to a date lookup, warns
`log for 'stash' only goes back to ...` on stderr and returns an entry anyway.
The probe therefore succeeds for every stash, including ones with only two
parents.

The consequence is concrete: `quinjet stash show` currently works for a stash
taken with `--include-untracked` and fails for a plain or `--staged` stash.

```console
$ quinjet stash show 'stash@{0}'
error: Git command failed: fatal: bad revision 'stash@{0}^3'
```

That is exit 1, and it happens after the tracked half was already read, so
nothing partial is printed: on a non-zero exit stdout is empty, as
[conventions](../conventions.md) requires. Until this is fixed, `git stash show
-p 'stash@{0}'` is the way to read such an entry, and `git stash list` plus
`git log --format=%P` is the way to tell which entries have a third parent.

## Output

The text form is the group's patch renderer: one header line per file, then the
hunks, with `+`, `-` and a leading space for context. Each header is the path,
then the status word (`added`, `modified`, `deleted`, `renamed from <old>`,
`copied`, `type changed`, `changed`, plus `binary` when the counts say so),
then the additions and deletions. A blank line separates files. If a cap was
crossed the last line is
`[output reached Quinjet's size cap and was truncated]`.

`--json` shape, one object with the document in it. `lines` is the whole patch
flattened, one entry per rendered line, in print order:

```json
{
  "title": "stash@{1} \u2014 launch work",
  "lines": [
    {
      "kind": "file-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": ".gitignore  · added", "foreground": null, "bold": false, "italic": false },
        { "text": "+1", "foreground": null, "bold": false, "italic": false },
        { "text": "-0", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "hunk-header",
      "oldLine": null,
      "newLine": null,
      "spans": [
        { "text": "@@ -0,0 +1 @@", "foreground": null, "bold": false, "italic": false }
      ]
    },
    {
      "kind": "added",
      "oldLine": null,
      "newLine": 1,
      "spans": [
        { "text": "target", "foreground": [163, 190, 140], "bold": false, "italic": false },
        { "text": "/", "foreground": [163, 190, 140], "bold": false, "italic": false }
      ]
    },
    {
      "kind": "file-footer",
      "oldLine": null,
      "newLine": null,
      "spans": [{ "text": "", "foreground": null, "bold": false, "italic": false }]
    }
  ],
  "truncated": false,
  "commitDetails": null,
  "pullRequestDetails": null
}
```

The keys that are not obvious:

- `title` is the reference, a spaced dash and the stash message. Quinjet emits
  the dash as the raw U+2014 character. The block above writes it as the JSON
  escape `\u2014`, which is the same string, so this page carries none of the
  character itself.
- `kind` is one of `file-header`, `file-footer`, `hunk-header`, `context`,
  `added`, `removed`, `meta`. The text renderer prints `file-header`'s spans
  joined by a space, drops `file-footer` entirely, and prefixes `added`,
  `removed` and `context` with `+`, `-` and a space.
- `spans` is the line split for syntax highlighting, so the line's real text is
  the concatenation of every `text`. Only `file-header` is joined with spaces
  instead. `foreground` is `[r, g, b]` or `null`, which is why a plain diff line
  arrives as several colored fragments.
- `oldLine` and `newLine` are the line numbers in the pre-image and post-image,
  `null` where the line has none.
- `commitDetails` and `pullRequestDetails` are always `null` for a stash. They
  exist because the same document type serves `quinjet show` and
  `quinjet pr diff`.
- `truncated` is `true` when either half crossed a cap.

Examples:

```bash
quinjet stash show 'stash@{0}'
quinjet stash show 'stash@{1}' --expanded
quinjet stash show 'stash@{0}' --json
quinjet stash show 'stash@{2}' -C ~/code/project
```

```console
$ quinjet stash show 'stash@{1}'
.gitignore  · added +1 -0
@@ -0,0 +1 @@
+target/

README.md  · modified +1 -1
@@ -1,5 +1,5 @@
 one
 two
-three
+THREE
 four
 five
```

The first file came from the untracked half and the second from the tracked
half. The two are printed as one patch, in the order the file listing gave.

## Where to go next

- [`quinjet stash`](./README.md), the rest of this group
- [`quinjet stash list`](./list.md) for the references this verb accepts
- [`quinjet diff` and `quinjet show`](../repository/README.md) for the same
  renderer over the working tree and over commits
- [All `quinjet` commands](../README.md)
