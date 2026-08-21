# `quinjet pr auto-merge`

Enables automatic merging after GitHub's required reviews, checks, and other
branch protections pass.

```bash
quinjet pr auto-merge <number> (--merge|--squash|--rebase) [--delete-branch] [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form runs `gh pr merge --auto` with the selected method and the
head commit observed during lookup. GitHub may add the pull request directly to
a required merge queue when its requirements have already passed.

Use [`disable-auto-merge`](./disable-auto-merge.md) to cancel the request.
