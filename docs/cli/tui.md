# `quinjet tui`

Opens the terminal interface on a repository, which is also what `quinjet` with
no verb does.

Usage:

```bash
quinjet [PATH] [--no-mouse] [--webhook-listen <ADDRESS>]
quinjet tui [PATH] [--no-mouse] [--webhook-listen <ADDRESS>]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATH]` | path | `.` | Directory to open. Any directory inside a worktree works; Quinjet asks Git for the top level. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--no-mouse` | flag | off | Starts with the mouse released, so the terminal keeps its own selection and copy behavior. Every feature stays reachable from the keyboard. |
| `--webhook-listen <ADDRESS>` | port, or `host:port` | not listening | Binds a loopback HTTP listener. A forwarded GitHub delivery refreshes the open pull request immediately instead of waiting for the next poll. |
| `-C, --path <DIR>` | path | `.` | Global. It selects the repository for a verb, and the interface does not read it: the positional `PATH` is what opens. |
| `--json` | flag | off | Global. Parsed and ignored here, because the interface writes to a screen rather than to stdout. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |
| `-V, --version` | flag | off | Root only. `quinjet --version` prints `quinjet 0.0.6`; `quinjet tui --version` is a usage error. |

## The two spellings

`quinjet` and `quinjet tui` reach the same code. `dispatch` returns
`Launch::Terminal` for both, before any session is built, so no verb machinery
runs and nothing is ever printed on stdout.

The bare positional path exists so that `quinjet ~/code/project` is short. A
verb always wins over a directory of the same name: `quinjet status` runs the
status verb even in a repository containing a directory called `status`. Write
`quinjet ./status` to open that directory instead. The positional path also
stops mattering the moment a verb follows it: in `quinjet ~/code/project status`
the verb takes over and reads `-C`, which is still `.`, so pass
`quinjet -C ~/code/project status` when a verb should run somewhere else.

`quinjet tui` is the form to use in a script, an alias or a shell function,
because it says what it opens and cannot be misread as a path.

## What it refuses

The interface needs a terminal on both ends. When stdin is not a terminal, or
stdout is not, it stops before touching the repository:

```console
$ quinjet tui < /dev/null
error: Quinjet requires an interactive terminal
```

That is exit 1, and it is the only thing the refusal affects. Every verb is
designed for a pipe and none of them checks: `quinjet status | cat` and
`quinjet pr checks 8 --json > checks.json` are ordinary invocations. See
[conventions and contracts](./conventions.md) for the stdout and exit-code
rules a verb obeys.

The repository is discovered next, with `git -C <path> rev-parse
--show-toplevel`. A path that is not inside a worktree fails with
`Not a Git repository` and Git's own message after it. A terminal smaller than
72 columns by 18 rows draws `Terminal too small` and `Resize to at least
72 × 18` rather than a broken layout, and starts drawing again as soon as the
window grows.

## What starts with it

Opening the interface starts a Git worker thread, a recursive filesystem
watcher on the worktree root, and, when asked for, the webhook listener. The
first frame asks for three things and nothing else: the working-tree status,
the current branch's history, and the list of local and remote-tracking
branches. No GitHub request is made at startup, when the Pull Requests tab is
opened, or when a tab is switched. Only typing a number and pressing Enter
contacts GitHub.

The terminal is put into raw mode with the alternate screen, bracketed paste
and, where the terminal supports them, the keyboard enhancement flags that let
Quinjet tell `Esc` apart from an escape sequence. All of it is undone on exit,
including after a panic, because the restore runs in a guard's `Drop`.

## `--no-mouse`, and why releasing the mouse matters

While Quinjet has mouse capture on, the terminal forwards clicks, drags and
wheel events to the application, so the terminal's own text selection never
sees them. That is what makes rows clickable and dividers draggable, and it is
also what stops a normal drag from selecting text.

There are three ways out of that. `--no-mouse` starts with the mouse released.
`m` toggles it at any time and says which way it went:
`Mouse off · select and copy with the terminal, m to restore`. Holding `Shift`
while dragging is the escape hatch most terminals implement themselves, and it
selects text without activating a control.

Releasing the mouse costs nothing else. Clickable rows, group actions and
divider dragging are the only things that stop working, and every one of them
has a key.

## `--webhook-listen`

An open pull request stays current on its own poll. `--webhook-listen` makes it
react the instant GitHub says something happened, by pairing with the GitHub
CLI's forwarder:

```bash
quinjet --webhook-listen 8787
gh webhook forward --repo owner/name --events '*' --url http://127.0.0.1:8787
```

The value is a bare port, which binds `127.0.0.1`, or a full `host:port`.
Anything else fails with `` `<value>` is not a port or a host:port address ``.
Binding happens after the terminal check and after repository discovery, so an
address already in use fails with
`failed to listen for webhooks on 127.0.0.1:8787` followed by the operating
system's reason, and Quinjet exits rather than opening without it.

What the listener accepts is deliberately narrow:

- Connections whose peer address is not loopback are dropped before their
  request is read at all.
- Only a `POST` is a delivery. Anything else is answered `404 Not Found`.
- A delivery is answered `204 No Content`, so `gh webhook forward` does not
  report a failure.
- Headers are read up to 16 KiB and the request body is read up to 4 MiB and
  then discarded. Reads and writes time out after 5 seconds.
- The only thing taken from the request is the `X-GitHub-Event` header, used as
  a label; a delivery without one is recorded as `unknown`.

Nothing from the body is parsed, trusted or displayed. A delivery is a signal
to refresh, not data, which is what makes an unauthenticated loopback listener
safe: the worst a forged request can do is trigger a read that the next poll
would have made anyway. Deliveries arriving together collapse into the single
refresh they would each have asked for, and the mailbox holds 64 of them; when
it is full the extra signal is dropped, because a refresh is already pending.

A delivery bypasses every interval floor and re-reads the checks, the metadata,
the conversation and a running log at once. The footer says so, showing
`webhooks · every 20s`, where the interval is the fallback rather than the
promise.

## How it stays live

Three clocks run, and they are independent.

The filesystem watcher reports every change under the worktree root, ignoring
pure access events, everything under `.git/objects`, `index.lock`, and
`.watchman-cookie-*` files. Event storms coalesce into one signal, because a
refresh reads the complete Git state rather than applying an event. If the
watcher cannot be created, the interface starts anyway and simply relies on the
next clock.

A 10 second tick refreshes the status regardless. It is the safety net for a
watcher that failed to start, a network filesystem that reports nothing, and a
change made by another process in a directory the watcher missed.

An open pull request polls itself. Check state is read every 5 seconds while
any run is in progress, every 20 seconds once everything has settled, and every
2 minutes when the reader is on another tab. That cadence is a ceiling rather
than a schedule: the metadata and the conversation hold their own 20 second
floor and a growing log holds an 8 second floor, so a fast tick costs one extra
request rather than four. A finished run's log is never re-read, because its
archive can no longer change. Each stream is read separately, so one failing
endpoint never stalls the others, and a read that coalesced into a request
still in flight is left unstamped so it is due again next tick.

## `--json` and the interface

There is no JSON shape for this page. `--json` is a global flag, so it parses
here, and it is then ignored: the interface writes to the alternate screen and
never emits a document. Every reading it shows has a verb that does emit one,
which is the point of the table below.

Examples:

```text
quinjet
quinjet ~/code/project
quinjet tui --no-mouse
quinjet tui ~/code/project --webhook-listen 8787
quinjet tui --help
```

```console
$ quinjet tui --help
Open the terminal interface

Usage: quinjet tui [OPTIONS] [PATH]

Arguments:
  [PATH]  Git repository to open [default: .]

Options:
      --no-mouse                  Disable mouse capture
      --webhook-listen <ADDRESS>  Listen for forwarded GitHub webhooks on a port or host:port
  -C, --path <DIR>                Repository to run a subcommand against [default: .]
      --json                      Print one JSON document on stdout instead of text
  -h, --help                      Print help
```

## Every key, and the verb that does the same thing

The interface has no private path to Git. Each of these keys builds a
`cli::Command` and runs it through the same session a verb runs through, so the
right-hand column is not an analogy: it is the same work, printed instead of
drawn. Keys that only move the view have no verb, and say so.

The verbs in the right-hand column are documented in their groups:
[`status`, `diff`, `log`, `show`](./repository/README.md),
[`stage`, `unstage`, `discard`, `commit`, `resolve`](./changes/README.md),
[`branch`](./branch/README.md), [`stash`](./stash/README.md),
[`fetch`, `pull`, `push`, `sync`, `repos`](./remotes/README.md) and
[`pr`](./pull-request/README.md).

| Key | Verb |
| --- | --- |
| `1` | `quinjet status` |
| `2` | `quinjet log` |
| `3` | opens the Pull Requests tab; the lookup itself is `quinjet pr view <n>` |
| `r`, `Ctrl+R` in Changes or History | `quinjet status`, plus `quinjet log` when the branch moved |
| `r` in Pull Requests | `quinjet pr view <n> --refresh` and `quinjet pr checks <n> --refresh` |
| selecting a file in Changes | `quinjet diff <path>` |
| `s`, or `Space` on a file row | `quinjet stage <path>` |
| `u` on a file row | `quinjet unstage <path>` |
| `[+]` / `[−]` click on a file | `quinjet stage <path>` / `quinjet unstage <path>` |
| `[+]` / `[−]` click on a group header | `quinjet stage --all` / `quinjet unstage --all`, scoped to that group |
| `a` | `quinjet stage --all` |
| `U` | `quinjet unstage --all` |
| `x`, then `y` or Enter | `quinjet discard <path> --yes` |
| `c`, then `Ctrl+Enter` | `quinjet commit -m "<message>"` |
| Amend in the command palette | `quinjet commit -m "<message>" --amend` |
| `o` in the conflict modal | `quinjet resolve <path> --ours` |
| `t` in the conflict modal | `quinjet resolve <path> --theirs` |
| `s` or Enter in the conflict modal | `quinjet resolve <path> --stage` |
| `f` | `quinjet fetch` |
| `l` | `quinjet pull` |
| `p` | `quinjet push` |
| `y` | `quinjet sync` |
| `f` or `p` in Pull Requests | nothing. Both show a toast, because reading someone's pull request is not the place to push your branch |
| `d` in Changes | `quinjet branch compare <ref>` |
| `b` outside History, or `B` | `quinjet branch list`; Enter on a row is `quinjet branch switch <name>` |
| `Ctrl+N` in the branch picker | `quinjet branch create <name>` |
| `F2` or `Ctrl+R` in the branch picker | `quinjet branch rename <old> <new>` |
| `Delete` in the branch picker, then confirm | `quinjet branch delete <name> --yes` |
| `b` in History | `quinjet branch list --all`, then `quinjet log <ref>` for the branch chosen |
| selecting a commit in History | `quinjet show <commit>` |
| `C` in History | `quinjet cherry-pick <commit>` |
| `R` in History | `quinjet revert <commit>` |
| `n` in History | `quinjet branch create <name> <commit>` |
| `S` in Changes | `quinjet stash list` |
| Enter in the stash manager | `quinjet stash show <ref>` |
| `Ctrl+N` in the stash manager | `quinjet stash push -m "<message>"` |
| `Ctrl+U` in the stash manager | `quinjet stash push --include-untracked` |
| `Ctrl+S` in the stash manager | `quinjet stash push --staged` |
| `Alt+A` in the stash manager | `quinjet stash apply <ref>` |
| `Alt+P` in the stash manager | `quinjet stash pop <ref>` |
| `Delete` in the stash manager, then confirm | `quinjet stash drop <ref> --yes` |
| `Ctrl+Delete` in the stash manager, then confirm | `quinjet stash clear --yes` |
| `/` in Pull Requests, a number, Enter | `quinjet pr view <n>` |
| `o` in Pull Requests | `quinjet repos`, with the chosen entry becoming `--repo owner/name` |
| `Shift+P` | `quinjet pr conversation <n>` beside `quinjet pr checks <n>` |
| `Shift+F` | `quinjet pr files <n>` |
| selecting a file in the Files tree | `quinjet pr diff <n> <path>` |
| selecting a check | `quinjet pr logs <n> "<check>"` |
| selecting a check that is still running | `quinjet pr logs <n> "<check>" --watch` |
| the pull-request poll itself | `quinjet pr checks <n> --watch` |
| `Shift+O` | `quinjet pr open <n>`, except that a selected check opens that run rather than the pull request |
| `t` or `T` | `--expanded`, on `diff`, `show`, `branch compare` and `stash show`. `pr diff` has no `--expanded`: a pull-request patch is cached per file by its merge-base and head commits at three lines of context, and a second context width would need a second cache key |
| `v` | no verb. The command line prints unified patches only |
| `e` / `E` on a diff | no verb. A verb prints every file it was asked for |
| `e` / `E` in a check log | no verb. `pr logs` prints every step unfolded |
| `Space` on a file header in the preview | no verb |
| `Space`, Enter, `[`, `]` in a check log | no verb. They fold and move between steps |
| `/` elsewhere | no verb. Filtering is a view over the list already read |
| `z`, `Tab`, Enter, `gg`, `G`, `PgUp`, `PgDn`, `h`, `l`, `[`, `]`, arrows, wheel | no verb. Navigation and scrolling |
| `Ctrl+D` / `Ctrl+U` | no verb. Half-page scroll |
| `m` | no verb. Mouse capture belongs to the terminal, not to Git |
| `:` or `Ctrl+P` | no verb. The palette runs the commands above |
| `?` | `--help`, in spirit |
| `Esc` | no verb. Clears a filter, closes a modal, or returns focus |
| `q` | no verb. Exit 0 |

## What only one side can do

The interface can do these, and no verb can:

- Side-by-side diffs (`v`), folding a single file in a multi-file patch, and
  folding one step of a check log. A verb prints the whole thing, unified.
- Filtering a list in place (`/`), the command palette, and the shortcut help.
- Releasing the mouse (`m`) so the terminal can select text.
- Opening the selected check run in a browser (`Shift+O`). `quinjet pr open`
  opens the pull request. The run's URL is the `link` field of
  `quinjet pr checks --json`, so a script can still reach it.
- Showing that an answer came from the cache, and showing which reads are in
  flight.

The command line can do these, and no key can:

- Emit JSON. Every read takes `--json`; the interface has no equivalent.
- `quinjet diff --staged` and `--unstaged`. The Changes tab previews one file
  at a time and has no whole-index patch.
- `quinjet status --watch`, and `quinjet pr checks --exit-code`, which turn a
  reading into a script's control flow.
- Name a check that is not selected. `quinjet pr logs <n> "<name>"` reads any
  run, including by unique partial name; the interface reads the one under the
  cursor.
- `quinjet pr diff <n>` as one document. The interface always loads per file.
- `quinjet log --skip` and `-n`, and `quinjet show <revision>` for any
  revision. History paginates 300 commits at a time from the branch on screen.
- `quinjet cherry-pick` and `quinjet revert` for a commit the History view is
  not listing.
- `-C`, and running at all without a terminal.

## Notes and gotchas

- Exit is 0 when `q` quits, and 1 for the interactive-terminal refusal, a path
  that is not a repository, a `--webhook-listen` address that will not parse or
  will not bind, or a draw that fails.
- `--json` and `-C` are accepted here because clap declares them globally.
  Neither does anything. `-V` is declared on the root only.
- On an unborn branch the branch line reads the branch name with no object id,
  History is empty, and every change is untracked. Staging and committing work;
  the first commit creates the branch.
- On a detached HEAD the branch line shows the first eight characters of the
  commit, `detached` when even that is unknown, and there is no upstream, so
  ahead and behind are both 0. `l`, `p` and `y` will run and Git will refuse
  them; the toast carries Git's message.
- Nothing here writes to the repository on its own. Opening a pull request
  never checks anything out, never creates a ref, and never touches the index.
  Reads run with `GIT_OPTIONAL_LOCKS=0`, so the interface cannot fight a Git
  command running in another window over `index.lock`.
- The filesystem watcher deliberately ignores `.git/objects`, so a fetch that
  writes thousands of loose objects does not cause thousands of refreshes. The
  ref updates that matter are not ignored, and the 10 second tick catches the
  rest.
- A watcher is per worktree root. Changes made inside a submodule, or in a
  linked worktree of the same repository, are seen only by the 10 second tick.
- The cache is shared with the command line. A pull request read on screen is
  already warm for `quinjet pr view`, and the other way round. Its root is the
  first of `$QUINJET_CACHE_DIR`, `%LOCALAPPDATA%\quinjet\cache` on Windows,
  `$XDG_CACHE_HOME/quinjet`, `~/Library/Caches/quinjet` on macOS, and
  `~/.cache/quinjet`, bounded to 128 MiB and 2,048 entries.
- `gh webhook forward` needs its own authenticated `gh` and stays running in
  another window. Quinjet never starts it and never checks that it is there.
  Without it, `--webhook-listen` simply binds a port nothing connects to.
- A listener bound to `0.0.0.0:9000` still accepts loopback connections only.
  The bind address widens what the socket answers on; the peer check does not
  move.
- Keyboard enhancement flags are pushed only when the terminal reports support
  for them. Without them, `Alt` and `Shift` combinations in the stash manager
  and the branch picker depend on what the terminal sends, which differs
  between terminal emulators and between platforms.
- The mouse toggle is best effort. If the terminal rejects the escape sequence,
  the state is left as it was rather than lying about it in the footer.
- Two Quinjets on one repository are fine. They watch the same files, share the
  same cache and take the same locks Git takes.

## Where to go next

- [Conventions and contracts](./conventions.md) for what a verb guarantees and
  the interface does not have to
- [`quinjet pr`](./pull-request/README.md) for the verbs behind the
  pull-request pane, including `--watch`
- [Watching CI from a script](../guides/watching-ci.md) for the same live
  checks without a terminal
- [`quinjet status`, `diff`, `log`, `show`](./repository/README.md) for what
  the first two tabs print
- [All `quinjet` commands](./README.md)
