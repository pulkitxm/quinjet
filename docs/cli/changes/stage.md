# `quinjet stage`

Adds paths, or every change in the repository, to the index.

Usage:

```bash
quinjet stage [OPTIONS] [PATHS]...

quinjet stage <path>... [-C <DIR>] [--json]
quinjet stage --all      [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATHS]...` | zero or more paths | none | Paths to act on. Each is passed to `git add` as a pathspec, so a directory stages everything under it. Required unless `--all` is given. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--all` | flag | off | Stage every change instead of named paths. Conflicts with `[PATHS]...`. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. Quinjet finds the worktree root from it. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

With paths, Quinjet runs `git add -- <paths>`. The `--` is always there, so a
path beginning with a dash is a path and not an option. With `--all` it runs
`git add -A` with no pathspec at all, which stages the whole worktree:
modifications, additions, deletions and renames, everywhere in the repository,
regardless of where you ran the command from. Ignored files are left alone in
both forms, and `-f` is never passed, so a path listed in `.gitignore` cannot be
staged through Quinjet at all.

Paths are resolved from the repository root, because Git is invoked with
`-C <root>`. `quinjet stage README.md` run from `src/` stages the README at the
top of the tree. `-C` chooses which repository to work in and does not change
what a relative path means inside it.

Giving neither paths nor `--all` is refused in this process, before Git runs:

```console
$ quinjet stage
error: stage needs paths, or --all for every change
```

That exits 1. Giving both is a clap error instead, so it exits 2 with
`error: the argument '--all' cannot be used with '[PATHS]...'`.

The sentence counts arguments, not files. `quinjet stage src` reports
`1 change staged` however many files live under `src`. For paths it is
`1 change staged` or `N changes staged`; for `--all` it is always
`All changes staged`.

Staging a conflicted path is how Git marks it resolved, and `stage` does not
look inside the file first. `quinjet stage --all` during a conflicted merge
therefore marks every conflict resolved with the conflict markers still in
place. Use [`quinjet resolve`](./resolve.md) for conflicts, which at least takes
one path at a time.

Nothing about this verb is confirmed or reversible on its own, but nothing is
lost either: [`quinjet unstage`](./unstage.md) puts it back.

`--json` shape, an object with one key, the same sentence the human form prints:

```json
{
  "message": "1 change staged"
}
```

Examples:

```bash
quinjet stage src/main.rs
quinjet stage src/cli/mod.rs src/cli/render.rs
quinjet stage src
quinjet stage --all
quinjet stage --all --json -C ~/code/quinjet
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

$ quinjet stage README.md docs/notes.md
2 changes staged
```

A `git add` that Git refuses comes back as its own text, prefixed once:

```console
$ quinjet stage nope
error: Git command failed: fatal: pathspec 'nope' did not match any files
```

## Where to go next

- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](./README.md), the
  rest of this group
- [All `quinjet` commands](../README.md)
