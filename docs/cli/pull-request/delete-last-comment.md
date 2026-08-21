# `quinjet pr delete-last-comment`

Deletes the current viewer's latest conversation comment.

```bash
quinjet pr delete-last-comment <number> [--repo <owner/name>] [--refresh] [--yes]
```

Quinjet's own `--yes` gate is required before it invokes the GitHub CLI's
non-interactive delete confirmation. GitHub reports when there is no deletable
comment.
