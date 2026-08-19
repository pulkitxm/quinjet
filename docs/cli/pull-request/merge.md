# `quinjet pr merge`

Merges an open pull request with one of GitHub's three merge methods.

Usage:

```bash
quinjet pr merge <number> (--merge|--squash|--rebase) [--delete-branch] [--repo <owner/name>] [--refresh] [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--merge` | flag | one of three required | Create a merge commit. |
| `--squash` | flag | one of three required | Squash commits into one and merge. |
| `--rebase` | flag | one of three required | Rebase commits onto the base branch and merge. |
| `--delete-branch` | flag | off | Ask GitHub to delete the head branch after a successful merge. |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for the metadata rather than using the five-minute cache. |
| `--yes` | flag | off | Confirm. Without it the command reports what it would do and changes nothing. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Exactly one of `--merge`, `--squash`, or `--rebase` is required. Without
`--yes` the verb looks the pull request up, names `#N` and its title, and exits
0 having changed nothing:

```console
$ quinjet pr merge 12 --squash
Would squash and merge #12 (Fix the thing). Pass --yes to merge it.
```

With `--yes` it runs:

```text
gh pr merge <number> --repo <url> --merge|--squash|--rebase [--delete-branch]
```

using the same canonical repository URL every other `pr` verb uses. Failures
from `gh` (draft restrictions, required checks, missing permissions) are
surfaced as exit 1 with the GitHub CLI's message. This verb does not offer
admin merge, auto-merge, or commit-message overrides; those remain `gh`
concerns for now.

`--json` shape, one object with a single key:

```json
{
  "message": "Squashed and merged #12"
}
```

Examples:

```bash
quinjet pr merge 12 --squash
quinjet pr merge 12 --squash --yes
quinjet pr merge 12 --merge --delete-branch --yes
quinjet pr merge 12 --rebase --repo pulkitxm/quinjet --yes --json
```

## Where to go next

- [`quinjet pr close`](./close.md) and [`quinjet pr reopen`](./reopen.md) for the
  other write verbs
- [`quinjet pr view`](./view.md) for the metadata this verb names in its preview
- [`quinjet pr`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
