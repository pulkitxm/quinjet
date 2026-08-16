# `quinjet branch rename`

Renames a local branch, keeping everything Git records about it.

Usage:

```bash
quinjet branch rename <OLD> <NEW> [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<OLD>` | string | required | The local branch to rename. Not validated by Quinjet; Git decides whether it exists. |
| `<NEW>` | string | required | The new name. Validated before anything is written. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Any directory inside the worktree works. |
| `--json` | flag | off | Prints `{"message": ...}` on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Two checks run before Git does. `<NEW>` is refused if it is empty or
whitespace, with `Branch name cannot be empty`, and is then put through
`git check-ref-format --branch <NEW>`, so the naming rules are Git's. `<OLD>`
gets neither check: it is a name that has to exist already, and Git's own
lookup is the test of that.

Then the rename is refused if it would change nothing:

```console
$ quinjet branch rename main main
error: New branch name must be different from the current name
```

That refusal is Quinjet's, not Git's. Plain `git branch -m main main` succeeds
and does nothing; this verb treats a no-op rename as a mistake and exits 1.

The rename itself is

```bash
git branch --move -- <OLD> <NEW>
```

`--move`, not `--move --force`, so an existing target name is refused rather
than overwritten. It works whether or not `<OLD>` is the branch you are on: if
it is, HEAD follows the new name and you stay checked out; if it is not,
nothing about your working tree changes.

`git branch --move` is the reason this verb exists rather than a create and
delete pair. It carries the branch's configuration across: `branch.<name>.remote`
and `branch.<name>.merge` are moved to the new section, the reflog moves with
it, and anything else configured under the old name follows. So a renamed
branch still tracks what it tracked.

That is worth reading precisely. Tracking is preserved, which means the
upstream still points at the *old* branch on the remote. A branch that tracked
`origin/topic` and is renamed to `feature/topic` still has
`branch.feature/topic.merge = refs/heads/topic`, so a later
[`quinjet push`](../remotes/README.md) updates `topic` on the server, not
`feature/topic`. Renaming locally never renames anything on a remote, and there
is no verb here that does.

What it will not do:

- A remote-tracking branch cannot be renamed. `quinjet branch rename origin/main x`
  exits 1 with Git's `fatal: no branch named 'origin/main'`, because
  `git branch --move` only looks in `refs/heads`.
- There is no force. An existing target fails with
  `fatal: a branch named 'main' already exists`.
- There is no confirmation. Unlike [`delete`](./delete.md), rename takes no
  `--yes` and acts immediately.

On a case-insensitive filesystem, macOS and Windows by default, a rename that
changes only case is a rename of the same file, and Git may refuse it. That is
a filesystem property, not something Quinjet checks for.

`--json` shape, an object with one key:

```json
{
  "message": "Renamed local branch topic to feature/topic"
}
```

Examples:

```bash
quinjet branch rename topic feature/topic
quinjet branch rename wip/thing feat/thing --json
quinjet branch rename -C ~/code/project old new
```

```console
$ quinjet branch rename topic feature/topic
Renamed local branch topic to feature/topic
```

```console
$ quinjet branch rename nosuch other
error: Git command failed: fatal: no branch named 'nosuch'
```

Everything after the `Git command failed:` prefix is Git's own text. Both failures exit 1.

## Where to go next

- [`quinjet branch`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
