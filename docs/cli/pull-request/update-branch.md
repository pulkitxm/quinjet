# `quinjet pr update-branch`

Brings a pull request's head branch up to date with its base branch.

```bash
quinjet pr update-branch <number> [--rebase] [--repo <owner/name>] [--refresh] [--yes]
```

The default merges the base into the head. `--rebase` rebases the head onto the
base instead. GitHub rejects conflicts, deleted head branches, disabled update
buttons, and viewers who cannot update the branch.
