# `quinjet pr disallow-maintainer-edits`

Stops maintainers from editing the head branch of a pull request from a fork.

```bash
quinjet pr disallow-maintainer-edits <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form calls GitHub's `updatePullRequest` GraphQL mutation with
`maintainerCanModify: false` and the exact pull request node identity. GitHub
enforces that the current viewer owns the forked head branch.
