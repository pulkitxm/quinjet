# `quinjet pr comment`

Adds a comment to the pull request conversation.

```bash
quinjet pr comment <number> <body> [--repo <owner/name>] [--refresh] [--yes]
```

The body is required and passed directly to `gh pr comment --body`. The command
never opens an editor. Use `edit-last-comment` or `delete-last-comment` for the
latest conversation comment authored by the current viewer.
