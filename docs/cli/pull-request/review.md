# `quinjet pr review`

Submits a review verdict or review comment.

```bash
quinjet pr review <number> (--approve|--comment|--request-changes) [--body <text>] [--repo <owner/name>] [--refresh] [--yes]
```

Exactly one verdict is required. The optional body is passed as one argument,
with no editor or prompt. GitHub decides whether the viewer may approve their
own pull request, request changes, or review the current state.
