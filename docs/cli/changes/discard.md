# `quinjet discard`

Throws away changes to paths, or to everything, permanently.

Usage:

```bash
quinjet discard [OPTIONS] [PATHS]...

quinjet discard <path>... [--yes] [-C <DIR>] [--json]
quinjet discard --all      [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATHS]...` | zero or more paths | none | Paths whose changes are thrown away. Matched against the paths `quinjet status` reports, by path component. Required unless `--all` is given. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Throw away every change instead of named paths. Conflicts with `[PATHS]...`. |
| `--yes` | flag | off | Confirm. Without it the command reports what it would discard and changes nothing. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`discard` is the only verb in Quinjet whose work cannot be undone, so it is the
only one that is gated. Without `--yes` it names what it would touch and exits 0
having changed nothing:

```console
$ quinjet discard src/main.rs
Would discard 2 change(s): src/main.rs, src/main.rs. Pass --yes to discard them.
```

A missing confirmation is never an error to handle. The exit code is 0 in both
the reporting case and the doing case, so a script that forgets `--yes` quietly
does nothing rather than failing loudly.

Unlike [`stage`](./stage.md) and [`unstage`](./unstage.md), this verb does not
hand your arguments to Git. It reads `quinjet status`, drops every conflicted
path, keeps the changes whose path starts with one of your arguments, and works
from that list. The match is by path component, so `quinjet discard src` covers
`src/main.rs` and `quinjet discard sr` covers nothing at all. An absolute path
matches nothing either, because status paths are relative to the repository
root.

Because the unit of work is a status row and not a file, a file that is both
staged and modified is two changes. That is why the example above says
`2 change(s)` and names `src/main.rs` twice for one file.

Each selected change takes one of three routes:

| The change | What runs |
| --- | --- |
| Untracked | `std::fs::remove_dir_all` when the path is a real directory, `std::fs::remove_file` otherwise |
| Staged | `git restore --staged --worktree --source=HEAD -- <paths>` |
| Unstaged and tracked | `git restore --worktree -- <paths>` |

The untracked removals happen first, one at a time, then the two `git restore`
calls run in that order. Nothing here is atomic: a failure partway through
leaves what has already been removed removed.

The filesystem removal is the part to be careful about. Quinjet does not run
`git clean`, and an untracked file's contents never enter Git's object store, so
after this there is no reflog entry, no dangling blob and no stash to recover
from. Content that was staged at some point does survive as a loose object, so
`git fsck --lost-found` can sometimes bring back a discarded staged version
before `git gc` runs, but nothing can bring back an untracked file. The entry is
read with `symlink_metadata`, so a symlink is unlinked rather than followed.
Because status is read with `--untracked-files=all`, an untracked directory is
never one change: its files are listed individually and removed one by one, and
the empty directory can be left behind.

Two `git restore` details matter. `--worktree` on its own restores from the
index, not from `HEAD`, so discarding the unstaged half of a partly staged file
gives you the staged version back rather than the committed one. And
`--staged --worktree --source=HEAD` on a file that `HEAD` has never seen, a
staged addition, drops the index entry and deletes the working-tree file.
Discarding a staged new file removes it from disk.

Conflicted paths are filtered out before anything else happens.
`quinjet discard --all` during a conflicted merge leaves every conflict exactly
as it was, and if the conflicts are the only changes there is nothing left to
select:

```console
$ quinjet discard --all
No changes match
```

That is also exit 0. Giving neither paths nor `--all` is refused before the
selection is used, and exits 1 with
`error: discard needs paths, or --all for every change`. Giving both exits 2.

On an unborn branch, discarding anything staged fails, because `--source=HEAD`
has nothing to read:

```console
$ quinjet discard --all --yes
error: Git command failed: fatal: could not resolve HEAD
```

Untracked removals in that same run have already happened by the time Git is
asked.

`--json` shape, an object with one key. It carries the report when `--yes` is
absent and the result when it is present, so read the sentence rather than
assuming work happened:

```json
{
  "message": "2 changes discarded"
}
```

Examples:

```bash
quinjet discard src/main.rs
quinjet discard src/main.rs --yes
quinjet discard docs --yes
quinjet discard --all
quinjet discard --all --yes --json
```

```console
$ quinjet status
On branch main

Staged Changes (1)
  M   src/main.rs

Changes (3)
  M   README.md
  U   docs/notes.md
  M   src/main.rs

$ quinjet discard docs/notes.md
Would discard 1 change(s): docs/notes.md. Pass --yes to discard them.

$ quinjet discard docs/notes.md --yes
1 change discarded
```

The second command deleted the file. There is nothing to run to get it back.

## Where to go next

- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](./README.md), the
  rest of this group
- [All `quinjet` commands](../README.md)
