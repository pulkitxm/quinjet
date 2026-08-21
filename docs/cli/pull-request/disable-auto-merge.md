# `quinjet pr disable-auto-merge`

Disables automatic merging for one pull request.

```bash
quinjet pr disable-auto-merge <number> [--repo <owner/name>] [--refresh] [--yes]
```

Without `--yes`, the command previews the change. The confirmed form runs
`gh pr merge --disable-auto`. GitHub reports an error when auto-merge is not
active or the viewer cannot change it.
