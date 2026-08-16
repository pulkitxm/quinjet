# `quinjet status`

Prints the branch, its upstream and divergence, and every change in the index,
the worktree and any merge in progress.

Usage:

```bash
quinjet status [--watch] [--interval <SECONDS>] [-C <DIR>] [--json]
```

Arguments: none. `status` takes no positional arguments, and passing one is a
usage error that exits 2 with `error: unexpected argument 'extra' found`.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--watch` | flag | off | Re-reads the repository forever and reprints it, instead of printing once and exiting. Never stops on its own. |
| `--interval <SECONDS>` | unsigned integer | `2` | Seconds to sleep between reads while watching. Values below 1, including `0`, are raised to 1. Ignored without `--watch`. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global, so `quinjet -C x status` and `quinjet status -C x` are the same. |
| `--json` | flag | off | Prints one JSON document on stdout instead of text. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Underneath, this is one Git process:

```text
git -C <root> -c core.quotepath=false status --porcelain=v2 --branch -z \
    --untracked-files=all --ignore-submodules=none
```

Porcelain v2 is chosen because it is stable, machine-first and unlocalised, and
`-z` is chosen because it is the only form that survives a path containing a
newline. Quinjet parses the bytes directly rather than the lines: `# branch.oid`,
`# branch.head`, `# branch.upstream` and `# branch.ab` fill the branch state;
records starting `1` are ordinary changes, `2` are renames and copies whose
original path arrives as the following NUL-separated record, `u` are unmerged
paths, and `?` are untracked ones.

An ordinary record carries two status codes, `X` for the index and `Y` for the
worktree. Every code that is not `.` becomes its own change, so a file staged
and then edited again shows up twice, once as a staged change and once as an
unstaged one. An unmerged record becomes exactly one change in the conflict
area, whatever its codes say.

Changes are then sorted by area, then by path. The areas sort in a fixed order,
conflict before staged before unstaged, and the text output prints them under
`Merge Changes`, `Staged Changes` and `Changes` in that same order with a count
in brackets. Rename and copy changes get a trailing `(from <original path>)` after the
path. A clean tree prints `Working tree clean` after a blank line and nothing
else.

The one-letter codes are Quinjet's, not Git's, and they do not line up with
`git status --short`. `A` added, `M` modified, `D` deleted, `R` renamed, `C`
copied, `T` type changed, `U` **untracked**, and `!` conflicted. Git spells
untracked `??` and uses `U` for unmerged, so read the group heading, not just
the letter.

On an unborn branch the head is the branch name, `oid` is `null`, and the tree
still lists its untracked files. On a detached HEAD the first line becomes
`HEAD detached at <8 characters of the oid>`, `detached` is `true`, and `head`
carries those eight characters rather than a name. `ahead` and `behind` are
`0` unless the branch has an upstream, and when it does they come from Git's own
`# branch.ab` count against the remote-tracking ref on disk, so they are as old
as your last fetch. `status` never fetches.

`--watch` prints a frame, sleeps, and prints another, forever. On a terminal
each frame clears the screen first and is followed by
`watching, refreshing every Ns (Ctrl+C to stop)`; redirected to a file or a
pipe, frames simply append with no escape sequences and no footer. With
`--json` each frame is one compact line, so
`quinjet status --watch --json | jq .` gives a reading at a time.

`--json` shape, an object with two keys:

```json
{
  "branch": {
    "head": "feat/cli-command-surface",
    "oid": "e2d95c224418b5568e27d705e9539daf191519b8",
    "upstream": null,
    "ahead": 0,
    "behind": 0,
    "detached": false
  },
  "changes": [
    {
      "path": ".github/labeler.yml",
      "originalPath": null,
      "area": "unstaged",
      "status": "modified"
    },
    {
      "path": "docs/cli/README.md",
      "originalPath": null,
      "area": "unstaged",
      "status": "untracked"
    }
  ]
}
```

`head` is a branch name, or the first eight characters of the object id when
`detached` is true. `oid` is `null` on an unborn branch and only then.
`upstream` is `null` when the branch tracks nothing, and `ahead`/`behind` are
then both `0`. `originalPath` is non-null only for a rename or a copy, and holds
the pre-image path. `area` is one of `"conflict"`, `"staged"`, `"unstaged"`.
`status` is one of `"added"`, `"modified"`, `"deleted"`, `"renamed"`,
`"copied"`, `"type-changed"`, `"untracked"`, `"conflicted"`. The `changes`
array is already sorted, so it can be compared between runs without
normalizing.

Examples:

```bash
quinjet status
quinjet status --json
quinjet status -C ~/code/project
quinjet status --watch --interval 5
quinjet status --watch --json | jq -c '.branch'
```

```console
$ quinjet status
On branch feat/cli-command-surface

Changes (6)
  M   .github/labeler.yml
  U   .github/workflows/wiki.yml
  M   README.md
  U   docs/cli/README.md
  U   docs/cli/conventions.md
  U   scripts/sync_wiki.py
```

```console
$ quinjet status
HEAD detached at 9f3c1d7e

Working tree clean
```

## Where to go next

- [`quinjet diff`](./diff.md) for the patch behind these rows
- [`quinjet stage`, `unstage`, `discard`](../changes/README.md) for acting on
  them
- [`quinjet status`, `diff`, `log`, `show`](./README.md), the rest of this group
  and the caps, the ordering rules and the exit codes
- [All `quinjet` commands](../README.md)
