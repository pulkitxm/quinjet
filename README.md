# Quinjet

[![CI](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml/badge.svg)](https://github.com/pulkitxm/quinjet/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/quinjet.svg)](https://crates.io/crates/quinjet)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, live, keyboard-first Git source-control interface for the terminal, written in Rust.

Quinjet discovers the containing Git repository from any nested directory, watches it for changes, and combines a VS Code-inspired changes list with syntax-highlighted diffs and commit history. Navigation and rendering remain independent from Git subprocess latency.

## Features

- Live working-tree, index, conflict, branch, and ahead/behind refresh
- Scrollable staged, unstaged, untracked, renamed, deleted, and conflict groups
- One-key file staging/unstaging with immediate authoritative refresh
- Syntax highlighting for TypeScript/TSX, Rust, Python, Go, JavaScript, and hundreds of other formats
- Unified and draggable side-by-side diff panes
- Compact change hunks by default; `t` expands the selected file to full context
- Paginated, branch-scoped commit history with a view-only local/remote branch picker that never checks out
- Progressive all-state GitHub pull-request loading, local 25-row pages, repository-scoped numeric lookup, and changed-file diff pages
- Disposable local PR fetches across multiple fetch/push remotes and forks—no checkout or persistent refs
- Commit, amend, stash, fetch, pull, push, sync, cherry-pick, and revert
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

### History branches

History starts at the currently checked-out branch instead of mixing every ref into one log. In the History view, press `b` to choose any local or remote-tracking branch. This changes only the revision passed to `git log`; Quinjet does not run `git switch`, move `HEAD`, touch the index/worktree, or create a temporary ref. The checked-out branch remains visible in the top bar while the viewed branch appears in the History panel title. Press `B` when you explicitly want the checkout branch picker.

### Pull requests, remotes, and cache

Press `3` to open Pull Requests. Quinjet derives standard `github.com` fetch/push repository identities locally, starts PR metadata prefetch in the background at launch, selects one repository, and progressively fetches **all open, merged, and closed PRs** in bounded 50-record GraphQL batches. The first batch appears immediately; skeleton rows, fetched/total counts, and a percentage show the remaining work while later batches fill the local snapshot. Press `o` to choose the base repository and `←` / `→` to move through local 25-row pages without another network request. The bottom `PR #` field accepts digits only; press `/`, enter a number, and press Enter for an exact lookup scoped to that selected repository.

Quinjet supports fork setups such as `origin` pointing to your fork and `upstream` to the base, separate push URLs, deleted fork heads exposed through GitHub's PR ref, and GitHub Enterprise hosts configured in `gh`. Selected metadata includes immutable base/head OIDs. PR patches are **not** downloaded with `gh pr diff`: when both OIDs already exist locally, Quinjet diffs them directly with no network request; otherwise it creates a disposable bare repository, shallow-fetches fixed internal base/head refs with partial-clone filtering, and deepens only as needed to find the merge base. It renders 20 changed files at a time. Use `,` / `.` for changed-file pages. The opened repository is never checked out or given temporary branches/refs.

Successful `gh` repository, PR-batch, and exact-PR responses are cached atomically with short TTLs. The cache is bounded to 32 MiB / 256 entries and stores metadata only—not credentials, Git objects, or patches. It lives under `$XDG_CACHE_HOME/quinjet/github` (or `~/.cache/quinjet/github`), `~/Library/Caches/quinjet/github` on macOS, and `%LOCALAPPDATA%\quinjet\cache\github` on Windows. Set `QUINJET_CACHE_DIR` to choose a different root. Cache directories/files use private permissions where supported; `r` bypasses fresh cache entries, while a stale entry can keep the view useful during a transient `gh` failure.

Branch rename is local and deliberately does not delete or create remote branches. Open the checkout branch picker with `b` outside History (or `B` anywhere), select a branch, and press `F2` or `Ctrl+R`; its existing upstream configuration is preserved by Git.

## Keyboard

The UI intentionally stays uncluttered; press `?` for the complete shortcut reference.

| Key | Action |
|---|---|
| `j` / `k`, arrows | Move through every file/commit or scroll the preview |
| Mouse wheel | Naturally scroll the pane under the pointer |
| `Tab` / `Enter` | Toggle sidebar/preview focus |
| `z` | Hide/show the sidebar |
| `e` / `E` | Collapse/expand every file diff; keep that preference across selections/views |
| `1` / `2` / `3` | Changes/history/pull requests (all states) |
| `s` or `Space` | Toggle stage/unstage for the selected file |
| `a` / `U` | Stage all/unstage all |
| `c` | Open commit editor; `Ctrl+Enter` submits |
| `t` | Toggle compact hunks/full-file context |
| `v` | Toggle unified/side-by-side diff |
| `x` | Discard selected change after confirmation |
| `b` in History | View another local/remote branch without checkout |
| `b` elsewhere / `B` | Checkout branch picker; `F2`/`Ctrl+R` renames a local branch |
| `o` in Pull Requests | Select the repository whose PRs are shown |
| `←` / `→` in Pull Requests | Previous/next 25-item PR page |
| `,` / `.` in Pull Requests | Previous/next 20-file diff page |
| `r` | Refresh status; bypass PR metadata cache in the PR view |
| `/` | Filter changes/history, or focus exact numeric PR lookup |
| `[` / `]` | Previous/next diff hunk |
| `:` or `Ctrl+P` | Command palette |
| `?` | Shortcut help |
| `q` | Quit |

Drag the divider between the file list and preview to resize the main panes. In side-by-side mode, drag the center divider to resize old/new sides.

Text fields support Unicode-safe editing plus familiar terminal and macOS motions: Option/Ctrl+Arrow moves by word, Option/Ctrl+Delete removes a word, Command+Arrow moves to a line boundary, Command+Delete removes to a line boundary, and Ctrl+A/E/B/F/W/U/K are supported.

## Performance and Safety

- Rendering and key handling never invoke Git directly.
- Fixed, coalescing mailboxes replace obsolete reads; local previews, PR/network previews, and background metadata run independently so one slow request cannot block tab switching.
- Filesystem event storms collapse into authoritative status snapshots.
- Preview requests carry generations so stale replies are ignored.
- History is paginated for one explicit branch revision; choosing another branch is read-only.
- PR discovery loads one selected repository progressively in 50-record cursor batches, caps the snapshot at 10,000 PRs, and exposes it in local 25-row pages. Exact lookup accepts only a positive integer and always includes the canonical base-repository URL.
- Pull-request discovery inspects at most 32 Git remotes, 64 configured fetch/push URL entries (32 distinct URLs), and 16 GitHub repositories.
- PR changed paths are capped at 4,096 / 2 MiB, patches at 8 MiB, and each preview fetches at most 20 files. Potentially large subprocess stdout is streamed and the child is terminated at the cap.
- PR previews use locally available immutable OIDs first. Missing Git history deepens only to 4,096 commits in a disposable bare repository. No checkout, worktree mutation, or persistent source-repository ref is used.
- Cached `gh` metadata is bounded to 32 MiB and 256 entries; fresh batch entries live for 60 seconds and exact PR entries for five minutes.
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
