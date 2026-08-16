# `quinjet fetch`, `pull`, `push`, `sync`, `repos`

These five verbs are the only ones in Quinjet that reach past your checkout.
Four of them move commits: `fetch` updates the remote-tracking refs, `pull`
brings the current branch up to date, `push` sends it, and `sync` does the last
two in order. The fifth, `repos`, moves nothing. It answers the question every
`pr` verb has to answer first: which GitHub repositories does this working copy
actually point at, and which remote name belongs to each one.

The four moving verbs are deliberately thin. Quinjet does not own the policy
here, your Git configuration does. `fetch` runs `git fetch --all --prune`,
`pull` runs a plain `git pull` with no `--rebase`, no `--ff-only` and no
`--no-rebase`, and `push` runs a plain `git push` when the branch already has an
upstream. Whether a pull merges or rebases, whether it fast-forwards, which
remote a bare `push` goes to and how many branches it carries are all decided by
`pull.rebase`, `pull.ff`, `branch.<name>.remote`, `push.default` and the rest of
your configuration, exactly as they would be if you had typed the Git command
yourself. Nothing here is an opinionated wrapper, and there is no flag to make
it one.

`push` is the single exception, and only when there is nothing to be plain
about. It reads the status first. If the current branch has an upstream it runs
`git push` and stops there. If it does not, it checks that an `origin` remote
exists with `git remote get-url origin`, and then runs
`git push --set-upstream origin HEAD`, so a branch created locally goes to
`origin` and gains a tracking ref in one step. If that check fails the verb
refuses rather than guessing at another remote, with the message
"Current branch has no upstream and no `origin` remote exists". `sync` is
`git pull` followed by that same push routine, so it inherits both behaviors,
including the refusal.

Every Git child process in Quinjet runs with `GIT_TERMINAL_PROMPT=0`, and this
group is where that matters. A fetch over HTTPS that has no usable credential
fails immediately with `terminal prompts disabled` instead of stopping to ask a
question that a pipe, a CI job or a repainting terminal interface cannot answer.
Quinjet also captures the child's stdout and stderr and closes its stdin, so
Git's progress meter never appears: a `quinjet fetch` prints nothing at all
while it works, and then prints one sentence. Silence is the normal state, not
a sign that it has hung.

`repos` is a different kind of command. It runs `git remote`, reads every
configured fetch and push URL for each remote, strips any credentials, and turns
each distinct URL into a repository identity. A `github.com` URL is recognized
locally with no network call at all, which is why `quinjet repos` in an ordinary
GitHub checkout is instant and works offline. Anything else, including every
GitHub Enterprise host, is handed to `gh repo view <url>`, whose answer is
cached for a day. The result is sorted with the repository that owns `origin`
first, which is the repository a `quinjet pr` command uses when you do not pass
`--repo`.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet fetch` | Runs `git fetch --all --prune`: updates every remote's tracking refs and deletes the ones whose branches are gone. |
| `quinjet pull` | Runs a plain `git pull` on the current branch, merging or rebasing according to your configuration. |
| `quinjet push` | Pushes the current branch, setting `origin` as its upstream when it does not have one. |
| `quinjet sync` | Pulls, then pushes, using the same logic as the two verbs above. |
| `quinjet repos` | Lists the GitHub repositories this checkout's remotes resolve to, with the remote names that reach each one. |

## Commands

- [`quinjet fetch`](./fetch.md)
- [`quinjet pull`](./pull.md)
- [`quinjet push`](./push.md)
- [`quinjet sync`](./sync.md)
- [`quinjet repos`](./repos.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | The Git command succeeded, or the listing printed. Also `--help` on any of the five verbs. A `fetch` that had nothing to fetch and a `push` that reported `Everything up-to-date` are both 0. |
| 1 | Git exited non-zero for `fetch`, `pull`, `push` or `sync`: no network, a rejected non-fast-forward push, a merge conflict left by `pull`, a credential Git could not obtain. Also `push` and `sync` when the branch has no upstream and no `origin` remote exists, `repos` when no remote resolves to GitHub and `gh` cannot infer one either, and any of the five when `-C` does not name a Git repository. |
| 2 | The command line was wrong in clap's own terms, such as `quinjet fetch --all`. None of these verbs takes a positional argument, so `quinjet push origin` is a usage error too. |

Codes **3** and **4** are not reachable from this group. Nothing here names a
branch, a stash, a commit or a check run, so there is nothing to fail to find,
and nothing here reads a check run's log.

## Notes and gotchas

- No verb in this group takes `--yes`, and none of them asks. `push` and `sync`
  send commits the moment they are run. The guard rails are Git's own: a
  non-fast-forward push is rejected by the remote, not by Quinjet.
- Nothing in this group is watchable. `--watch` exists on `status`,
  `pr checks` and `pr logs` only, so there is no built-in way to poll a remote.
  A shell loop around `quinjet fetch` is the honest way to do it.
- There is no `--remote`, no `--branch`, no `--force`, no `--tags` and no
  `--rebase` anywhere in this group. If you need one, run Git directly. The
  vocabulary here is deliberately the set of operations the terminal interface
  exposes, and the interface has four buttons, not a form.
- `fetch` is the only verb that touches every remote. `pull` and `push` touch
  whichever single remote your configuration points the current branch at.
- Git's progress output is discarded, not forwarded. Quinjet captures the child
  process's stdout and stderr, prints one sentence on success, and on failure
  prints Git's stderr as the tail of an `error:` line. Under `--json` the same
  sentence arrives as `{"message": "..."}`.
- The child's stdin is closed, and `GIT_TERMINAL_PROMPT=0` is set, so Git's own
  username and password prompt fails rather than blocking. This does not cover
  everything: a passphrase-protected SSH key is prompted for by `ssh` on the
  controlling terminal, which is outside Git's control. Use an agent, or expect
  a prompt to appear over whatever Quinjet was drawing.
- Credential helpers still work normally, because the child inherits the rest of
  the environment. `SSH_AUTH_SOCK`, `GIT_SSH_COMMAND`, `credential.helper` and
  `gh auth setup-git` all behave as they do for a hand-typed Git command.
- `GIT_OPTIONAL_LOCKS=0` is set on these commands too, alongside `LC_ALL=C` and
  `-c core.quotepath=false`. It only suppresses sub-operations Git considers
  optional, such as refreshing the index; the ref updates a fetch performs are
  not optional and still happen.
- The output of a fetch is buffered whole in memory before it is discarded.
  These four verbs use the unbounded child-process path rather than the capped
  one the read verbs use, so the size caps listed in
  [conventions and contracts](../conventions.md) do not apply here.
- On a detached HEAD, `fetch` works normally and `pull` behaves as Git does.
  `push` sees no upstream, finds `origin`, and then fails, because
  `git push --set-upstream origin HEAD` cannot name a destination branch from a
  detached HEAD. The error is Git's own, prefixed with `Git command failed`.
- On an unborn branch, that is a fresh repository with no commit yet, `push` and
  `sync` fail the same way: there is no upstream, `origin` may well exist, and
  `git push --set-upstream origin HEAD` has no commit to send.
- Quinjet finds the repository with `git rev-parse --show-toplevel`, so a bare
  repository is rejected before any of these verbs runs, and running from a
  subdirectory is the same as running from the top.
- `pull` can leave the working tree mid-merge. Quinjet reports Git's failure and
  exits 1, and the conflicted files are then visible in `quinjet status` and
  fixable with [`quinjet resolve`](../changes/README.md). Nothing is aborted for
  you.
- These four verbs are also four keys in the terminal interface: `f` fetches,
  `l` pulls, `p` pushes and `y` synchronizes, and the command palette lists them
  as Fetch All Remotes, Pull, Push and Synchronize (Pull, Then Push). In the
  pull-request view `f` and `p` are refused with a note, because that view uses
  those letters for something else. All four run through the same command layer
  the verbs use.
- `repos` never derives an identity for a non-`github.com` host by itself. Every
  Enterprise remote costs one `gh repo view` on a cold cache, so a checkout with
  several Enterprise remotes is noticeably slower than a `github.com` one the
  first time it is listed each day.
- `repos` warnings are part of the listing on stdout, printed as trailing
  `warning:` lines and repeated in the `warnings` array under `--json`. This is
  the one place where a warning is not on stderr; the `pr` verbs print theirs on
  stderr, as [conventions and contracts](../conventions.md) describes.
- Remote discovery is capped at 32 remotes, 64 fetch and push URL entries, 32
  distinct URLs and 16 repositories. Each cap that is reached adds its own
  warning rather than silently shortening the list. See
  [`quinjet repos`](./repos.md) for the exact sentences.
- The caps and the ordering apply to `quinjet pr` too, because every `pr` verb
  runs the same discovery to find the repository a number belongs to.
- Repository identity is cached for one day under the cache root, keyed by the
  credential-stripped remote URL. `--refresh` skips reading that entry but still
  writes the fresh one. It has no effect on a `github.com` remote, which is
  never cached because it is never asked about.
- If `gh` fails and a cached identity exists, `repos` answers from the cache and
  says so with `Using stale cached GitHub identity for remote <name>`, and exits
  0. It only fails when there is nothing cached to fall back to.
- On Windows the cache lives under `%LOCALAPPDATA%\quinjet\cache`; elsewhere it
  follows `$QUINJET_CACHE_DIR`, `$XDG_CACHE_HOME/quinjet`, then `~/.cache`.
  Nothing else in this group is platform-specific.

## Where to go next

- [`quinjet repos`](./repos.md) for the discovery algorithm in full, including
  what happens to a URL that carries a token
- [`quinjet pr`](../pull-request/README.md) for the verbs that consume what
  `repos` finds
- [`quinjet branch`](../branch/README.md) for the upstream a `push` creates and
  the ahead and behind counts a `fetch` refreshes
- [Conventions and contracts](../conventions.md) for the `--json` guarantee, the
  exit-code table and the caching rules these pages refer to
- [All `quinjet` commands](../README.md)
