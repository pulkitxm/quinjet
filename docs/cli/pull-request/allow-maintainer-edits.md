# `quinjet pr allow-maintainer-edits`

Lets maintainers edit the head branch of a pull request opened from a fork.

```bash
quinjet pr allow-maintainer-edits <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form calls GitHub's `updatePullRequest` GraphQL mutation with
`maintainerCanModify: true` and the exact pull request node identity. GitHub
enforces that the current viewer owns the forked head branch.
