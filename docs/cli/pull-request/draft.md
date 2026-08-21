# `quinjet pr draft`

Converts an open pull request back to a draft.

```bash
quinjet pr draft <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form runs `gh pr ready --undo`. A draft cannot be merged until it
is marked ready again. GitHub enforces author and repository permission rules.
