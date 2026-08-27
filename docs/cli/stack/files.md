# `quinjet stack files`

Lists the files changed by a contiguous range of pull requests in one stack.

Usage:

```bash
quinjet stack files <number> [--from <position>] [--to <position>] [--repo <owner/name>] [--refresh] [-C <dir>] [--json]
```

Options:

| Name | Default | What it does |
| --- | --- | --- |
| `--from <POSITION>` | selected entry | Sets the inclusive floor of the comparison. |
| `--to <POSITION>` | selected entry | Sets the inclusive ceiling of the comparison. |
| `--repo <OWNER/NAME>` | unset | Chooses which discovered repository owns the number. |
| `--refresh` | off | Bypasses cached pull-request metadata before reading the stack. |
| `-C, --path <DIR>` | `.` | Selects the repository. Global. |
| `--json` | off | Prints the changed-file index as JSON. Global. |

The command compares the exact base commit of the floor entry to the exact head
commit of the ceiling entry. Its status letters, rename handling, line counts,
caps, and JSON shape match [`quinjet pr files`](../pull-request/files.md).

Examples:

```bash
quinjet stack files 42
quinjet stack files 42 --from 1 --to 3
quinjet stack files 42 --from 1 --to 3 --json
```

## Where to go next

- [`quinjet stack diff`](./diff.md) for the patches behind these paths
- [`quinjet stack`](./README.md) for range semantics
