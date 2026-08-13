# Quinjet

A fast, keyboard-first Git source-control interface for the terminal, written in Rust.

Quinjet keeps navigation and rendering independent from Git subprocess latency, watches the repository for live changes, and presents working-tree changes and commit history in one focused terminal UI.

## Current Features

- Live working-tree, index, conflict, branch, and ahead/behind refresh
- Staged, unstaged, untracked, renamed, deleted, and merge-conflict groups
- Syntax-highlighted unified and side-by-side diffs
- Paginated commit history with refs and commit patch previews
- Stage, unstage, discard, commit, amend, stash, fetch, pull, push, and sync
- Branch switch/create/delete and create-at-commit flows
- Cherry-pick and revert from history
- Keyboard-first navigation, filtering, command palette, and shortcut help
- Optional mouse selection and scrolling
- Background Git worker, filesystem event coalescing, stale-result rejection, and bounded diff output

## Install and Run

A recent stable Rust toolchain and Git are required.

```bash
git clone https://github.com/pulkitxm/quinjet.git
cd quinjet
cargo run --release -- /path/to/repository
```

Run without mouse capture if desired:

```bash
cargo run --release -- --no-mouse /path/to/repository
```

## Essential Keys

| Key | Action |
|---|---|
| `j` / `k`, arrows | Move selection or scroll preview |
| `Tab` | Switch sidebar/preview focus |
| `1` / `2` | Changes/history |
| `s` / `u` | Stage/unstage selected file |
| `a` / `U` | Stage/unstage all |
| `c` | Commit (`Ctrl+Enter` submits) |
| `x` | Discard selected change with confirmation |
| `b` | Branch picker |
| `f` / `l` / `p` / `y` | Fetch/pull/push/sync |
| `/` | Filter active list |
| `v` | Toggle unified/side-by-side diff |
| `[` / `]` | Previous/next diff hunk |
| `:` or `Ctrl+P` | Command palette |
| `?` | Complete in-app shortcut help |
| `q` | Quit |

## Performance Model

- Rendering and key handling never run Git commands directly.
- A bounded worker queue executes Git operations off the UI thread.
- Filesystem event storms collapse into authoritative status snapshots.
- Preview requests are debounced and tagged with generations so stale results are ignored.
- History is loaded in pages; diff output is capped before parsing/highlighting.
- Read-only Git commands use `GIT_OPTIONAL_LOCKS=0` and non-interactive execution.

## Development

```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

## Safety Notes

Destructive discard, branch deletion, cherry-pick, and revert flows ask for confirmation where appropriate. Git hooks, credentials, signing, configuration, and repository semantics are delegated to the installed Git CLI.
