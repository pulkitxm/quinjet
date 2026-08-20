# `quinjet remove`

Deletes paths from the working tree and, when Git knows them, from the index.
Aliased as `quinjet rm`.

Usage:

```bash
quinjet remove [OPTIONS] [PATHS]...

quinjet remove <path>... [--yes] [-C <DIR>] [--json]
quinjet remove --all      [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATHS]...` | zero or more paths | none | Paths to delete, relative to the repository root. Required unless `--all` is given. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Delete every changed path `quinjet status` reports instead of named paths. Conflicts with `[PATHS]...`. |
| `--yes` | flag | off | Confirm. Without it the command reports what it would remove and changes nothing. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`remove` deletes files. [`discard`](./discard.md) puts a file back the way Git
last saw it; `remove` takes the file away. Both are preview-first, so without
`--yes` the command names what it would touch and exits 0 having changed
nothing:

```console
$ quinjet remove src/main.rs notes.txt
Would remove 2 file(s): src/main.rs, notes.txt. Pass --yes to remove them.
```

Unlike `discard`, the selection is not read from `quinjet status`, so a file
with no pending change is removable too. Each requested path is classified once,
with `git ls-files -z -- <paths>`: a path the index lists, or a directory whose
entries the index lists, is tracked, and everything else is not.

| The path | What runs |
| --- | --- |
| Tracked | `git rm --force -r -- <paths>`, in one call for all of them |
| Untracked | `std::fs::remove_dir_all` when the path is a real directory, `std::fs::remove_file` otherwise |

The untracked deletions happen first, one at a time, then the single `git rm`
runs. Nothing here is atomic: a failure partway through leaves what has already
been deleted deleted.

`--force` is what lets the removal proceed when the file differs from `HEAD` or
from the index, which is the ordinary case for a file you are removing on
purpose. `-r` lets a directory argument take its contents with it. Because Git
records the deletion in the index, a tracked removal shows up as a staged `D`
row in the next status and a commit is all that is left to do:

```console
$ quinjet remove docs/notes.md --yes
1 file removed

$ quinjet status
On branch main

Staged Changes (1)
  D   docs/notes.md
```

A tracked file's content is still in Git's object store afterwards, so
`git checkout HEAD -- <path>` brings back the committed version. An untracked
file's content was never in the object store, and Quinjet unlinks it directly,
so nothing brings that back. The entry is read with `symlink_metadata`, so a
symlink is unlinked rather than followed, and a path outside the repository is
refused rather than resolved.

Repeated paths are collapsed before anything runs, so the count in the sentence
is a count of files and not of status rows. A path that is neither tracked nor
present on disk fails the run:

```console
$ quinjet remove missing.txt --yes
error: failed to inspect missing.txt
```

Giving neither paths nor `--all` is refused before anything runs, and exits 1
with `error: remove needs paths, or --all for every changed file`. Giving both
exits 2. `--all` in a clean repository selects nothing:

```console
$ quinjet remove --all
No files match
```

That is exit 0.

`--json` shape, an object with one key. It carries the report when `--yes` is
absent and the result when it is present, so read the sentence rather than
assuming work happened:

```json
{
  "message": "2 files removed"
}
```

Examples:

```bash
quinjet remove build/output.log
quinjet remove build/output.log --yes
quinjet rm generated --yes
quinjet remove --all
quinjet remove --all --yes --json
```

In the terminal interface this is `Shift+X` on a file row, and **Remove
Selected File** or **Remove Checked Files** in the Changes dropdown.

## Where to go next

- [`quinjet discard`](./discard.md), which restores a file instead of deleting it
- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](./README.md), the
  rest of this group
- [All `quinjet` commands](../README.md)
