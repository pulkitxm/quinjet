# `quinjet pr revert`

Creates a new pull request that reverts a merged pull request.

```bash
quinjet pr revert <number> [--title <text>] [--body <text>] [--draft] [--repo <owner/name>] [--refresh] [--yes]
```

GitHub supplies a default title and body when they are omitted. `--draft`
creates the revert as a draft. The source pull request must be merged, its
changes must still be revertible, and the viewer must be able to create the new
branch and pull request.
