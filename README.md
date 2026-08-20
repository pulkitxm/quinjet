# Quinjet

[![CI](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml/badge.svg)](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/quinjet.svg)](https://crates.io/crates/quinjet)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, live, keyboard-first Git source-control interface for the terminal, written in Rust.

Quinjet discovers the containing Git repository from any nested directory, watches it for changes, and combines a VS Code-inspired changes list with syntax-highlighted diffs and commit history. Navigation and rendering remain independent from Git subprocess latency.

## Features

- One command layer behind user-visible Git and GitHub operations in both faces, excluding terminal-only presentation state
- Non-interactive `--json` output and a `--watch` flag that streams one document per read
- Live working-tree, index, conflict, branch, and ahead/behind refresh
- Scrollable staged, unstaged, untracked, renamed, deleted, and conflict groups
- Visible, clickable per-file and per-group stage/unstage actions with immediate authoritative refresh
- Per-file and multi-file revert and removal, from a row, a checkbox selection, or the changes dropdown, always behind a confirmation
- Syntax highlighting for TypeScript/TSX, Rust, Python, Go, JavaScript, and hundreds of other formats
- Themed Font Awesome Brands and Devicons file logos for common languages, ecosystems, documents, media, and data formats
- Twelve unified color themes, each with light and dark variants selected from the system appearance at startup
- Unified and draggable side-by-side diff panes
- Index-first, lazy per-file patches for changes, commits, branch comparisons, stashes, and PRs
- Compact change hunks by default; `t` expands the selected file to full context
- Paginated, branch-scoped commit history with a view-only local/remote branch picker that never checks out
- On-demand pull-request lookup by number, with no startup prefetch and no repository-wide PR listing
- PR title, source/destination branches, state, totals, description, and the whole conversation: comments, reviews and their inline replies, pushed commits, force pushes, and every lifecycle event
- Foldable GitHub Actions logs per check run, with each step's output attached to the step that produced it, tailing while the job is still running
- Adaptive live refresh that watches a running pull request closely and a settled one loosely, plus optional instant refresh from forwarded webhooks
- Exact per-file line counts the moment a diff is indexed, and a virtualized changed-file tree with batched background patches
- Reused local PR workspaces across multiple fetch/push remotes and forks, with no checkout or persistent source refs
- Current-branch comparison with any local or remote-tracking branch, without checkout
- Named, staged-only, untracked-inclusive, preview, apply, pop, drop, and clear stash workflows
- Commit, amend, fetch, pull, push, sync, cherry-pick, and revert
- Preview-first cherry-pick and revert, with `--yes` required before either mutates
- Local branch switching, creation, rename, deletion, and creation at a selected commit
- Natural mouse scrolling, clickable rows, and draggable pane dividers
- Keyboard-first filtering, command palette, modal text editing, and accessibility help
- Persistent collapse/expand preference while moving among changes, commits, PRs, and views
- Coalesced background Git work, stale-result rejection, bounded output, and a private metadata cache

## Installation

### Homebrew

On macOS or Linux:

```bash
brew install pulkitxm/tap/quinjet
```

The formula downloads the release binary for your platform, verifies its
published checksum, and installs the `quinjet` executable, the `q` shortcut,
shell completions, and the manual page under `brew --prefix`. Homebrew owns that
installation, so upgrade it with `brew upgrade quinjet` rather than
`quinjet update`. See [the Homebrew guide](docs/guides/homebrew.md) for the rest
of the commands.

### Install script

On Linux or macOS:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh
```

On Windows PowerShell:

```powershell
powershell -c "irm https://quinjet.pulkit.page/install.ps1 | iex"
```

The installer detects the operating system and CPU architecture, downloads the matching binary from the latest GitHub release, verifies its SHA-256 checksum, adds the installation directory to `PATH` when needed, installs completions, and adds `q` as a shortcut for `quinjet`. A root installation uses `/usr/local/bin` when it is writable and already on `PATH`, which makes both commands available in the current shell, including in containers. Other installations use the configured user directory and print the exact `export PATH=...` command when the parent shell still needs it. It does not require Rust or Cargo.

Pass `--version` or `--bin-dir` to the shell installer to select a release or installation directory:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh -s -- --version vX.Y.Z
```

The equivalent PowerShell environment variables are `QUINJET_VERSION` and `QUINJET_INSTALL_DIR`.

Whichever installation path you chose, Quinjet can check for and install the
latest stable release itself:

```bash
quinjet update --check
quinjet update
```

The updater replaces the executable that is actually running, following the `q`
shortcut to that same file, so it works for script, Cargo, cargo-binstall, and
custom-directory installations without guessing where the binary lives. It pins
the download to one release, verifies the published SHA-256 checksum before
replacing anything, and refreshes the installed shell completions with the new
executable. It leaves a completion or `q` shortcut alone when you removed it
after the first installation.

### Cargo

From crates.io:

```bash
cargo install quinjet
```

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), the package
metadata maps every currently released Linux, macOS, and Windows target to its
published binary:

```bash
cargo binstall quinjet
```

From the latest source:

```bash
cargo install --git https://github.com/pulkitxm/quinjet --locked
```

The first invocation after a Cargo, cargo-binstall, package-manager, or copied-binary installation detects your shell, installs its completions, and adds a `q` launcher on `PATH`, beside the Quinjet executable when that directory is writable. Even when no shell can be detected, the first invocation still installs `q`. The launcher works in the current shell whenever the executable directory was already on `PATH`; no profile reload is needed for `q` itself. Later invocations refresh stale scripts without rewriting current ones. Persistent installed-once markers mean that deleting a generated script, marked completion block, or `q` launcher opts out permanently; updates do not restore what you removed.

Git is required at runtime. A terminal with true-color support and a current [Nerd Font](https://www.nerdfonts.com/) is recommended. The patched font renders the Font Awesome-compatible [file icons](docs/file-icons.md); filenames remain usable when the terminal font lacks a glyph. The Pull Requests view additionally requires the [GitHub CLI](https://cli.github.com/) with an authenticated account (`gh auth login`). All non-GitHub features remain available without `gh`.

## Usage

Run from any directory inside a Git repository:

```bash
quinjet
```

Quinjet asks Git for the top-level repository, so running it from `project/apps/web/src` still displays every change in `project`. You can also provide a path explicitly:

```bash
quinjet tui /path/to/project
```

Mouse capture can be disabled without losing functionality:

```bash
quinjet tui --no-mouse
```

An open pull request stays current on its own poll. To make it react the instant something happens instead, let the GitHub CLI forward deliveries to Quinjet:

```bash
quinjet tui --webhook-listen 8787
gh webhook forward --repo owner/name --events '*' --url http://127.0.0.1:8787
```

Quinjet uses its own palette and the system light or dark appearance by default.
Open the command palette with `:` or `Ctrl+P`, then choose `Change Theme…` or
`Change Appearance…` to switch either setting without leaving the interface.
The launch flags choose the initial palette and appearance for this run:

```bash
quinjet tui --theme catppuccin
quinjet tui --theme rose-pine --appearance light
quinjet tui --theme tokyo-night --appearance dark
```

Only loopback connections are accepted, and a delivery is treated purely as a signal to re-read the pull request through `gh`; nothing from the request body is trusted or displayed.

### Command line

Every user-visible Git and GitHub operation the interface performs is also reachable through a subcommand, and repository data operations execute the same command layer rather than reaching Git itself. Browser opening uses the same helper on both faces. Presentation state such as focus, scrolling, folding, and filtering remains terminal-only:

```bash
quinjet status                        # the Changes view, as text
quinjet diff --staged                 # the patch a commit would record
quinjet log -n 10                     # the History view
quinjet branch list --all             # local and remote-tracking branches
quinjet stash show 'stash@{0}'        # a stash as a patch
quinjet pr view 12                    # a pull request's metadata
quinjet pr view 12 --watch            # keep its metadata current
quinjet pr conversation 12 --watch    # follow conversation updates
quinjet pr checks 12 --watch          # block until CI settles
quinjet pr logs 12 clippy --watch     # tail a running job's log
quinjet pr open 12 --check clippy      # open one check run in a browser
quinjet update --check                 # report whether a stable update exists
```

`quinjet --help` lists the whole tree. Subcommands are dispatched before the terminal is claimed, so a piped run never meets the interactive-terminal refusal. `-C <dir>` chooses the repository, `--json` prints one document on stdout, and `--watch` prints one compact document per read. Destructive subcommands report what they would do and change nothing until `--yes`.

Exit codes are part of the contract: `0` success, `1` failure or a watched run that did not go green, `2` a bad command line, `3` a name that matches nothing or matches too much, `4` something that exists but cannot be read. The [command-line reference](docs/cli/README.md) documents every subcommand, and the [conventions page](docs/cli/conventions.md) is the whole contract.

### Changes, staging, branch comparison, and stashes

The Changes view follows the VS Code SCM grouping model: conflicts, staged changes, and working-tree changes are separate selectable groups. Every file row has a checkbox for multi-file selection plus a visible `[+]`, `[−]`, or `[!]` stage action, and group headers carry their own checkbox that checks or clears the whole group, showing `[x]`, `[ ]`, or `[-]` when only part of it is checked, next to the stage-all/unstage-all action. The bottom toolbar is a single primary button (`Commit`) until any file is checked, at which point it becomes `Stash (n)` with a red `Revert (n)` button above it. Both count the checked files, share one width, and align their labels to the same column whatever the count grows to. The `▶` menu follows the selection too: Stage All, Unstage All, then Revert and Remove for the checked files with their count, or for the selected file when nothing is checked, then Revert Unstaged Changes, Revert All Changes, Compare Branch, Manage Stashes, and the full-tree stash variants. Reverting restores a file; removing deletes it from the working tree and stages the deletion. Both ask first, and `x` and `X` are their keyboard equivalents.

All local code views are index-first. Quinjet enumerates paths and immediately renders stable collapsed file headers, then loads only the first file in the background. Expanding another header requests only that path and keeps the result cached for the current view; it never calculates the complete patch up front. Click a file header or focus the preview and press `Space` to expand it. A single-file preview always stays expanded and shows no collapse control.

Press `d` (or run **Compare Current Branch With…** from the command palette) to select a local or remote-tracking branch. Quinjet indexes the read-only comparison between that branch and `HEAD`; it does not check out the branch or modify the index/worktree. Background status refreshes and collapse/expand actions do not restart or replace an active comparison. Press `Esc` to return to the selected working-tree diff.

Press `S` to open the stash manager. It lists stash reference, message, source branch, commit, and age. `Enter` previews a stash; `Ctrl+N`, `Ctrl+U`, and `Ctrl+S` create a normal, untracked-inclusive, or staged-only stash; `Alt+A` applies, `Alt+P` pops, `Delete` drops one, and `Ctrl+Delete` drops all after confirmation. The command palette exposes the same creation and latest-pop flows.

### History branches

History starts at the currently checked-out branch instead of mixing every ref into one log. In the History view, press `b` to choose any local or remote-tracking branch. This changes only the revision passed to `git log`; Quinjet does not run `git switch`, move `HEAD`, touch the index/worktree, or create a temporary ref. Selecting a commit indexes its changed paths first and lazily loads individual file patches. The checked-out branch remains visible in the top bar while the viewed branch appears in the History panel title. Press `B` when you explicitly want the checkout branch picker.

### Pull requests, remotes, and cache

Press `3`, enter a positive PR number, and press Enter. That explicit action is the first time Quinjet performs any GitHub request: startup and tab switching never list, prefetch, or auto-fetch pull requests. Quinjet lazily discovers the most appropriate configured GitHub repository and fetches only that PR's metadata.

The view has two halves. **PR** (`Shift+P`) lists the conversation and every check run on the left. With the conversation selected, the right side is the pull request itself: title, state, author, source and destination, totals, check summary, description, and the full thread of comments, reviews and their inline replies, pushed commits, force pushes, renames, labels, and merges. Selecting a check replaces the right side with that run's steps, each showing its status and duration and folding open to the log lines that step produced. `j`/`k` move through the steps, `Space` folds one, `e` folds them all, and `PgUp`/`PgDn` or the wheel scroll an unfolded step's output.

A run still in progress is shown as it happens rather than as a placeholder: finished steps carry their durations, the step in flight counts up, and its output is re-read often enough to tail. The view stays on the newest lines while you sit at the end and holds still once you scroll up to read something earlier. The step that failed, or the one still running, opens on its own.

Prose is wrapped to the pane and stays put. Code blocks, log output and diff context keep their original width instead, and `h`/`l` or `←`/`→` scroll only those, so a pasted terminal capture or a long log line can be read without the surrounding comment sliding away. The panel title shows how far the widest line extends past the edge.

**Files** (`Shift+F`) is the changed-file tree. Quinjet prepares a local Git comparison as soon as a PR opens and indexes every changed path together with its exact line counts, so headers read `+40 -0` immediately rather than resolving one file at a time. Patches then stream in behind batched reads, and anything you select is usually already there. Every file and folder is selectable; click a folder or press `Space` to collapse and expand it. Left and right move through the hierarchy without changing its folded state. Rapid selections coalesce and stale results are discarded, so a PR with one file and a PR with thousands use the same stable layout.

Press `o` to discover or choose a configured remote repository, and `r` to refetch the current PR immediately.

Quinjet supports fork setups such as `origin` pointing to your fork and `upstream` to the base, separate push URLs, deleted fork heads exposed through GitHub's PR ref, and GitHub Enterprise hosts configured in `gh`. PR patches are **not** downloaded with `gh pr diff`: when both immutable OIDs already exist locally, Quinjet diffs them directly with no network request. Otherwise it creates one disposable bare workspace for the opened PR, shallow/partial-fetches the fixed base and PR-head refs, deepens only as needed to find the merge base, and reuses that workspace for every selected file. The opened repository is never checked out or given temporary branches/refs.

Anything whose key already identifies it is cached permanently and can never go stale: a finished run's steps and log by job, the changed-file listing and each file's patch by the two commits they describe, the conversation by the stamp GitHub moves whenever the thread changes. A new commit or a new comment asks a different question instead of invalidating an old answer. Only genuinely time-varying reads keep a clock: repository identity for a day, PR metadata for five minutes, the check list for thirty seconds. A run still in progress is never cached, since re-reading it is what tails it. Opening a PR also warms every settled run's log in the background, so selecting any check is a disk read. The panel shows `⟳` while anything is refreshing and `cached` when what you see came from disk.

The cache is bounded to 128 MiB / 2,048 entries, pruned oldest first, and never stores credentials. It lives under `$XDG_CACHE_HOME/quinjet/github` (or `~/.cache/quinjet/github`), `~/Library/Caches/quinjet/github` on macOS, and `%LOCALAPPDATA%\quinjet\cache\github` on Windows. Set `QUINJET_CACHE_DIR` to choose a different root. Cache directories/files use private permissions where supported; `r` bypasses fresh cache entries, while a stale entry can keep the view useful during a transient `gh` failure.

Branch rename is local and deliberately does not delete or create remote branches. Open the checkout branch picker with `b` outside History (or `B` anywhere), select a branch, and press `F2` or `Ctrl+R`; its existing upstream configuration is preserved by Git.

## Keyboard

The UI intentionally stays uncluttered; press `?` for the complete shortcut reference.

| Key | Action |
| --- | --- |
| `j` / `k`, arrows | Move through every file/commit or scroll the preview |
| Mouse wheel | Naturally scroll the pane under the pointer |
| `m` | Release the mouse so the terminal can select and copy text, and take it back |
| `Shift` + `O` | Open the selected check run on github.com, or the pull request itself |
| `Shift` + mouse drag | Select terminal text without activating Quinjet controls (`--no-mouse` starts with the mouse released) |
| `Tab` / `Enter` | Toggle sidebar/preview focus |
| `z` | Hide/show the sidebar |
| `e` / `E` | Collapse/expand every file diff; keep that preference across selections/views |
| `Space` with preview focus | Expand/collapse the selected file in a multi-file preview |
| `1` / `2` / `3` | Changes/history/on-demand pull request |
| `s` or `Space` | Toggle stage/unstage for the selected file |
| `[+]` / `[−]` click | Stage/unstage that file or resource group |
| `a` / `U` | Stage all/unstage all |
| `c` | Open commit editor; `Ctrl+Enter` submits |
| `S` | Open stash manager |
| `d` in Changes | Compare current branch with another branch |
| `t` | Toggle compact hunks/full-file context |
| `v` | Toggle unified/side-by-side diff |
| `x` | Revert a normal change, or every checked file, after confirmation; open resolution for a conflict |
| `X` | Remove the selected file, or every checked file, from the working tree and the index |
| `b` in History | View another local/remote branch without checkout |
| `b` elsewhere / `B` | Checkout branch picker; `F2`/`Ctrl+R` renames a local branch |
| `o` in Pull Requests | Discover/select a repository and reopen the entered PR |
| `Shift+P` / `Shift+F` in Pull Requests | The PR and its checks/all changed files |
| `j` / `k`, `[` / `]` in a check log | Previous/next step |
| `Space` / `e` in a check log | Fold the selected step/every step |
| `Space` on a PR folder | Collapse/expand that folder |
| `r` | Refresh status; refetch the opened PR and checks in the PR view |
| `/` | Filter changes/history, or focus the numeric PR field |
| `[` / `]` | Previous/next diff hunk |
| `:` or `Ctrl+P` | Command palette |
| `?` | Shortcut help |
| `q` | Quit |

Drag the divider between the file list and preview to resize the main panes. In side-by-side mode, drag the center divider to resize old/new sides. Double-click either divider to restore that split to its default size.

Text fields support Unicode-safe editing plus familiar terminal and macOS motions: Option/Ctrl+Arrow moves by word, Option/Ctrl+Delete removes a word, Command+Arrow moves to a line boundary, Command+Delete removes to a line boundary, and Ctrl+A/E/B/F/W/U/K are supported.

## Performance and Safety

The [optimization reference](docs/optimization/techniques.md) explains the
complete performance model in depth. Its 23 chapters cover Git storage and
history, diff algorithms and parsing, isolated pull-request workspaces, API and
cache strategy, byte-budgeted prefetch, progressive viewport rendering,
concurrency, benchmarking, failure modes, and regression review.

- Rendering and key handling never invoke Git directly.
- Fixed, coalescing mailboxes replace obsolete reads; local previews, PR/network previews, and background metadata run independently so one slow request cannot block tab switching.
- Filesystem event storms collapse into authoritative status snapshots.
- Preview requests carry generations so stale replies are ignored.
- Working-tree groups, commits, branch comparisons, and stashes first use bounded path indexes; only selected files produce patches, through capped subprocess pipes.
- Syntax grammar work is bounded to 512 KiB per patch and 32 KiB per row. Larger generated patches retain diff coloring but use plain source spans, preventing highlighting, rather than Git, from dominating load time; collapsed cached files also keep only their headers in the combined document.
- History is paginated for one explicit branch revision; choosing another branch is read-only.
- No pull-request command is queued at startup or when the PR tab opens. Only an explicit positive-number lookup contacts GitHub, and refreshing refetches only that PR.
- Once a pull request is open it refreshes on an adaptive schedule: check state every 5 seconds while a run is in progress, every 20 seconds once it settles, and every 2 minutes from another view. Metadata and the conversation hold a 20-second floor and a growing log an 8-second floor regardless of the tick, so a fast cadence costs one extra request rather than five. A finished run's log is never re-read, each read is independent so one failing endpoint never stalls the others, and a moved head reindexes only the diff. A forwarded webhook bypasses every floor.
- Conversations are bounded to 500 entries and check logs to 8 MiB / 200,000 lines, read through the same capped pipes as every other subprocess.
- On-demand repository discovery inspects at most 32 Git remotes, 64 configured fetch/push URL entries (32 distinct URLs), and 16 GitHub repositories.
- PR file indexes are capped at 16,384 paths / 8 MiB. Each selected file has an independent 8 MiB patch cap; the full PR patch is never materialized. Potentially large subprocess output is streamed and the child is terminated at the cap.
- The changed-file tree is virtualized: render cost follows terminal height rather than PR size. Rapid file selections coalesce into the newest per-file Git diff with no loading-layout replacement.
- PR previews use locally available immutable OIDs first. Missing Git history uses the compare API's merge base when available and otherwise deepens through a bounded ladder ending at 16,384 commits in one reusable disposable bare repository. No checkout, worktree mutation, or persistent source-repository ref is used.
- The private persistent cache is bounded to 128 MiB and 2,048 entries, with a 1 MiB ceiling for one file patch. Immutable content is keyed by its object identities; exact PR metadata lives for five minutes.
- Git and `gh` receive argument arrays directly, never shell-concatenated commands; embedded remote credentials are stripped before URLs become `gh` arguments.
- Destructive operations are confirmed where appropriate.
- Hooks, credentials, signing, filters, and repository semantics remain delegated to the installed Git CLI.

See [ARCHITECTURE.md](ARCHITECTURE.md) for implementation details.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md), the
[Code of Conduct](CODE_OF_CONDUCT.md), [Security Policy](SECURITY.md),
[Support guide](SUPPORT.md), and [Governance](GOVERNANCE.md) before opening a
pull request or report.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --locked
```

`scripts/drive.py` drives a real build through a pty and reads back what it drew. It opens a pull request against live GitHub, so it needs a working `gh` login and a network and is deliberately not part of CI; use it for what a unit test cannot see, such as which requests overlap and how long a pane waits.

```bash
python3 -m venv .venv && .venv/bin/pip install pyte
QUINJET=$PWD/target/release/quinjet .venv/bin/python scripts/drive.py switch /path/to/repo 507 506
```

## License

Quinjet is available under the [MIT License](LICENSE).
