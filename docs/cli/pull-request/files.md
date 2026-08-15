# `quinjet pr files`

Lists the files a pull request changes, with each file's status and line counts.

Usage:

```bash
quinjet pr files <number> [--repo <owner/name>] [--refresh] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. `0` is rejected at runtime, a non-integer at parse time. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for the metadata instead of using the five-minute cache. It does not invalidate the file listing, which is keyed by commits. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON object on stdout. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

This is the first verb that needs commits rather than metadata, so it is where
the diff workspace is prepared. If your checkout already contains both
`baseOid` and `headOid`, Quinjet uses your repository directly and finds the
merge base with `git merge-base <base> <head>`. If it does not, it creates a
disposable bare repository, fetches the two refs into it, and finds the merge
base there. Either way, the listing itself is one command:

```bash
git diff --name-status -z --find-renames <merge-base> <head> --
```

A second pass over the same range produces the counts:

```bash
git diff --numstat -z --find-renames <merge-base> <head> --
```

Both are read against the merge base rather than against the base branch tip,
so the listing is what the pull request changed and not what the base has moved
on to since. Both results are cached forever under `pr-files-v1` and
`pr-numstat-v1` keyed by the merge base and head commits, because a listing
between two fixed commits cannot change. A new head asks a different question
and gets a different key rather than a stale answer, which is why `--refresh`
has nothing to do here beyond re-reading the metadata that supplies those two
commits.

The `-z` form is what makes paths safe: records are NUL-separated, so a path
containing a newline, a quote or a tab is one record and nothing else. Renames
and copies occupy three records rather than two, the old path first, and the
listing reports the new path with the old one in `oldPath`. `--find-renames` is
Git's default similarity threshold, so a heavily edited moved file may appear as
a delete plus an add instead.

Statuses are Git's own letters: `A` added, `M` modified, `D` deleted, `R`
renamed, `C` copied, `T` type changed, `U` unmerged, and `?` for anything Git
adds later. The counts column is `+n -m` for a text file and the word `binary`
for a binary one, where `additions` and `deletions` are both `0`.

The read is capped at 8 MiB of NUL-separated output and at 16,384 paths,
whichever comes first, and the cap kills the `git diff` rather than reading it
all and trimming afterwards. When either is crossed, a truncated tail record is
dropped so a half-read path is never reported, `truncated` becomes true, and the
text form ends with an explicit notice giving both numbers:

```json
[the changed-file list reached Quinjet's size cap; 16384 of 41207 shown]
```

The total in that notice is GitHub's own `changedFiles` when it is larger than
what was read, so it is the real size of the pull request rather than the size
of what survived.

`--json` shape, one object. `totalFiles` equals `files.length` unless the
listing truncated, in which case it is the real total. `counts` is `null` for a
file `--numstat` did not report, which happens when the numstat pass itself hit
a cap or failed; it is not the same as a zero count:

```json
{
  "files": [
    {
      "path": "README.md",
      "oldPath": null,
      "status": "modified",
      "counts": { "additions": 42, "deletions": 6, "binary": false }
    },
    {
      "path": "scripts/__pycache__/drive.cpython-312.pyc",
      "oldPath": null,
      "status": "added",
      "counts": { "additions": 0, "deletions": 0, "binary": true }
    },
    {
      "path": "src/git/github/mod.rs",
      "oldPath": "src/git/github.rs",
      "status": "renamed",
      "counts": { "additions": 493, "deletions": 155, "binary": false }
    }
  ],
  "totalFiles": 14,
  "truncated": false
}
```

`status` is a lower-case hyphenated enum: `added`, `modified`, `deleted`,
`renamed`, `copied`, `type-changed`, `unmerged`, `unknown`.

Examples:

```bash
quinjet pr files 8
quinjet pr files 8 --json
quinjet pr files 8 --json | jq -r '.files[].path'
quinjet pr files 8 --repo pulkitxm/quinjet --refresh
```

```console
$ quinjet pr files 8
M ARCHITECTURE.md  +15 -5
M README.md  +42 -6
A scripts/__pycache__/drive.cpython-312.pyc  binary
A scripts/drive.py  +190 -0
M src/app.rs  +1541 -182
M src/git/diff.rs  +260 -2
A src/git/github/checks.rs  +960 -0
A src/git/github/conversation.rs  +439 -0
R src/git/github/mod.rs (from src/git/github.rs)  +493 -155
M src/git/mod.rs  +154 -12
M src/git/worker.rs  +242 -11
M src/main.rs  +89 -7
M src/ui/mod.rs  +1616 -222
A src/webhook.rs  +243 -0
```

Order is whatever `git diff --name-status` produces, which is Git's own path
ordering. Quinjet does not sort, so the list is stable between runs for a fixed
pair of commits.

One thing to know before scripting against this: the counts here disagree with
[`quinjet pr diff`](./diff.md) for renames. This listing sees both paths at once
and reports the rename with its real `+493 -155`; the patch reader restricts
`git diff` to the new path, which hides the rename and prints the file as a
whole-file addition. Take counts from here.

## Where to go next

- [`quinjet pr`](./README.md), the rest of this group and the diff workspace it
  describes
- [`quinjet pr diff`](./diff.md) for the patches behind these paths
- [All `quinjet` commands](../README.md)
