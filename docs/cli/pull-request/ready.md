# `quinjet pr ready`

Marks a draft pull request ready for review.

```bash
quinjet pr ready <number> [--repo <owner/name>] [--refresh] [--yes]
```

The confirmed form runs `gh pr ready`. GitHub requests reviews from applicable
code owners when the stage changes. Only an open draft can make this transition.

Use [`draft`](./draft.md) to reverse it.
