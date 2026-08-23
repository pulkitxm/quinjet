# `quinjet project list`

Prints recently opened Git projects grouped by their shared Git directory.

Usage:

```bash
quinjet project list [--json]
```

The command reads Quinjet's state index and verifies each entry with Git. Missing
or invalid repositories are skipped. Each project contains its display name,
common Git directory, and complete worktree list.

The JSON form is designed for native clients:

```json
[
  {
    "name": "quinjet",
    "commonDir": "/code/quinjet/.git",
    "worktrees": [
      {
        "path": "/code/quinjet",
        "head": "abcdef0123456789",
        "branch": "main",
        "current": true,
        "bare": false,
        "detached": false,
        "locked": null,
        "prunable": null
      }
    ]
  }
]
```

An empty index prints an empty array in JSON mode and no rows in text mode. Both
forms exit 0. The command does not change the recent-project order or any Git
repository.
