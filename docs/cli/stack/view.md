# `quinjet stack view`

Prints the ordered pull requests in the stack containing one pull request.

Usage:

```bash
quinjet stack view <number> [--repo <owner/name>] [--refresh] [-C <dir>] [--json]
```

Arguments:

| Name | Default | What it does |
| --- | --- | --- |
| `<NUMBER>` | required | Selects a pull request and the stack containing it. |

Options:

| Name | Default | What it does |
| --- | --- | --- |
| `--repo <OWNER/NAME>` | unset | Chooses which discovered repository owns the number. |
| `--refresh` | off | Bypasses cached pull-request metadata before reading the stack. |
| `-C, --path <DIR>` | `.` | Selects the repository. Global. |
| `--json` | off | Prints the complete stack snapshot as JSON. Global. |

The selected pull request is marked with `>` in text output. Each row includes
its one-based position, number, state, title, author, head branch, review
decision, checks state, and mergeability. Warnings report malformed or missing
entries and whether GitHub returned fewer entries than the stack's declared
size.

Examples:

```bash
quinjet stack view 42
quinjet stack view 42 --repo acme/project --refresh
quinjet stack view 42 --json
```

## Where to go next

- [`quinjet stack files`](./files.md) for a composed file list
- [`quinjet stack diff`](./diff.md) for a composed patch
- [`quinjet stack`](./README.md) for range semantics
