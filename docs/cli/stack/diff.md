# `quinjet stack diff`

Prints the composed patch for a contiguous range of pull requests in one stack.

Usage:

```bash
quinjet stack diff <number> [<path>] [--from <position>] [--to <position>] [--repo <owner/name>] [--refresh] [-C <dir>] [--json]
```

Arguments:

| Name | Default | What it does |
| --- | --- | --- |
| `<NUMBER>` | required | Selects a pull request and the stack containing it. |
| `[PATH]` | unset | Limits output to one exact path in the composed comparison. |

Options:

| Name | Default | What it does |
| --- | --- | --- |
| `--from <POSITION>` | selected entry | Sets the inclusive floor of the comparison. |
| `--to <POSITION>` | selected entry | Sets the inclusive ceiling of the comparison. |
| `--repo <OWNER/NAME>` | unset | Chooses which discovered repository owns the number. |
| `--refresh` | off | Bypasses cached pull-request metadata before reading the stack. |
| `-C, --path <DIR>` | `.` | Selects the repository. Global. |
| `--json` | off | Prints the parsed diff document as JSON. Global. |

The command compares the exact base commit of the floor entry to the exact head
commit of the ceiling entry. Intermediate pull requests are therefore composed
into one patch rather than printed separately. Path matching, batching, output
caps, syntax spans, and JSON shape match
[`quinjet pr diff`](../pull-request/diff.md).

Examples:

```bash
quinjet stack diff 42
quinjet stack diff 42 --from 1 --to 3
quinjet stack diff 42 README.md --from 1 --to 3
quinjet stack diff 42 --from 1 --to 3 --json
```

## Where to go next

- [`quinjet stack files`](./files.md) for the paths in the comparison
- [`quinjet stack`](./README.md) for range semantics
