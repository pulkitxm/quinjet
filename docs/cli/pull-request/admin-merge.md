# `quinjet pr admin-merge`

Merges an open pull request with administrator privileges.

```bash
quinjet pr admin-merge <number> (--merge|--squash|--rebase) [--delete-branch] [--repo <owner/name>] [--refresh] [--yes]
```

Exactly one merge method is required. Without `--yes`, the command previews the
action. The confirmed form runs `gh pr merge --admin` and pins the head commit
observed during lookup. GitHub rejects the action unless the viewer can bypass
the relevant branch protections or merge queue.

See [`quinjet pr merge`](./merge.md) for shared repository and output behavior.
