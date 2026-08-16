# `quinjet cherry-pick`

Applies one existing commit onto the current branch.

Usage:

```bash
quinjet cherry-pick <REVISION> [--yes] [-C <DIR>] [--json]
```

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<REVISION>` | branch, tag, commit id, or revision | required | Commit to apply. Quinjet resolves it before previewing or mutating. |
| `--yes` | flag | off | Performs the cherry-pick. Without it, reports the resolved revision and changes nothing. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Emits the result sentence in a `message` object. |
| `-h, --help` | flag | off | Prints help and exits 0. |

The default is a safe preview:

```console
$ quinjet cherry-pick feature
Would cherry-pick `refs/heads/feature`. Pass --yes to cherry-pick it.
```

With `--yes`, Quinjet runs `git cherry-pick <resolved revision>` with no shell,
no editor, and stdin closed. Git's configured hooks and identity still apply.

```console
$ quinjet cherry-pick a1b2c3d --yes
Cherry-picked a1b2c3d
```

An unknown or ambiguous revision exits 3 and suggests `quinjet log` or
`quinjet branch list --all`. A conflict or hook failure exits 1 and leaves Git's
normal cherry-pick state in place. Resolve the files and use Git's own
`cherry-pick --continue` or `--abort`; Quinjet does not hide or automatically
finish that state.

JSON success and preview responses have the same shape:

```json
{
  "message": "Cherry-picked a1b2c3d"
}
```

Examples:

```bash
quinjet cherry-pick v1.2.3
quinjet cherry-pick origin/fix --yes
quinjet -C ../project cherry-pick a1b2c3d --yes --json
```

## Where to go next

- [`quinjet revert`](./revert.md) for recording the inverse of a commit
- [`quinjet log`](../repository/log.md) for revisions to name
- [Changing a repository](./README.md), the rest of this group
