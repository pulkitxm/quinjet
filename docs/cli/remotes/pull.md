# `quinjet pull`

Brings the current branch up to date with whatever it tracks, exactly as a bare
`git pull` would.

Usage:

```bash
quinjet pull [-C <DIR>] [--json]
```

Arguments: none. `quinjet pull` takes no positional argument, so there is no way
to name a remote or a branch here; it exits 2 if you try.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | Repository to run against. Global, so it may appear before or after the verb. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Underneath this is one child process, and it is as plain as it looks:

```bash
git -C <root> -c core.quotepath=false pull
```

No `--rebase`, no `--no-rebase`, no `--ff-only`, no `--autostash`, no remote and
no refspec. Every decision belongs to your configuration. `pull.rebase` decides
whether the incoming commits are merged or replayed, `pull.ff` decides whether a
merge commit is created when a fast-forward was possible, `rebase.autoStash` and
`merge.autoStash` decide what happens to a dirty working tree, and
`branch.<name>.remote` with `branch.<name>.merge` decide what is pulled at all.
A repository where `git pull` warns about `pull.rebase` not being set produces
that same warning here, though the warning is captured rather than printed
because Git's stderr is only surfaced when the command fails.

`pull` is the only verb in this group that can leave your working tree in a
state you have to deal with. A conflicting merge or a stopped rebase makes Git
exit non-zero, so Quinjet reports the failure and exits 1 without unwinding
anything. The conflicted paths then show up in `quinjet status` and can be
settled with [`quinjet resolve`](../changes/README.md). Quinjet never runs
`git merge --abort` or `git rebase --abort` for you.

The child runs with `GIT_TERMINAL_PROMPT=0` and a closed stdin, so a pull that
needs a password fails instead of hanging. It also means `pull` cannot open an
editor: if your configuration would produce a merge commit whose message needs
editing, set `core.editor` to something non-interactive or expect Git to fail.
Progress output is captured and discarded, so a large pull is silent until it
finishes.

`--json` shape, an object with a single key:

```json
{
  "message": "Pull complete"
}
```

The message is fixed. It says the Git command exited 0, not what it did, so it
is the same sentence whether the branch moved by a hundred commits, was
fast-forwarded by one, or was already up to date. Read `quinjet log -n 5` or
`quinjet status` afterwards if you need to know which.

Examples:

```bash
quinjet pull
quinjet pull --json
quinjet pull -C ~/code/project
quinjet pull && quinjet log -n 5
```

```console
$ quinjet pull
Pull complete
```

```console
$ quinjet pull
error: Git command failed: fatal: Not possible to fast-forward, aborting.
```

## Where to go next

- [`quinjet fetch`, `pull`, `push`, `sync`, `repos`](./README.md), the rest of
  this group
- [`quinjet sync`](./sync.md), which is this verb followed by a push
- [All `quinjet` commands](../README.md)
