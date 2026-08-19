# `quinjet pr reopen`

Reopens a closed pull request that has not been merged.

Usage:

```bash
quinjet pr reopen <number> [--repo <owner/name>] [--refresh] [--yes] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NUMBER>` | unsigned integer | required | The pull-request number. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--repo <OWNER/NAME>` | string | unset | Chooses which discovered repository the number belongs to. |
| `--refresh` | flag | off | Asks GitHub again for the metadata rather than using the five-minute cache. |
| `--yes` | flag | off | Confirm. Without it the command reports what it would do and changes nothing. |
| `-C, --path <DIR>` | path | `.` | The repository to run against. Global. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. Global. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Without `--yes` the verb looks the pull request up, names `#N` and its title,
and exits 0 having changed nothing:

```console
$ quinjet pr reopen 12
Would reopen #12 (Fix the thing). Pass --yes to reopen it.
```

With `--yes` it runs `gh pr reopen <number> --repo <url>` against the same
canonical repository URL every other `pr` verb uses. Reopening an open or
merged pull request fails with whatever `gh` reports.

`--json` shape, one object with a single key:

```json
{
  "message": "Reopened #12"
}
```

Examples:

```bash
quinjet pr reopen 12
quinjet pr reopen 12 --yes
quinjet pr reopen 12 --repo pulkitxm/quinjet --yes --json
```

## Where to go next

- [`quinjet pr close`](./close.md) for the inverse
- [`quinjet pr merge`](./merge.md) to land the pull request once it is open again
- [`quinjet pr`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
