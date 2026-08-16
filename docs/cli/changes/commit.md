# `quinjet commit`

Records what is staged as a commit, or replaces the previous one.

Usage:

```bash
quinjet commit [OPTIONS] --message <MESSAGE>

quinjet commit --message <text> [--amend] [-C <DIR>] [--json]
```

Arguments:

This verb takes no positional arguments.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-m`, `--message <MESSAGE>` | string | required | The commit message. Passed to Git as one argument, so it may contain spaces, quotes and newlines. |
| `--amend` | flag | off | Replace the previous commit instead of adding one. |
| `-C, --path <DIR>` | directory | `.` | Repository to run in. |
| `--json` | flag | off | Prints one JSON document on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

The whole verb is `git commit --message <message>`, with `--amend` inserted
before the message when it is asked for. There is no `-a`, no `--allow-empty`,
no `--no-verify`, no `--author` and no `--date`: Quinjet commits what is in the
index and nothing else, so stage first with [`quinjet stage`](./stage.md).

A message that is empty or only whitespace is refused in this process, before
Git is run at all:

```console
$ quinjet commit --message "   "
error: Commit message cannot be empty
```

That exits 1, and it is what stops an unset shell variable from becoming a
commit. Note that the check is `trim`-based but the message is passed through
unmodified, so leading and trailing whitespace you meant to keep is kept. Git
itself then strips comment lines and trailing blank lines as usual, because
`--cleanup` is left at its default.

`--message` is required by clap, so there is no editor to fall into:

```console
$ quinjet commit
error: the following required arguments were not provided:
  --message <MESSAGE>

Usage: quinjet commit --message <MESSAGE>

For more information, try '--help'.
```

That exits 2, and so does passing `--message` twice, which reports
`the argument '--message <MESSAGE>' cannot be used multiple times`. For a
subject and a body, put a newline inside the one argument.

Git is spawned with stdin closed and its output captured. Hooks still run,
because `--no-verify` is never passed, but a hook that tries to ask a question
sees end of file, and a signing key whose pinentry wants the terminal cannot get
it. A hook's output is discarded when it succeeds and becomes part of the error
text when it fails. When Git writes nothing to stderr Quinjet reports its stdout
instead, which is why a commit with nothing staged prints Git's whole status
blurb:

```console
$ quinjet commit --message "fix: nothing at all"
error: Git command failed: On branch main
Changes not staged for commit:
  (use "git add <file>..." to update what will be committed)
  (use "git restore <file>..." to discard changes in working directory)
    modified:   src/main.rs

no changes added to commit (use "git add" and/or "git commit -a")
```

`--amend` replaces the previous commit with a new one that has a new id, so it
rewrites history: amend something already pushed and the next push needs a
force. It also takes whatever is staged now, so `quinjet commit --amend
--message "<the same message>"` is how to fold a forgotten file into the last
commit. On an unborn branch there is nothing to replace, and Git says so:
`error: Git command failed: fatal: You have nothing to amend.`

The sentence is `Commit created`, or `Commit amended` with `--amend`. Neither
prints the new commit id. Read it with `quinjet log -n 1`.

During a conflicted merge, once every conflicted path has been staged or
resolved, `quinjet commit --message ...` is what finishes the merge, exactly as
`git commit` would. For a rebase or a cherry-pick, `git rebase --continue` and
`git cherry-pick --continue` are still yours to run: Quinjet has no verb for
them.

`--json` shape, an object with one key:

```json
{
  "message": "Commit created"
}
```

Examples:

```bash
quinjet commit --message "fix: keep the index lock out of the watcher"
quinjet commit -m "feat: add the pull-request pane"
quinjet commit -m "$(printf 'feat: add checks\n\nPolls until CI settles.')"
quinjet commit --amend -m "fix: keep the index lock out of the watcher"
quinjet commit -m "chore: release" --json -C ~/code/quinjet
```

```console
$ quinjet stage --all
All changes staged

$ quinjet commit --message "fix: let a reader close the pipe without it being an error"
Commit created

$ quinjet log -n 3
696175c  13 seconds ago  Pulkit            fix: let a reader close the pipe without it being an error  (HEAD -> feat/cli-command-surface)
0a9b685  4 minutes ago  Pulkit            docs: generate the wiki from the repository
e2d95c2  10 minutes ago  Pulkit            test: pin the command line's contract
```

## Where to go next

- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](./README.md), the
  rest of this group
- [All `quinjet` commands](../README.md)
