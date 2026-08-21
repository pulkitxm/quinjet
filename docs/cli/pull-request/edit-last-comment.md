# `quinjet pr edit-last-comment`

Replaces the current viewer's latest conversation comment.

```bash
quinjet pr edit-last-comment <number> <body> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form runs `gh pr comment --edit-last --body <body>`. GitHub CLI
reports when the viewer has no editable comment.
