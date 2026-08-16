# Conventions and contracts

These are the shared command-line contracts. Repository and GitHub verbs pass
through `cli::dispatch`, `cli::Session`, the `Emitter`, and `cli::Failure`.
`completions`, `man`, `capabilities`, and `update` dispatch before a session is built because
their answers do not require a repository.

## One command layer, two callers

The terminal interface does not have its own path to Git. `src/git/worker.rs`
turns each terminal request into a `cli::Command` and runs it through
`cli::Session::execute`, which is the same call a verb makes. The worker adds
only two things a verb does not need: a generation tag, so an answer to a
question the reader has moved on from is discarded, and a lane, so a slow GitHub
read cannot block a diff.

The practical consequence is that repository and GitHub operations share their
behavior. `quinjet stage` and
pressing `s` on a row build the same `GitOperation::Stage`, run the same
`git add -- <path>`, and produce the same sentence. The interface shows it in a
toast; the command line prints it on stdout.

## `--json` is one document on stdout

`--json` is global. It is declared once on the root command and propagated to
every subcommand, so `quinjet --json status` and `quinjet status --json` are the
same invocation. There is no `-j`.

A verb ends in exactly one write, so one invocation produces one document:

```console
$ quinjet status --json
{
  "branch": {
    "head": "main",
    "oid": "6ce4acd0c1e4c1d0f7a2b3c4d5e6f7a8b9c0d1e2",
    "upstream": "origin/main",
    "ahead": 0,
    "behind": 0,
    "detached": false
  },
  "changes": []
}
```

The shape rules:

- Keys are camelCase, because they are the Rust field names rewritten by one
  `#[serde(rename_all = "camelCase")]` on each type. `originalPath`,
  `shortId`, `startedAt`, `changedFiles`.
- Enum values are lower-case and hyphenated: `"unstaged"`, `"type-changed"`,
  `"file-header"`. They are never the Rust variant name.
- An optional value is emitted as `null` rather than dropping its key, so two
  runs of the same verb have the same keys whether or not the value is there.
- The top level is not always an object. Anything that is a list is a list:
  `log`, `branch list` and `stash list` are arrays. `status`, `diff`,
  `pr view`, `pr checks` and `pr conversation` are objects; a conversation's
  rows live under `entries`, beside `truncated` and `fromCache`.
- Output is pretty-printed with two-space indentation, except under `--watch`.

Mutating verbs answer with the sentence they would have printed:

```console
$ quinjet stage --all --json
{
  "message": "All changes staged"
}
```

### Watching prints a stream

`--watch` breaks the one-document rule on purpose, the way `tail -f` does:

```bash
quinjet pr checks 12 --json                one pretty document, then exit
quinjet pr checks 12 --json --watch        one compact document per read
```

Under `--watch --json` each read is a single line, so
`quinjet pr checks 12 --json --watch | jq .` shows a reading at a time instead
of waiting for a document that never ends. Without `--json`, watching repaints
the screen when stdout is a terminal and simply appends when it is not, so a
redirected watch produces a readable log rather than a file full of escape
sequences.

## stdout is data, stderr is commentary

stdout carries the answer and nothing else: the table, the JSON document, the
patch, the log. stderr carries everything about the answer:

- `error: <message>` for a failure, followed by `hint: <next step>` when there
  is one.
- `warning: <message>` for the non-fatal notes a pull-request lookup collects,
  such as a repository the remote discovery had to skip or a stale cache it fell
  back to.

Interactive stderr may show a spinner while a one-shot command is waiting on
Git, GitHub, or the network. The spinner is cleared before the answer or an
error is written. It is disabled under `--json`, when stderr is redirected, and
during `--watch`, so scripts and logs never receive animation or half-written
lines.

When a verb fails, stdout is empty, so `quinjet pr view 12 --json > out.json`
either writes a whole document or writes nothing. The exceptions are the two
places where a non-zero exit is a verdict rather than a failure:
`quinjet pr checks --exit-code` and `quinjet pr checks --watch` print their
complete document and then exit 1 because a check failed or is still pending.

A closed pipe is not a failure either. When a reader goes away, as in
`quinjet show HEAD | head -20`, the write fails with a broken pipe and Quinjet
exits 0 without printing anything on stderr.

## Exit codes

| Code | Name | Meaning |
| --- | --- | --- |
| 0 | success | The verb did what it was asked. A `--yes`-less destructive verb that deliberately changed nothing is also 0. |
| 1 | failure | The verb was understood and could not finish: Git refused, `gh` failed, a required argument was semantically wrong, or a watched check did not go green. |
| 2 | usage | The command line was wrong in clap's own terms: an unknown flag, a missing argument, a value that will not parse. |
| 3 | not found | The thing you named does not exist or names more than one thing: an unknown branch, stash, revision, path in a pull request, or check name. |
| 4 | unavailable | The thing you named exists but cannot be read: a check run that publishes no GitHub Actions log, or one GitHub has not published anything for yet. |

**0** is what falling off the end of a verb means, and it is also what a
missing `--yes` means. `quinjet discard README.md` with no `--yes` reports what
it would discard, changes nothing, and exits 0, so a missing confirmation is
never an error to handle.

**1** is the default. Any `anyhow` error that is not a `cli::Failure` is
reported and exits 1. It is also the code `quinjet pr checks --watch` and
`quinjet pr checks --exit-code` produce when a check has failed or is still
pending, because a red run is a failure of the thing you asked about rather
than of the command.

**3** is the code for a name, and it always carries a hint listing what you
could have named instead:

```console
$ quinjet pr logs 12 Format
error: `Format` matches more than one check
hint: name one of: Format, lint, and test (ubuntu-latest), Format, lint, and test (macos-latest)
```

A revision that names nothing is the same answer. `log`, `show`, `cherry-pick`,
`revert` and `branch create <START>` all resolve their revision through one
helper, so any of them exits 3 on a name Git cannot resolve, with the hint to
run `quinjet log` or `quinjet branch list --all` for what the repository holds:

```console
$ quinjet show deadbeefdead
error: `deadbeefdead` does not name a commit in this repository
hint: run `quinjet log` or `quinjet branch list --all` for what this repository holds
```

**4** is narrow on purpose. Two things produce it, both when a check run exists
and its log does not. The first is a check whose `link` is not a GitHub Actions
job URL, which is every third-party status context and every merge-queue check:
`<name> does not publish logs through GitHub Actions`. The second is an Actions
job whose steps and archive are both still empty, in the first seconds of a run:
`GitHub has not published anything for this check yet`. Both runs are listed by
`quinjet pr checks`; neither can be read.

## Flags and values

`--path`, spelled `-C` for the muscle memory Git already built, chooses the
repository for every repository or GitHub verb and defaults to the current
directory. Quinjet discovers the worktree root from it, so running from a
subdirectory is the same as running from the top. `completions`, `man`,
`capabilities`, and
`update` accept the global option but do not use it.

```bash
quinjet -C ~/code/project status
quinjet status -C ~/code/project
```

Repository paths for the terminal interface belong to the explicit `tui` verb:
`quinjet tui ~/code/project`. The bare `quinjet` form still opens the current
directory. Any other first word must be a real verb, so a typo is a clap usage
error instead of an attempt to open a similarly named directory.

Boolean flags are presence only. There are no `--no-` inversions. Long options
take their value as the next word or after an equals sign, so `--interval 5`
and `--interval=5` are the same.

`--yes` means the same thing everywhere it appears, on `discard`,
`branch delete`, `stash drop`, `stash clear`, `cherry-pick`, and `revert`:
without it the verb reports what it would do and changes nothing.

`--expanded` means the same thing everywhere it appears, on `diff`, `show`,
`branch compare` and `stash show`: print whole files instead of three lines of
context around each change. It is the `t` key of the terminal interface.

`--refresh` means the same thing on `repos` and on every `pr` verb: ask GitHub
again rather than answer from the on-disk cache. It does not affect entries
whose cache key already names what they contain, because those can never be
stale. See [the caching rules](#what-is-cached).

`--help` and `--version` are generated for every verb and every group, print on
stdout, and exit 0. The root help includes common examples and the web reference.
`completions`, `man`, and `capabilities` generate their output on demand from the
same fully defined clap command tree. `update --check` checks release metadata
without replacing the executable.

## What needs `git`, and what needs `gh`

Repository verbs need `git` on `PATH`. Quinjet never links libgit2 and never
runs a shell: `git` and `gh` receive argument arrays directly, so a branch name
or a path containing a space, a quote or a semicolon is one argument and
nothing else. The metadata verbs run without a repository.

| Verb | Also needs |
| --- | --- |
| `completions`, `man`, `capabilities` | nothing, including no repository and no `git` |
| `update` | network access to GitHub Releases, permission to replace the running executable, and `curl`/`wget` on Unix or PowerShell on Windows; no `git`, `gh`, or Cargo |
| `status`, `diff`, `log`, `show`, `stage`, `unstage`, `discard`, `commit`, `resolve`, `branch`, `stash`, `cherry-pick`, `revert` | nothing |
| `fetch`, `pull`, `push`, `sync` | network and whatever credentials Git is configured with |
| `repos` | `gh`, but only for a host Quinjet cannot recognize locally |
| `pr view`, `files`, `diff`, `conversation`, `checks`, `logs` | `gh`, authenticated |
| `pr open` | a desktop opener: `open`, `xdg-open`, or `explorer` |

`gh` runs with prompts, paging, color and update checks disabled, and with the
repository as its working directory, so `GH_TOKEN`, `GH_HOST` and `GH_REPO`
behave exactly as they do for `gh` itself.

Every Git child gets the same environment, read or write: `LC_ALL=C`,
`GIT_OPTIONAL_LOCKS=0` and `GIT_TERMINAL_PROMPT=0`, with
`-c core.quotepath=false` on the command line. `GIT_OPTIONAL_LOCKS=0` means
running `quinjet status` in a loop never takes `index.lock` for a read and never
fights a Git command you are running in another window. It does not make a write
lock-free: `quinjet stage` still takes Git's own index lock, because it is
changing the index. `GIT_TERMINAL_PROMPT=0` means a fetch that needs credentials
fails rather than blocking on a prompt nobody can answer, and `LC_ALL=C` with
`core.quotepath=false` keeps parsing locale-independent and paths raw.

## Reading a pull request never writes to your repository

Every `pr` verb is read-only with respect to your checkout. Quinjet never
checks out a pull request, never creates a ref in your repository, and never
touches your index or worktree.

When both of the pull request's commits already exist locally it diffs them in
place, which costs no network at all. When they do not, it creates a disposable
bare repository under the cache root, fetches the base and head refs into it
with a blob-filtered shallow fetch whose depth escalates until a merge base is
found, and deletes the whole thing when the session ends.

The one thing to know is that this makes the fetch a per-process cost. The
terminal interface prepares the workspace once and keeps it while you read; a
`quinjet pr diff` prepares it, uses it, and drops it. Repeated `pr diff`
invocations for the same pull request are fast anyway, because the per-file
patches are cached by the merge-base and head commits, but the first one after
a force push pays the fetch again.

## What is cached

The cache root is the first of these that is set: `$QUINJET_CACHE_DIR`, then
`%LOCALAPPDATA%\quinjet\cache` on Windows, then `$XDG_CACHE_HOME/quinjet`, then
`~/Library/Caches/quinjet` on macOS, then `~/.cache/quinjet`. It is created with
owner-only permissions because patches are repository content sitting outside
the repository. It is bounded to 128 MiB and 2,048 entries, pruned oldest
first, and it is shared between the terminal interface and the command line, so
a pull request you opened on screen is already warm on the command line.

Entries are split by whether their key already names what they contain:

| What | Life | Why |
| --- | --- | --- |
| A settled run's steps and log | forever | Keyed by job id. A finished job's output cannot change. |
| A changed-file listing and each file's patch | forever | Keyed by the merge-base and head commits. A new head asks a different question. |
| A pull request's conversation | forever | Keyed by the stamp GitHub moves on any activity. A new comment asks a different question. |
| Repository identity | one day | Remotes change, rarely. |
| Pull-request metadata | five minutes | Title, state and head commit move. |
| The check list | thirty seconds | Moves constantly while CI runs. |
| A running run's log | never cached | Re-reading it is what tails it. |

`--refresh` skips the read for the three timed entries. It does nothing for the
immutable ones, because a stale answer is impossible there.

Credentials are never cached. Patches are.

## Size caps

Nothing here is unbounded, and a read that crosses a cap kills its child process
rather than allocating everything first and truncating afterwards. When a cap is
crossed the output says so explicitly rather than looking complete:

```json
[output reached Quinjet's size cap and was truncated]
```

| What | Cap |
| --- | --- |
| Any patch read | 8 MiB |
| A changed-file index | 8 MiB, 16,384 paths |
| Pull-request metadata | 2 MiB |
| A check run's log | 8 MiB, 200,000 lines |
| A conversation | 500 entries, 64 KiB per body, 8 KiB per quoted hunk |
| Remote discovery | 32 remotes, 64 fetch/push URL entries, 32 distinct URLs, 16 repositories |
| Merge-base deepening | 4,096 commits |
| Syntax highlighting | 512 KiB per patch, 32 KiB per line |

## Where to go next

- [Getting started](./getting-started.md) for the first commands worth running
- [The terminal interface](./tui.md) for the face this page's rules do not
  govern, and the map from its keys to these verbs
- [`quinjet pr`](./pull-request/README.md) for the verbs that need `gh` and the
  ones that watch
- [All `quinjet` commands](./README.md)
