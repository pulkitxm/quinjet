# Quinjet

[![CI](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml/badge.svg)](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/quinjet.svg)](https://crates.io/crates/quinjet)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, live, keyboard-first Git source-control interface for the terminal, written in Rust.

Quinjet discovers the containing Git repository from any nested directory, watches it for changes, and combines a VS Code-inspired changes list with syntax-highlighted diffs and commit history. Navigation and rendering remain independent from Git subprocess latency.

## Features

- Live working-tree, index, conflict, branch, and ahead/behind refresh
- Scrollable staged, unstaged, untracked, renamed, deleted, and conflict groups
- Visible, clickable per-file and per-group stage/unstage actions with immediate authoritative refresh
- Syntax highlighting for TypeScript/TSX, Rust, Python, Go, JavaScript, and hundreds of other formats
- Unified and draggable side-by-side diff panes
- Index-first, lazy per-file patches for changes, commits, branch comparisons, stashes, and PRs
- Compact change hunks by default; `t` expands the selected file to full context
- Paginated, branch-scoped commit history with a view-only local/remote branch picker that never checks out
- On-demand pull-request lookup by number—no startup prefetch and no repository-wide PR listing
- PR title, source/destination branches, state, totals, description, and the whole conversation: comments, reviews and their inline replies, pushed commits, force pushes, and every lifecycle event
- Foldable GitHub Actions logs per check run, with each step's output attached to the step that produced it
- Adaptive live refresh that watches a running pull request closely and a settled one loosely, plus optional instant refresh from forwarded webhooks
- Exact per-file line counts the moment a diff is indexed, and a virtualized changed-file tree with batched background patches
- Reused local PR workspaces across multiple fetch/push remotes and forks—no checkout or persistent source refs
- Current-branch comparison with any local or remote-tracking branch, without checkout
- Named, staged-only, untracked-inclusive, preview, apply, pop, drop, and clear stash workflows
- Commit, amend, fetch, pull, push, sync, cherry-pick, and revert
- Local branch switching, creation, rename, deletion, and creation at a selected commit
- Natural mouse scrolling, clickable rows, and draggable pane dividers
- Keyboard-first filtering, command palette, modal text editing, and accessibility help
- Persistent collapse/expand preference while moving among changes, commits, PRs, and views
- Coalesced background Git work, stale-result rejection, bounded output, and a private metadata cache

## Installation

### Install script

On Linux or macOS:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh
```

On Windows PowerShell:

```powershell
powershell -c "irm https://quinjet.pulkit.page/install.ps1 | iex"
```

The installer detects the operating system and CPU architecture, downloads the matching binary from the latest GitHub release, verifies its SHA-256 checksum, and adds the installation directory to `PATH` when needed. It does not require Rust or Cargo.

Pass `--version` or `--bin-dir` to the shell installer to select a release or installation directory:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh -s -- --version v0.0.1
```

The equivalent PowerShell environment variables are `QUINJET_VERSION` and `QUINJET_INSTALL_DIR`.

### Cargo

From crates.io:

```bash
cargo install quinjet
```

From the latest source:

```bash
cargo install --git https://github.com/pulkitxm/quinjet --locked
```

Git is required at runtime. A terminal with true-color support is recommended. The Pull Requests view additionally requires the [GitHub CLI](https://cli.github.com/) with an authenticated account (`gh auth login`). All non-GitHub features remain available without `gh`.

## Usage

Run from any directory inside a Git repository:

```bash
quinjet
```

Quinjet asks Git for the top-level repository, so running it from `project/apps/web/src` still displays every change in `project`. You can also provide a path explicitly:

```bash
quinjet /path/to/project
```

Mouse capture can be disabled without losing functionality:

```bash
quinjet --no-mouse
```

An open pull request stays current on its own poll. To make it react the instant something happens instead, let the GitHub CLI forward deliveries to Quinjet:

```bash
quinjet --webhook-listen 8787
gh webhook forward --repo owner/name --events '*' --url http://127.0.0.1:8787
```

Only loopback connections are accepted, and a delivery is treated purely as a signal to re-read the pull request through `gh`; nothing from the request body is trusted or displayed.

### Changes, staging, branch comparison, and stashes

The Changes view follows the VS Code SCM grouping model: conflicts, staged changes, and working-tree changes are separate selectable groups. Every file row has a visible `[+]`, `[−]`, or `[!]` action, and group headers expose stage-all/unstage-all actions. The bottom toolbar provides Commit, Stashes, Stage All, Unstage All, and Compare Branch entry points. Keyboard equivalents remain available.

All local code views are index-first. Quinjet enumerates paths and immediately renders stable collapsed file headers, then loads only the first file in the background. Expanding another header requests only that path and keeps the result cached for the current view; it never calculates the complete patch up front. Click a file header or focus the preview and press `Space` to expand it. A single-file preview always stays expanded and shows no collapse control.

Press `d` (or run **Compare Current Branch With…** from the command palette) to select a local or remote-tracking branch. Quinjet indexes the read-only comparison between that branch and `HEAD`; it does not check out the branch or modify the index/worktree. Background status refreshes and collapse/expand actions do not restart or replace an active comparison. Press `Esc` to return to the selected working-tree diff.

Press `S` to open the stash manager. It lists stash reference, message, source branch, commit, and age. `Enter` previews a stash; `Ctrl+N`, `Ctrl+U`, and `Ctrl+S` create a normal, untracked-inclusive, or staged-only stash; `Alt+A` applies, `Alt+P` pops, `Delete` drops one, and `Ctrl+Delete` drops all after confirmation. The command palette exposes the same creation and latest-pop flows.

### History branches

History starts at the currently checked-out branch instead of mixing every ref into one log. In the History view, press `b` to choose any local or remote-tracking branch. This changes only the revision passed to `git log`; Quinjet does not run `git switch`, move `HEAD`, touch the index/worktree, or create a temporary ref. Selecting a commit indexes its changed paths first and lazily loads individual file patches. The checked-out branch remains visible in the top bar while the viewed branch appears in the History panel title. Press `B` when you explicitly want the checkout branch picker.

### Pull requests, remotes, and cache

Press `3`, enter a positive PR number, and press Enter. That explicit action is the first time Quinjet performs any GitHub request: startup and tab switching never list, prefetch, or auto-fetch pull requests. Quinjet lazily discovers the most appropriate configured GitHub repository and fetches only that PR's metadata.

The view has two halves. **Pull request** (`P`) lists the conversation and every check run on the left. With the conversation selected, the right side is the pull request itself: title, state, author, source and destination, totals, check summary, description, and the full thread of comments, reviews and their inline replies, pushed commits, force pushes, renames, labels, and merges. Selecting a check replaces the right side with that run's steps, each showing its status and duration and folding open to the log lines that step produced; `[` and `]` move between steps, `Space` folds one, and `e` folds them all. The failing step opens on its own.

**Files** (`F`) is the changed-file tree. Quinjet prepares a local Git comparison as soon as a PR opens and indexes every changed path together with its exact line counts, so headers read `+40 -0` immediately rather than resolving one file at a time. Patches then stream in behind batched reads, and anything you select is usually already there. Every file and folder is selectable; click a folder or use `Left`/`Right`, `h`/`l`, `Enter`, or `Space` to collapse and expand it. Rapid selections coalesce and stale results are discarded, so a PR with one file and a PR with thousands use the same stable layout.

Press `o` to discover or choose a configured remote repository, and `r` to refetch the current PR immediately.

Quinjet supports fork setups such as `origin` pointing to your fork and `upstream` to the base, separate push URLs, deleted fork heads exposed through GitHub's PR ref, and GitHub Enterprise hosts configured in `gh`. PR patches are **not** downloaded with `gh pr diff`: when both immutable OIDs already exist locally, Quinjet diffs them directly with no network request. Otherwise it creates one disposable bare workspace for the opened PR, shallow/partial-fetches the fixed base and PR-head refs, deepens only as needed to find the merge base, and reuses that workspace for every selected file. The opened repository is never checked out or given temporary branches/refs.

Successful `gh` repository-identity and exact-PR responses are cached atomically. The cache is bounded to 32 MiB / 256 entries and stores metadata only—not credentials, Git objects, or patches. It lives under `$XDG_CACHE_HOME/quinjet/github` (or `~/.cache/quinjet/github`), `~/Library/Caches/quinjet/github` on macOS, and `%LOCALAPPDATA%\quinjet\cache\github` on Windows. Set `QUINJET_CACHE_DIR` to choose a different root. Cache directories/files use private permissions where supported; `r` bypasses fresh cache entries, while a stale entry can keep the view useful during a transient `gh` failure.

Branch rename is local and deliberately does not delete or create remote branches. Open the checkout branch picker with `b` outside History (or `B` anywhere), select a branch, and press `F2` or `Ctrl+R`; its existing upstream configuration is preserved by Git.

## Keyboard

The UI intentionally stays uncluttered; press `?` for the complete shortcut reference.

| Key | Action |
|---|---|
| `j` / `k`, arrows | Move through every file/commit or scroll the preview |
| Mouse wheel | Naturally scroll the pane under the pointer |
| `Shift` + mouse drag | Select terminal text without activating Quinjet controls (`--no-mouse` disables capture entirely) |
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
| `x` | Discard selected change after confirmation |
| `b` in History | View another local/remote branch without checkout |
| `b` elsewhere / `B` | Checkout branch picker; `F2`/`Ctrl+R` renames a local branch |
| `o` in Pull Requests | Discover/select a repository and reopen the entered PR |
| `P` / `F` in Pull Requests | The pull request and its checks/all changed files |
| `[` / `]` in a check log | Previous/next step |
| `Space` / `e` in a check log | Fold the selected step/every step |
| `←` / `→`, `h` / `l`, `Enter`, or `Space` on a PR folder | Collapse/expand that folder |
| `r` | Refresh status; refetch the opened PR and checks in the PR view |
| `/` | Filter changes/history, or focus the numeric PR field |
| `[` / `]` | Previous/next diff hunk |
| `:` or `Ctrl+P` | Command palette |
| `?` | Shortcut help |
| `q` | Quit |

Drag the divider between the file list and preview to resize the main panes. In side-by-side mode, drag the center divider to resize old/new sides. Double-click either divider to restore that split to its default size.

Text fields support Unicode-safe editing plus familiar terminal and macOS motions: Option/Ctrl+Arrow moves by word, Option/Ctrl+Delete removes a word, Command+Arrow moves to a line boundary, Command+Delete removes to a line boundary, and Ctrl+A/E/B/F/W/U/K are supported.

## Performance and Safety

- Rendering and key handling never invoke Git directly.
- Fixed, coalescing mailboxes replace obsolete reads; local previews, PR/network previews, and background metadata run independently so one slow request cannot block tab switching.
- Filesystem event storms collapse into authoritative status snapshots.
- Preview requests carry generations so stale replies are ignored.
- Working-tree groups, commits, branch comparisons, and stashes first use bounded path indexes; only selected files produce patches, through capped subprocess pipes.
- Syntax grammar work is bounded to 512 KiB per patch and 32 KiB per row. Larger generated patches retain diff coloring but use plain source spans, preventing highlighting—not Git—from dominating load time; collapsed cached files also keep only their headers in the combined document.
- History is paginated for one explicit branch revision; choosing another branch is read-only.
- No pull-request command is queued at startup or when the PR tab opens. Only an explicit positive-number lookup contacts GitHub, and refreshing refetches only that PR.
- Once a pull request is open it refreshes on an adaptive schedule: every 5 seconds while a check is running, every 20 seconds once they settle, and every 2 minutes from another view. Each read is independent, so one failing endpoint never stalls the others, and a moved head reindexes only the diff.
- Conversations are bounded to 2,000 entries and check logs to 8 MiB / 200,000 lines, read through the same capped pipes as every other subprocess.
- On-demand repository discovery inspects at most 32 Git remotes, 64 configured fetch/push URL entries (32 distinct URLs), and 16 GitHub repositories.
- PR file indexes are capped at 16,384 paths / 8 MiB. Each selected file has an independent 8 MiB patch cap; the full PR patch is never materialized. Potentially large subprocess output is streamed and the child is terminated at the cap.
- The changed-file tree is virtualized: render cost follows terminal height rather than PR size. Rapid file selections coalesce into the newest per-file Git diff with no loading-layout replacement.
- PR previews use locally available immutable OIDs first. Missing Git history deepens only to 4,096 commits in one reusable disposable bare repository. No checkout, worktree mutation, or persistent source-repository ref is used.
- Cached `gh` metadata is bounded to 32 MiB and 256 entries; exact PR entries live for five minutes.
- Git and `gh` receive argument arrays directly, never shell-concatenated commands; embedded remote credentials are stripped before URLs become `gh` arguments.
- Destructive operations are confirmed where appropriate.
- Hooks, credentials, signing, filters, and repository semantics remain delegated to the installed Git CLI.

See [ARCHITECTURE.md](ARCHITECTURE.md) for implementation details.

## Contributing

Contributions are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md), the [Code of Conduct](CODE_OF_CONDUCT.md), and [Security Policy](SECURITY.md) before opening a pull request or report.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --locked
```

## License

Quinjet is available under the [MIT License](LICENSE).
