# `quinjet search`

Searches the same lists the terminal interface filters with `/`: working-tree
changes, a bounded page of history, or the files of one prepared pull request.
Name matching is a case-insensitive substring. Contents matching uses
BurntSushi's ripgrep engine (`grep-regex` and `grep-searcher`) in-process, so
the query is a regex and does not spawn `rg`. Invalid regex falls back to a
fixed string. Default mode is Name.

Usage:

```bash
quinjet search <QUERY> [--mode name|contents|both] [--scope changes|history|pull-request]
    [--revision <REVISION>] [--skip <SKIP>] [-n <LIMIT>] [--pr <NUMBER>] [--repo <OWNER/NAME>]
    [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<QUERY>` | string | required | The text to match. Empty is refused by clap. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--mode <MODE>` | `name`, `contents`, `both` | `name` | Name matches list labels. Contents searches file bytes, commit messages, and the selected commit patch. Both is the union. |
| `--scope <SCOPE>` | `changes`, `history`, `pull-request` | `changes` | Which list to search. |
| `--revision <REVISION>` | branch, tag, or commit | `HEAD` | History start. Resolved the same way as `quinjet log`. |
| `--skip <SKIP>` | unsigned integer | `0` | History commits to drop from the front. |
| `-n, --limit <LIMIT>` | unsigned integer | `300` | History page size. `0` means 300. Contents never walks the entire history. |
| `--pr <NUMBER>` | unsigned integer | none | Required for `--scope pull-request`. |
| `--repo <OWNER/NAME>` | string | none | Optional GitHub identity for the pull request. |
| `-C, --path <DIR>` | path | `.` | The repository to read. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of text. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

`changes` reads `status`, then searches those listed paths only. Staged files
are read from the index (`git cat-file blob :0:<path>`). Unstaged, untracked
and conflict files are read from the worktree, capped at 8 MiB. Listed
untracked files are searched even when gitignore would hide a walk of the
tree, because they are already in the change list. The search never walks the
rest of the machine.

`history` reads one page of `log` and, for Contents or Both, the commit
messages of that same page. It does not ripgrep every commit in the
repository. A selected commit patch is only searched from the terminal
interface, which already has a current commit.

`pull-request` looks up the number, prepares the PR workspace, then searches
those file paths in the prepared repository, not the local worktree.

The text form is one matching path or commit id per line. No matches prints
`No matches`. `--json` is the `SearchHits` object: `mode`, `query`, `paths`,
and `commits`.

## Examples

```bash
quinjet search readme
quinjet search TODO --mode contents
quinjet search fix --mode both --scope history -n 50
quinjet search Button --scope pull-request --pr 42 --mode contents
```
