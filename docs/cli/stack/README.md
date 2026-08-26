# `quinjet stack`

`quinjet stack` reads the stack containing one pull request and compares any
contiguous range of its entries. Pull-request numbers identify the stack;
one-based positions identify the range inside it.

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

- [`quinjet stack view`](./view.md)
- [`quinjet stack files`](./files.md)
- [`quinjet stack diff`](./diff.md)

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The stack or comparison was printed. |
| 1 | Repository discovery, GitHub lookup, stack parsing, or diff preparation failed. |
| 2 | The command line was invalid. |
| 3 | A position or path did not belong to the selected stack comparison. |

## Where to go next

- [`quinjet pr`](../pull-request/README.md) for one pull request
- [All `quinjet` commands](../README.md)
