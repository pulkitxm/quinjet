# `quinjet fetch`

Updates every remote's tracking refs and deletes the ones whose remote branches
are gone.

Usage:

```bash
quinjet fetch [-C <DIR>] [--json]
```

Arguments: none. `quinjet fetch` takes no positional argument, so a bare word
after it is a usage error and exits 2.

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `-C, --path <DIR>` | path | `.` | Repository to run against. Global, so `quinjet -C ~/code/project fetch` and `quinjet fetch -C ~/code/project` are the same. |
| `--json` | flag | off | Prints one JSON object on stdout instead of the sentence. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |

Underneath this is one child process:

```bash
git -C <root> -c core.quotepath=false fetch --all --prune
```

`--all` means every configured remote, not just the one the current branch
tracks, and it honors `remote.<name>.skipFetchAll` the way Git does. `--prune`
removes remote-tracking refs whose branches no longer exist on the remote, which
is what stops `quinjet branch list --all` filling up with branches that were
merged and deleted months ago. Tags are neither pruned nor forced: no
`--prune-tags`, no `--tags`, no `--force`, so Git's own tag-following rules
apply. Submodules follow `fetch.recurseSubmodules`, because Quinjet passes no
opinion there either.

Nothing in your working tree moves. A fetch changes refs under
`refs/remotes/` and nothing else, so the ahead and behind counts in
`quinjet status` change but the files do not. That makes it the safe half of
`quinjet pull`.

The child process runs with `GIT_TERMINAL_PROMPT=0`, its stdin closed and its
stdout and stderr captured. A fetch that needs a credential Git cannot obtain
fails at once rather than waiting for an answer, and Git's progress meter never
reaches your terminal. Expect no output at all until the verb finishes. On
failure the exit code is 1 and Git's own stderr is appended to the error line.

`--json` shape, an object with a single key, the same sentence the human path
prints:

```json
{
  "message": "Fetch complete"
}
```

Examples:

```bash
quinjet fetch
quinjet fetch --json
quinjet fetch -C ~/code/project
quinjet fetch && quinjet status
```

```console
$ quinjet fetch
Fetch complete
```

```console
$ quinjet fetch
error: Git command failed: fatal: unable to access 'https://github.com/pulkitxm/quinjet.git/': Could not resolve host: github.com
```

## Where to go next

- [`quinjet fetch`, `pull`, `push`, `sync`, `repos`](./README.md), the rest of
  this group
- [`quinjet pull`](./pull.md), which fetches and then integrates
- [All `quinjet` commands](../README.md)
