# `quinjet push`

Sends the current branch to its upstream, or to `origin` while setting an
upstream when it does not have one yet.

Usage:

```bash
quinjet push [-C <DIR>] [--json]
```

Arguments: none. `quinjet push` takes no positional argument. There is no
remote, no refspec and no `--force`; `quinjet push origin` exits 2.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | Repository to run against. Global, so it may appear before or after the verb. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

This is the one verb in the group with a decision in it. Quinjet reads the
status first:

```bash
git -C <root> -c core.quotepath=false status --porcelain=v2 --branch -z --untracked-files=all --ignore-submodules=none
```

If the branch header reports an upstream, the push is plain:

```bash
git -C <root> -c core.quotepath=false push
```

which means `push.default`, `branch.<name>.remote` and `remote.<name>.push`
decide where the commits go and how many refs travel. Quinjet adds nothing.

If there is no upstream, Quinjet checks that an `origin` remote exists before
inventing one:

```bash
git -C <root> -c core.quotepath=false remote get-url origin
git -C <root> -c core.quotepath=false push --set-upstream origin HEAD
```

`get-url` is only a probe. Its output is thrown away and only its exit status is
read, so a remote configured with an unreachable URL still counts as existing.
If it exits non-zero the verb stops there and never contacts a network:

```console
$ quinjet push
error: Current branch has no upstream and no `origin` remote exists
```

That refusal is deliberate. A repository whose only remote is called `upstream`
or `fork` is not guessed at, because a first push is the moment a branch's home
is chosen and choosing it wrongly is expensive to undo. Configure the upstream
with Git once, and every later `quinjet push` takes the plain path.

`HEAD` in the set-upstream form is what makes the destination branch take the
current branch's name. It also means a detached HEAD fails: there is no upstream,
`origin` usually exists, and Git then refuses because `HEAD` does not name a
branch it can push to. An unborn branch fails for the neighboring reason, that
there is no commit to send.

The child runs with `GIT_TERMINAL_PROMPT=0` and a closed stdin, so a push that
needs credentials fails rather than blocking, and Git's progress and its
`remote:` lines are captured rather than printed. A rejected push, the
`fetch first` case, is a plain exit 1 carrying Git's own explanation.

`--json` shape, an object with a single key. The sentence is the same on both
paths, so it does not tell you whether an upstream was created:

```json
{
  "message": "Push complete"
}
```

Examples:

```bash
quinjet push
quinjet push --json
quinjet push -C ~/code/project
quinjet commit -m "fix: prune stale tracking refs" && quinjet push
```

```console
$ quinjet push
Push complete
```

```console
$ quinjet push
error: Git command failed: ! [rejected]        main -> main (non-fast-forward)
```

## Where to go next

- [`quinjet fetch`, `pull`, `push`, `sync`, `repos`](./README.md), the rest of
  this group
- [`quinjet branch`](../branch/README.md) for the upstream this verb creates and
  the ahead and behind counts it clears
- [All `quinjet` commands](../README.md)
