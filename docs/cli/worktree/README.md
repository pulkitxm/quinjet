# `quinjet worktree`

`quinjet worktree` is the read-only listing of Git worktrees attached to this
repository. It is the group the terminal interface uses for the Recent projects
picker: one row per checkout, with the branch that is checked out there and a
mark on the tree this session is bound to.

Nothing in this group adds, removes, locks or moves a worktree. Quinjet never
runs `git worktree add` or `git worktree remove`. Switching to another tree is
a session rebind in the terminal interface, the same as `quinjet tui <path>`,
not a Git mutation.

The listing starts with `git worktree list --porcelain -z`. Records are
NUL-separated, so a path can contain any character except NUL. One batched
`git show --no-patch` resolves the listed HEAD commit times. Quinjet then asks
`git rev-parse --git-common-dir` so several trees that share one object store
are one project in the picker rather than several recents.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet worktree list` | Prints every worktree, with its path, branch or detached state, latest commit age, and whether it is this session. |

## Commands

- [`quinjet worktree list`](./list.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | A listing printed, including an empty listing of a repository with only its main tree. Also `--help` on the group or on the verb. |
| 1 | The command ran outside a Git repository. |
| 2 | The command line was wrong in clap's terms: `quinjet worktree` with no verb, an unknown verb, or an unknown flag. |

No verb in this group can exit 3 or 4. Nothing here reads GitHub, so there is
nothing that exists but cannot be read.

## See also

- [`quinjet tui`](../tui.md) for the Recent projects picker that consumes this listing
- [`quinjet branch list`](../branch/list.md) for the branches those trees have checked out
