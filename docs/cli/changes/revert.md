# `quinjet revert`

Records a new commit that reverses one existing commit.

Usage:

```bash
quinjet revert <REVISION> [--yes] [-C <DIR>] [--json]
```

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<REVISION>` | branch, tag, commit id, or revision | required | Commit whose changes should be reversed. |
| `--yes` | flag | off | Performs the revert. Without it, reports the resolved revision and changes nothing. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Emits the result sentence in a `message` object. |
| `-h, --help` | flag | off | Prints help and exits 0. |

The default resolves the name and previews the operation:

```console
$ quinjet revert HEAD~1
Would revert `HEAD~1`. Pass --yes to revert it.
```

With `--yes`, Quinjet runs `git revert --no-edit <resolved revision>`. Git uses
its generated revert message without opening an editor. The original commit
remains in history, and a new commit records its inverse.

```console
$ quinjet revert a1b2c3d --yes
Revert commit created
```

An unknown or ambiguous revision exits 3. A conflict, empty revert, hook
failure, or missing Git identity exits 1 and leaves Git's normal revert state in
place. Resolve or abort it with Git before starting unrelated work.

JSON success and preview responses have the same shape:

```json
{
  "message": "Revert commit created"
}
```

Examples:

```bash
quinjet revert HEAD
quinjet revert release-tag --yes
quinjet -C ../project revert a1b2c3d --yes --json
```

## Where to go next

- [`quinjet cherry-pick`](./cherry-pick.md) for applying an existing commit
- [`quinjet log`](../repository/log.md) for revisions to name
- [Changing a repository](./README.md), the rest of this group
