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
- Paginated commit history with one commit-details card and a titled diff pane for every changed file
- GitHub CLI-backed open pull requests and per-file diffs across multiple fetch/push remotes and forks
- Commit, amend, stash, fetch, pull, push, sync, cherry-pick, and revert
- Local branch switching, creation, rename, deletion, and creation at a selected commit
- Natural mouse scrolling, clickable rows, and draggable pane dividers
- Keyboard-first filtering, command palette, modal text editing, and accessibility help
- Coalesced background Git work, stale-result rejection, and bounded diff output

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

### Pull requests and remotes

Press `3` to load open pull requests. Quinjet asks `gh` to resolve every distinct fetch **and** push URL from the repository's Git remotes, then combines the results. This supports common fork setups such as `origin` pointing to your fork and `upstream` pointing to the base repository, as well as a single remote with a separate push URL. Duplicate remote URLs are collapsed.

Each PR preview identifies its base and head repository, branches, matching local remote aliases, and whether it crosses a fork boundary. Diffs are requested with an explicit base-repository URL, so a PR number from one remote cannot accidentally resolve against another. GitHub Enterprise hosts configured in `gh` are supported too. Press `r` in the Pull Requests view to reload it.

Branch rename is local and deliberately does not delete or create remote branches. Open the branch picker with `b`, select a branch, and press `F2` or `Ctrl+R`; its existing upstream configuration is preserved by Git.

## Keyboard

The UI intentionally stays uncluttered; press `?` for the complete shortcut reference.

| Key | Action |
|---|---|
| `j` / `k`, arrows | Move through every file/commit or scroll the preview |
| Mouse wheel | Naturally scroll the pane under the pointer |
| `Tab` / `Enter` | Toggle sidebar/preview focus |
| `z` | Hide/show the sidebar |
| `e` | Collapse/expand every file diff |
| `1` / `2` / `3` | Changes/history/open pull requests |
| `s` or `Space` | Toggle stage/unstage for the selected file |
| `a` / `U` | Stage all/unstage all |
| `c` | Open commit editor; `Ctrl+Enter` submits |
| `t` | Toggle compact hunks/full-file context |
| `v` | Toggle unified/side-by-side diff |
| `x` | Discard selected change after confirmation |
| `b` | Branch picker; `F2`/`Ctrl+R` renames the selected local branch |
| `r` | Refresh status (and reload PRs when that view is active) |
| `/` | Filter the active list |
| `[` / `]` | Previous/next diff hunk |
| `:` or `Ctrl+P` | Command palette |
| `?` | Shortcut help |
| `q` | Quit |

Drag the divider between the file list and preview to resize the main panes. In side-by-side mode, drag the center divider to resize old/new sides.

Text fields support Unicode-safe editing plus familiar terminal and macOS motions: Option/Ctrl+Arrow moves by word, Option/Ctrl+Delete removes a word, Command+Arrow moves to a line boundary, Command+Delete removes to a line boundary, and Ctrl+A/E/B/F/W/U/K are supported.

## Performance and Safety

- Rendering and key handling never invoke Git directly.
- A coalescing worker mailbox replaces obsolete read requests.
- Filesystem event storms collapse into authoritative status snapshots.
- Preview requests carry generations so stale replies are ignored.
- History is paginated and all Git/PR diff output is bounded.
- Pull-request discovery inspects at most 32 Git remotes, 64 configured fetch/push URL entries (32 distinct URLs), 16 GitHub repositories, 100 open PRs per repository, and 500 combined PRs.
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
