# `quinjet resolve`

Takes one side of a merge conflict for one path, or marks that path resolved as
it stands.

Usage:

```bash
quinjet resolve [OPTIONS] <PATH>

quinjet resolve <path> --ours   [-C <DIR>] [--json]
quinjet resolve <path> --theirs [-C <DIR>] [--json]
quinjet resolve <path> --stage  [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<PATH>` | one path | required | The conflicted path, relative to the repository root. Exactly one; there is no way to resolve two paths in one run. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--ours` | flag | off | Keep the version already on this branch. Mutually exclusive with `--theirs` and `--stage`. |
| `--theirs` | flag | off | Keep the version being merged in. Mutually exclusive with `--ours` and `--stage`. |
| `--stage` | flag | off | Accept the file as it stands and stage it. This is the "mark resolved" action. Mutually exclusive with `--ours` and `--theirs`. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

One of the three side flags is required, but clap does not enforce it, so the
refusal is Quinjet's own and exits 1:

```console
$ quinjet resolve f.txt
error: resolve needs one of --ours, --theirs or --stage
```

Two of them is a clap error and exits 2, with
`error: the argument '--ours' cannot be used with '--theirs'`.

`--ours` and `--theirs` run two commands in order:
`git checkout --ours -- <path>` or `git checkout --theirs -- <path>`, then
`git add -- <path>`. The checkout writes one side into the working tree, and the
add is what tells Git the conflict is settled. If the checkout fails, the add
never runs, so a failure leaves the conflict intact. The sentence is
`Accepted --ours for <path>` or `Accepted --theirs for <path>`, with the path as
you typed it.

`--stage` skips the checkout and runs `git add -- <path>` alone, which is how to
keep a file you have edited by hand. Because it is plain staging, its sentence
is the staging one, `1 change staged`, rather than an `Accepted ...` line.
Nothing checks the file first, so `--stage` will happily record a file that
still has `<<<<<<<` in it.

The words mean what Git means by them, which is not always what you expect.
During a merge, "ours" is the branch you are on and "theirs" is the branch being
merged in. During a rebase or a cherry-pick the sides are swapped: "ours" is the
commit being replayed onto, and "theirs" is your own work. Read `quinjet status`
or `git status` first if you are not sure which operation is in progress.

Not every conflict has both sides. A modify/delete conflict, where one branch
changed a file and the other deleted it, has no version on the deleting side,
and Git says so:

```console
$ quinjet resolve g.txt --theirs
error: Git command failed: error: path 'g.txt' does not have their version
```

Resolving that one by taking the deletion means `git rm g.txt`, and Quinjet has
no verb for it.

`--ours` and `--theirs` do not check that the path is conflicted at all, and
they have no `--yes` gate. On a path that merged cleanly, or in a repository
with no merge in progress, `git checkout --ours -- <path>` behaves like
`git checkout -- <path>`: it overwrites the working-tree file from the index and
exits 0. Quinjet then stages it and reports `Accepted --ours for <path>` as if
something had been resolved. Unstaged edits to that file are gone, and unlike
[`quinjet discard`](./discard.md) nothing asked first.

Resolving finishes one path and nothing else. It does not continue the merge,
and Quinjet has no `merge`, `rebase` or `--continue` verb. Once
`quinjet status` shows no more `Merge Changes`, finish a merge with
[`quinjet commit`](./commit.md), and finish a rebase or a cherry-pick with
`git rebase --continue` or `git cherry-pick --continue`.

On screen this is the conflict modal: `o` for ours, `t` for theirs, `s` or
`Enter` for stage. The same three operations, chosen the same way.

`--json` shape, an object with one key. Note that the sentence differs by flag:

```json
{
  "message": "Accepted --ours for src/main.rs"
}
```

Examples:

```bash
quinjet resolve src/main.rs --ours
quinjet resolve Cargo.lock --theirs
quinjet resolve src/cli/mod.rs --stage
quinjet resolve src/cli/mod.rs --stage --json
quinjet resolve -C ~/code/quinjet f.txt --ours
```

```console
$ quinjet status
On branch main

Merge Changes (2)
  !   f.txt
  !   g.txt

$ quinjet resolve f.txt --ours
Accepted --ours for f.txt

$ quinjet status
On branch main

Merge Changes (1)
  !   g.txt
```

`f.txt` is gone from the listing rather than moved to `Staged Changes`, and that
is correct. For a content conflict "ours" is the version `HEAD` already has, so
staging it leaves the index matching `HEAD` and there is nothing to report.
`--theirs` on the same file would have left a `Staged Changes (1)` section
holding `M   f.txt`. Either way the conflict is settled, which is what the
shrinking `Merge Changes` count tells you.

## Where to go next

- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](./README.md), the
  rest of this group
- [All `quinjet` commands](../README.md)
