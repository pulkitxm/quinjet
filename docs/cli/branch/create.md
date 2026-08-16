# `quinjet branch create`

Creates a branch and switches to it.

Usage:

```bash
quinjet branch create <NAME> [START] [-C <DIR>] [--json]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `<NAME>` | string | required | The branch to create. Validated before anything is written. |
| `[START]` | revision | current HEAD | The commit to branch from. A branch, tag, commit id, or anything `git rev-parse` understands. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | The repository to act on. Any directory inside the worktree works. |
| `--json` | flag | off | Prints `{"message": ...}` on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Two things happen before Git is asked to create anything.

First, if `[START]` was given, it goes through Quinjet's revision resolution,
the same one [`quinjet log`](../repository/README.md) and `show` use. A name
that resolves to a branch, a remote-tracking branch or a tag becomes its full
ref: `main` becomes `refs/heads/main`, `origin/main` becomes
`refs/remotes/origin/main`, `vX.Y.Z` becomes `refs/tags/vX.Y.Z`. `HEAD` stays
`HEAD`. Anything else is verified as a commit with
`git rev-parse --verify --quiet <start>^{commit}` and becomes a full
forty-character object id, so `HEAD~3` is pinned to one commit before the
branch is made. A start point that resolves to nothing fails with
`` `x` does not name a commit in this repository `` and exits **1**, not 3:
this is the general revision error, not the branch lookup that
[`compare`](./compare.md) does. A start point that is empty or begins with `-`
is refused outright with `` refusing to resolve `-x` as a revision ``.

Second, `<NAME>` is validated. An empty or whitespace-only name fails with
`Branch name cannot be empty`, and everything else is handed to

```bash
git check-ref-format --branch <NAME>
```

so the rules are Git's rules: no space, no `..`, no `~`, `^`, `:`, `?`, `*`,
`[`, no leading or trailing `/`, no trailing `.lock`. Be aware that
`--branch` also *expands* Git's shorthands, so `@{-1}` and `-` pass validation
because Git turns them into a real branch name, and `git switch --create` then
interprets them its own way rather than as literal names.

Only then does Quinjet run

```bash
git switch --create <NAME> [<START>]
```

Because the start point was resolved to a full ref rather than a short name,
tracking still works the way it does with plain Git: creating from
`refs/remotes/origin/main` sets the new branch's upstream to `origin/main`.
Creating from a tag or a raw commit sets no upstream, and there is no `--track`
or `--no-track` flag here to change that.

There is no `--force`. A name that already exists fails with Git's
`fatal: a branch named 'x' already exists` and nothing is created or moved.
Uncommitted changes are carried onto the new branch, exactly as `git switch`
carries them, because the new branch starts at a commit your working tree is
usually already consistent with. A name beginning with `-` is a flag to clap
before it is a name to Git, so pass `--` first.

The order matters when both arguments are wrong: the start point is resolved
first, so `quinjet branch create "bad name" nosuchrev` complains about
`nosuchrev` and never mentions the name.

`--json` shape, an object with one key:

```json
{
  "message": "Created and switched to feat/wiki"
}
```

Examples:

```bash
quinjet branch create feat/wiki
quinjet branch create feat/wiki main
quinjet branch create hotfix/1 origin/main
quinjet branch create archive/old HEAD~3 --json
quinjet branch create -- --odd-name
```

```console
$ quinjet branch create feat/wiki origin/main
Created and switched to feat/wiki
```

```console
$ quinjet branch create "feature branch"
error: Git command failed: fatal: 'feature branch' is not a valid branch name
```

Everything after the `Git command failed:` prefix is Git's own text. Both failures exit 1.

## Where to go next

- [`quinjet branch`](./README.md), the rest of this group
- [All `quinjet` commands](../README.md)
