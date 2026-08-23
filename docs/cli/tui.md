# `quinjet tui`

Opens the terminal interface on a repository, which is also what `quinjet` with
no verb does.

Usage:

```bash
quinjet
quinjet [--pr <NUMBER>]
quinjet --remote <SSH_TARGET> --folder <DIR>
quinjet [--pr <NUMBER>] [--client <CLIENT>]
quinjet tui [PATH] [--pr <NUMBER>] [--client <CLIENT>] [--theme <THEME>] [--appearance <APPEARANCE>] [--no-mouse] [--webhook-listen <ADDRESS>]
```

Arguments:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `[PATH]` | path | `.` | Directory to open. Any directory inside a worktree works; Quinjet asks Git for the top level. |

Options:

| Name | Type / values | Default | What it does |
| --- | --- | --- | --- |
| `--theme <THEME>` | `quinjet`, `catppuccin`, `dracula`, `everforest`, `gruvbox`, `nord`, `one`, `rose-pine`, `solarized`, `tokyo-night`, `ayu`, `monokai`, `github` | `quinjet` | Selects one unified palette for every surface, state, diff background, status color, and syntax token. |
| `--appearance <APPEARANCE>` | `system`, `light`, `dark` | `system` | Selects the light or dark variant. `system` detects the operating-system preference once during startup. |
| `--no-mouse` | flag | off | Starts with the mouse released, so the terminal keeps its own selection and copy behavior. Every feature stays reachable from the keyboard. |
| `--webhook-listen <ADDRESS>` | port, or `host:port` | not listening | Binds a loopback HTTP listener. A forwarded GitHub delivery refreshes the open pull request immediately instead of waiting for the next poll. |
| `--pr <NUMBER>` | unsigned integer | unset | Opens the interface already focused on this pull request: the Pull Requests tab is selected and the lookup starts before the first frame. Also accepted on a bare `quinjet` launch. With any other verb it is an error. |
| `-C, --path, --folder <DIR>` | path | `.` | Selects a repository when the positional `PATH` is omitted. |
| `--remote <SSH_TARGET>` | SSH target | local machine | Runs the interface on the SSH machine and allocates a remote terminal. |
| `--ssh-control-path <PATH>` | path | unset | Reuses an existing SSH control socket for the remote session. |
| `--client <CLIENT>` | `edith` | unset | Runs inside a supported embedding client. `edith` delegates project and worktree navigation to Edith. |
| `--json` | flag | off | Global. Parsed and ignored here, because the interface writes to a screen rather than to stdout. |
| `-h, --help` | flag | off | Prints this verb's help on stdout and exits 0. |
| `-V, --version` | flag | off | Prints the installed version and exits 0. |

When the interface runs through `--remote`, the project picker displays the
active SSH target alongside recent targets ordered by use count. `Tab` moves
directly to the next reachable machine and `Shift+Tab` moves to the previous
one. Unavailable targets are shown but skipped. SSH machine switching is
available only from this project picker.

## The two spellings

`quinjet` and `quinjet tui .` reach the same code. `dispatch` returns
`Launch::Terminal` for both, before any session is built, so no verb machinery
runs and nothing is ever printed on stdout.

The no-argument form opens the current directory. A path can use the explicit
verb, as in `quinjet tui ~/code/project`, or the global folder option, as in
`quinjet --folder ~/code/project`. This keeps the root command unambiguous:
`quinjet statsu` is an unknown verb with exit 2 rather than a request to open a
directory named `statsu`.

## Themes and appearance

Every palette has a complete light and dark variant. The selected palette owns
all colors in the interface, including the root background, panels, borders,
selection, focus, status and feedback colors, diff rows, intraline emphasis,
and syntax highlighting. No view keeps a separate fixed color scheme.

The palettes are `quinjet`, `catppuccin`, `dracula`, `everforest`, `gruvbox`,
`nord`, `one`, `rose-pine`, `solarized`, `tokyo-night`, `ayu`, `monokai`, and
`github`. The last is labeled `GitHub` in the picker. The default is `quinjet`.

Inside the interface, open the command palette with `:` or `Ctrl+P`. `Change
Theme…` opens all thirteen palettes and `Change Appearance…` opens the system,
light, and dark choices. Moving through the theme list previews each palette
immediately. Moving through the appearance list likewise previews light, dark,
or system mode. `Enter` keeps the preview for the current run, while `Esc`
closes the picker and restores the theme or appearance that was active when it
opened. Picker lists wrap between their first and last entries. Use the launch
flags to choose the initial settings for a later run.

`--appearance system` asks the operating system for its current preference once
after Quinjet verifies that it owns an interactive terminal, and again when
`System` is explicitly selected in the appearance picker. It does not watch for
later changes. Explicit `light` and `dark` choices skip detection. A system that
does not publish a preference, including a minimal or remote Linux session
without an available desktop portal, safely uses the dark variant.

Examples:

```bash
quinjet tui --theme solarized
quinjet tui --theme nord --appearance light
quinjet tui ~/code/project --theme gruvbox --appearance dark
```

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
watcher on the worktree root plus the Git common directory so linked worktrees
show up without waiting, and, when asked for, the webhook listener. The first
frame asks for the working-tree status, the current branch's history, the list
of local and remote-tracking branches, the worktrees attached to this
repository, and a repository link derived only from configured Git remotes. No
GitHub request is made at startup or when the Pull Requests view is selected.
Only typing a number and pressing Enter contacts GitHub.

The terminal is put into raw mode with the alternate screen, bracketed paste
and, where the terminal supports them, the keyboard enhancement flags that let
Quinjet tell `Esc` apart from an escape sequence. Setup arms rollback as soon
as raw mode succeeds, so failure in any later setup step restores from the
first successful terminal mutation. Normal exit restores through the terminal
guard. In release builds, which abort on panic, the panic hook restores from
any thread before aborting. In unwind builds, a panic on the terminal thread
restores, while a worker panic leaves a still-running terminal intact.

## `--no-mouse`, and why releasing the mouse matters

While Quinjet has mouse capture on, the terminal forwards clicks, drags and
wheel events to the application, so the terminal's own text selection never
sees them. That is what makes rows clickable and dividers draggable, and it is
also what stops a normal drag from selecting text.

Dragging plain preview content selects and copies its text. In a side-by-side
diff, the selection stays inside the half where the drag began, so unrelated
text in the other half is not included. File headers and other controls remain
clickable.

`--no-mouse` starts with the mouse released. `m` changes the mouse setting at any
time and says which way it went: `Mouse off · select and copy with the terminal,
m to restore`. Holding `Shift` while dragging is the escape hatch most terminals
implement themselves, and it uses the terminal's own selection without
activating a control.

Horizontal trackpad swipes scroll wide preview lines. Terminals that encode a
horizontal gesture as `Shift` plus vertical wheel events are supported too.

Modal lists use one interaction contract. Up and down move the keyboard
selection, `Enter` activates it, moving the pointer highlights the row beneath
it, and clicking activates that row. The wheel pans a long modal list freely
without dragging the keyboard selection along with it. Returning to the
keyboard reveals the selected row again. This applies to branches, history
branches, comparisons, stashes, projects, pull-request repositories, command
and review actions, themes, and appearances. Visible modal controls have both a
mouse target and a key: `Tab` switches a commit between new and amend, `y` and
`n` answer confirmations, and `o`, `t`, and `s` resolve conflicts.

Releasing the mouse costs nothing else. Clickable rows, group actions and
divider dragging are the only things that stop working, and every one of them
has a key.

Repository names, branches, commit IDs, pull-request numbers, check names and
visible URL fields share one link treatment. Links are not underlined by
default. Holding Command or Control while hovering underlines the target. With
mouse capture on, Quinjet owns those cells and a normal single click opens the
target, including the pull-request and check URL fields. With mouse capture
off, Cmd-click or Ctrl-click uses a terminal hyperlink instead. Holding that
modifier while hovering also exposes the terminal hyperlink when capture is
on, which lets a local terminal open the target even when Quinjet runs over
SSH. A cmux SSH relay is used for ordinary browser clicks when its socket is
available. Clicking the worktree path opens Recent projects. The rest of a
commit row keeps selecting that commit, so opening and selecting remain
distinct targets.

## Project tabs

A session starts with one project and no project-tab strip. `w`, the header
path, and the worktree count open the project picker in `Switch project` mode.
Choosing a project or worktree replaces the repository in the current tab and
keeps that tab's identity and position.

`N` opens the same picker in `Open in new tab` mode. Adding a second project
reveals the project-tab strip. Every project tab owns its own Git worker,
filesystem watcher, repository data, selected view, filters, folds, and scroll
positions. The strip mixes local and SSH projects in one order. Tabs show their
machine when more than one machine is present. Selecting a tab on another
machine connects directly to that tab's host and project. `Ctrl+Tab` and
`Ctrl+Shift+Tab` cycle through the whole strip without resetting repository
state. A project selected for a new tab activates its existing tab when the
exact worktree root is already open on that machine. Different worktree roots
remain separate tabs even when they belong to the same Git repository.

The picker orders worktrees by their HEAD commit time, newest first. A project's
time is the newest time among its worktrees, and projects use that time for the
same newest-first ordering. Both levels show live relative ages. Long worktree
paths keep their beginning and ending text around a middle ellipsis and are
capped at 34 cells.

Each project heading starts with a bordered `[⌄]` or `[›]` control. Clicking
that control collapses or expands only that project's worktrees without opening
anything. Project headings also participate in keyboard selection. Move onto a
heading with `j`, `k`, or the arrow keys, then use `Enter`, `Space`, left, or
right to change its fold. `Ctrl+E` expands every project when any project is
collapsed, then collapses every project when all are expanded. Fold choices are
stored in Quinjet's state directory and restored when the picker is reopened or
Quinjet is started again. The footer names the next action.
Opening a selected worktree keeps the picker visible with the destination path
until repository discovery finishes. A failed open returns to the picker and
shows the error without losing the current filter or selection.
Filtering temporarily reveals matching worktrees inside collapsed projects, so
collapse never hides a search result. Worktrees that Git marks as prunable are
omitted because their directories no longer exist and cannot be opened.

When recent SSH repositories exist, `Open a project` and both the `w` and `N`
project pickers show the active machine. The originating computer appears
first under its hostname and is marked as the host. `Tab` and `Shift+Tab`
switch directly between reachable machines, while clicking a machine switches
to it in one step. Choosing the host returns to its local project picker
without a reverse SSH connection. A machine switch from the `N` picker keeps
`Open in new tab` mode, so the selected project is added as another tab on the
destination machine and appears in the same shared strip. The machine picker is
only needed to open a project that does not already have a tab.

Drag a visible project tab to reorder it. When the strip overflows, its left
and right controls cycle until the hidden tab becomes visible. Right-click a
tab for `Open Project...` in a new tab, `Close`, `Close Others`, and `Close
All`. `Ctrl+W` closes the active project tab. Closing the active tab chooses
its right neighbor when one exists, otherwise its left neighbor. Closing the
final tab exits the session.

Changes are divided into merge, staged, and unstaged sections. Untracked files
sit in Changes with the other worktree edits. Click a section header or press
`Space` to collapse or expand it. Left and right arrows move between a section
and its files without changing its folded state.
Pull-request Overview lists Conversation first, then a Checks heading above
the failed, in-progress, successful, and skipped groups. Pull-request file
trees also compact directory chains with only one child, matching
`apps/web/src/` rather than spending one row on each component.

Inside one pull-request file, `j` and `k` select reviewable diff lines and
existing review threads render below their anchors. `c` starts a pending line
comment, `C` starts a file comment, `a` replies to the selected line's thread,
and `x` resolves or reopens it. `Shift+V` opens the final review editor, where
`Tab` chooses comment, approve, or request changes. Comment and review editors
submit with `Ctrl+Enter`. Clicking an inline thread opens the actions GitHub
permits for that thread: reply, copy or open the latest comment, edit or delete
your latest comment, and resolve or reopen the thread. Review traffic has its
own worker lane and cannot block the diff or check-log workers.

## `--webhook-listen`

An open pull request stays current on its own poll. `--webhook-listen` makes it
react the instant GitHub says something happened, by pairing with the GitHub
CLI's forwarder:

```bash
quinjet tui --webhook-listen 8787
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
the conversation and a running log at once. The panel title still shows when a
pull request is refreshing or served from cache.

## How it stays live

Four clocks run, and they are independent.

The filesystem watcher reports every change under the worktree root, ignoring
pure access events, everything under `.git/objects`, `index.lock`, and
`.watchman-cookie-*` files. Event storms coalesce into one signal, because a
refresh reads the complete Git state rather than applying an event. If the
watcher cannot be created, the interface starts anyway and simply relies on the
next clock.

A 1 second display tick repaints relative timestamps without reading Git or
GitHub. Directly rendered lists advance on that tick, while cached pull-request
conversations and check details rebuild at most once every 10 seconds.

A 10 second tick refreshes the status regardless. It is the safety net for a
watcher that failed to start, a network filesystem that reports nothing, and a
change made by another process in a directory the watcher missed.

An open pull request polls itself. Check state is read every 5 seconds while
any run is in progress, every 20 seconds once everything has settled, and every
2 minutes when the reader is in another view or project tab. That cadence is a
ceiling rather than a schedule: the metadata and the conversation hold their
own 20 second floor and a growing log holds an 8 second floor, so a fast tick
costs one extra request rather than four. A finished run's log is never
re-read, because its archive can no longer change. Each stream is read
separately, so one failing endpoint never stalls the others, and a read that
coalesced into a request still in flight is left unstamped so it is due again
next tick.

## `--json` and the interface

There is no JSON shape for this page. `--json` is a global flag, so it parses
here, and it is then ignored: the interface writes to the alternate screen and
never emits a document. Every reading it shows has a verb that does emit one,
which is the point of the table below.

Examples:

```text
quinjet
quinjet tui ~/code/project
quinjet tui --no-mouse
quinjet tui --theme catppuccin
quinjet tui --theme rose-pine --appearance light
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
      --theme <THEME>             Color palette to use throughout the interface [default: quinjet]
      --appearance <APPEARANCE>   Use the system, light, or dark variant of the palette [default: system]
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
[`stage`, `unstage`, `discard`, `remove`, `commit`, `resolve`](./changes/README.md),
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
| `x` on a normal change, then `y` or Enter | `quinjet discard <path> --yes` |
| `x` with files checked, then `y` or Enter | `quinjet discard <paths> --yes` |
| `Revert (n)` in the toolbar, then `y` or Enter | `quinjet discard <paths> --yes` |
| `Stash (n)` in the toolbar | `quinjet stash push -- <paths>` |
| the checkbox on a group header | checks or clears every file in that group; selection state, not an operation |
| `X` on a file row, then `y` or Enter | `quinjet remove <path> --yes` |
| Remove Checked Files in the dropdown, then `y` or Enter | `quinjet remove <paths> --yes` |
| Revert Unstaged Changes / Revert All Changes in the dropdown, then `y` or Enter | `quinjet discard <paths> --yes` |
| `x` on a conflict | opens the conflict resolution modal, matching `quinjet resolve <path> --ours`, `--theirs`, or `--stage`; conflict discard is not an operation |
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
| `w`, clicking the header path, or clicking the worktree count in the footer | `quinjet worktree list` for this repository, plus the same listing for each recently opened project; Enter switches the current project tab, the same repository setup as `quinjet tui <path>` |
| `N` | opens the recent-project picker in new-tab mode; no verb, because project tabs belong to the terminal session |
| `Ctrl+E` in the project picker | expands all when any project is collapsed, or collapses all when every project is expanded; presentation state, not a repository operation |
| `Ctrl+Tab`, `Ctrl+Shift+Tab` | no verb. Cycles through project tabs while retaining each tab's state |
| `Ctrl+W` | no verb. Closes the active project tab |
| dragging or right-clicking a project tab | no verb. Reorders it or opens Close, Close Others, and Close All |
| `Ctrl+N` in the branch picker | `quinjet branch create <name>` |
| `F2` or `Ctrl+R` in the branch picker | `quinjet branch rename <old> <new>` |
| `Delete` in the branch picker, then confirm | `quinjet branch delete <name> --yes` |
| `b` in History | `quinjet branch list --all`, then `quinjet log <ref>` for the branch chosen |
| selecting a commit in History | `quinjet show <commit>` |
| `C` in History, then confirm | `quinjet cherry-pick <commit> --yes` |
| `R` in History, then confirm | `quinjet revert <commit> --yes` |
| `n` in History | `quinjet branch create <name> <commit>` |
| `S` in Changes | `quinjet stash list` |
| `*` or a file checkbox, then Commit becomes Stash | `quinjet stash push -- <paths>` after confirm and a message |
| `▶` on the Changes toolbar | Stage All, Unstage All, Compare Branch, Manage Stashes, and the three stash-create variants |
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
| `Shift+O` | Opens the selected branch or commit on GitHub. In Pull Requests it is `quinjet pr open <n>`, or `quinjet pr open <n> --check <name>` when a check is selected. |
| primary CTA in Pull Requests | merge, Ready for review, Disable auto-merge, Remove from merge queue, Reopen, or Open in browser, selected from live GitHub state |
| `▶` on the Pull Requests toolbar | every valid lifecycle, review, comment, metadata, branch update, merge, lock, notification, revert, close, and browser action for the loaded PR |
| `t` or `T` | `--expanded`, on `diff`, `show`, `branch compare` and `stash show`. `pr diff` has no `--expanded`: a pull-request patch is cached per file by its merge-base and head commits at three lines of context, and a second context width would need a second cache key |
| `v` | no verb. The command line prints unified patches only |
| `e` / `E` on a diff | no verb. A verb prints every file it was asked for |
| `e` / `E` in a check log | no verb. `pr logs` prints every step unfolded |
| `Space` on a file header in the preview | no verb |
| `Space`, `[`, `]` in a check log | no verb. They fold and move between steps |
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
- Remembering recently opened projects and keeping independent project tabs.
  The command line can list this repository's trees with `quinjet worktree
  list` and open another path with `quinjet tui <path>`, but it does not keep a
  recents file or multiple interface states in one process.
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
  not listing. Both preview until `--yes` is passed.
- `quinjet update`, which replaces the running executable rather than changing
  a repository or terminal view.
- `-C`, and running at all without a terminal.

## Notes and gotchas

- Exit is 0 when `q` quits, and 1 for the interactive-terminal refusal, a path
  that is not a repository, a `--webhook-listen` address that will not parse or
  will not bind, or a draw that fails.
- `--json` and `-C` are accepted here because clap declares them globally.
  Neither does anything. `-V` prints the propagated binary version and exits.
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
