# `quinjet stack`

`quinjet stack` reads stacked pull requests natively and manages their branch
and GitHub lifecycle through the `github/gh-stack` extension. Install and
authenticate the GitHub CLI before running a lifecycle command.

Quinjet asks GitHub for `PullRequest.stack` and `PullRequest.stackEntry` through
GraphQL. The response supplies every entry's exact base and head commit. A range
from position 2 through position 4 compares the base commit of entry 2 directly
to the head commit of entry 4, so intermediate pull requests appear together in
one file list or patch.

If neither range option is present, `files` and `diff` compare only the selected
pull request. `--from` without `--to` compares from that position through the
selected pull request. `--to` without `--from` compares from the selected pull
request through that position. The floor must not be above the ceiling.

The comparison uses exact commit identifiers rather than a merge base. Quinjet
uses the current repository when it contains both commits. Otherwise it prepares
a disposable bare repository in the cache and fetches the required pull-request
refs without changing the caller's worktree, index, branches, or refs.

## Commands

### Inspect

- [`quinjet stack view`](./view.md)
- [`quinjet stack files`](./files.md)
- [`quinjet stack diff`](./diff.md)
- [`quinjet stack gate`](./gate.md)

### Build

- [`quinjet stack init`](./init.md)
- [`quinjet stack add`](./add.md)
- [`quinjet stack checkout`](./checkout.md)
- [`quinjet stack modify`](./modify.md)
- [`quinjet stack unstack`](./unstack.md)

### Publish

- [`quinjet stack link`](./link.md)
- [`quinjet stack merge`](./merge.md)
- [`quinjet stack push`](./push.md)
- [`quinjet stack rebase`](./rebase.md)
- [`quinjet stack submit`](./submit.md)
- [`quinjet stack sync`](./sync.md)

### Navigate

- [`quinjet stack bottom`](./bottom.md)
- [`quinjet stack down`](./down.md)
- [`quinjet stack top`](./top.md)
- [`quinjet stack trunk`](./trunk.md)
- [`quinjet stack up`](./up.md)

Lifecycle commands print a preview unless `--yes` is present. Quinjet disables
terminal prompts and editors when it invokes `gh stack`. Commands that would
open an interactive picker or editor are intentionally absent. Use
`--git-remote` to select a Git remote because the global `--remote` option
selects an SSH machine for Quinjet itself.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The read, preview, or confirmed lifecycle operation succeeded. |
| 1 | Repository discovery, GitHub lookup, stack parsing, diff preparation, or `gh stack` failed. |
| 2 | The command line was invalid. |
| 3 | A position or path did not belong to the selected stack comparison. |

## Where to go next

- [`quinjet pr`](../pull-request/README.md) for one pull request
- [All `quinjet` commands](../README.md)
