# `quinjet unstage`

Takes paths, or everything, back out of the index, leaving the working tree
untouched.

Usage:

```bash
quinjet unstage [OPTIONS] [PATHS]...

quinjet unstage <path>... [-C <DIR>] [--json]
quinjet unstage --all      [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATHS]...` | zero or more paths | none | Paths to act on, as pathspecs, resolved from the repository root. Required unless `--all` is given. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Unstage every change instead of named paths. Conflicts with `[PATHS]...`. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

This verb is the exact inverse of [`quinjet stage`](./stage.md) and it never
touches file contents. What it runs depends on whether `HEAD` resolves, which is
checked first with `git rev-parse --verify HEAD`.

With a commit to compare against, named paths become
`git restore --staged -- <paths>` and `--all` becomes
`git reset --mixed --quiet HEAD --`. On an unborn branch, where there is no
`HEAD` to restore from, named paths become
`git rm --cached --ignore-unmatch -- <paths>` and `--all` becomes
`git rm --recursive --cached .`, whose failure is tolerated when the resulting
status is clean.

Two things follow from that fallback, and both are worth knowing before the
first commit exists. `--ignore-unmatch` means unstaging a path that was never
staged succeeds silently on an unborn branch, while on a normal branch
`git restore --staged` fails on it. And `git rm` accepts `-r` but not
`--recursive`, so the `--all` fallback exits 129 with
`error: unknown option 'recursive'`. Quinjet only tolerates that failure when
the status it reads afterwards is empty, so on an unborn branch with anything
staged the verb fails:

```console
$ quinjet unstage --all
error: Unable to unstage changes: error: unknown option `recursive'
```

Until the repository has its first commit, unstage by name.

`--all` is `git reset`, and `git reset` clears merge state. Running it during a
conflicted merge deletes `MERGE_HEAD`, collapses the conflict's three index
stages into one entry, and leaves the conflict markers in your files as an
ordinary modification. Git then no longer believes a merge is in progress, so
`git merge --continue` will not help you finish. Unstage the paths you meant by
name, or resolve them with [`quinjet resolve`](./resolve.md).

Giving neither paths nor `--all` is refused before Git runs, and exits 1:

```console
$ quinjet unstage
error: unstage needs paths, or --all for every change
```

Giving both exits 2 with
`error: the argument '--all' cannot be used with '[PATHS]...'`.

The sentence counts arguments rather than files: `1 change unstaged` or
`N changes unstaged` for paths, and always `All changes unstaged` for `--all`.
Unstaging a path that has nothing staged is not an error on a normal branch,
because `git restore --staged` succeeds and does nothing; unstaging an untracked
path is, because Git has never heard of it.

`--json` shape, an object with one key:

```json
{
  "message": "All changes unstaged"
}
```

Examples:

```bash
quinjet unstage src/main.rs
quinjet unstage src/cli
quinjet unstage --all
quinjet unstage --all --json
quinjet unstage -C ~/code/quinjet Cargo.lock
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

$ quinjet unstage src/main.rs
1 change unstaged
```

```console
$ quinjet unstage docs/notes.md
error: Git command failed: error: pathspec 'docs/notes.md' did not match any file(s) known to git
```

## Where to go next

- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](./README.md), the
  rest of this group
- [All `quinjet` commands](../README.md)
