# `quinjet pr dequeue`

Removes one pull request from its base branch's merge queue.

```bash
quinjet pr dequeue <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form calls GitHub's `dequeuePullRequest` GraphQL mutation with
the exact node identity returned during lookup. A pull request that is not
queued, or a viewer without permission, is rejected by GitHub.
