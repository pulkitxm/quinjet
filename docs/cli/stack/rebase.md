# `quinjet stack rebase`

Fetches the trunk and cascade-rebases each stack branch onto its parent. It can
also continue or abort an interrupted stack rebase.

Usage:

```bash
quinjet stack rebase [<branch>] [--downstack|--upstack] [--no-trunk] [--committer-date-is-author-date] [--git-remote <remote>] [--yes]
quinjet stack rebase <--abort|--continue> [--yes]
```

`--preserve-dates` is an alias for `--committer-date-is-author-date`. Recovery
actions conflict with options that start a new rebase. Without `--yes`, Quinjet
prints a preview and changes nothing.

```bash
quinjet stack rebase api --downstack --yes
```

See [`quinjet stack`](./README.md) for lifecycle safety and prerequisites.
