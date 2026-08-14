# Quinjet Architecture

## Design Goals

Quinjet optimizes for immediate input response, authoritative Git behavior, and predictable memory use. The terminal render path never spawns Git or performs filesystem traversal.

## Layers

```text
Terminal events
      │
      ▼
App state + intents ───────────────► Ratatui renderer
      │                                  ▲
      ▼                                  │ latest snapshot
bounded Git command queue                │
      │                                  │
      ▼                                  │
Git worker ── Git CLI ── parsed events ──┘
      ▲
      │
filesystem watcher (coalesced signal)
```

- `src/app.rs`: focus, selection, modal, command, lazy-file, live-check, progress, and generation state.
- `src/git/mod.rs`: safe argv construction, reusable index-first local diff workspaces, branch/stash workflows, branch-scoped history reads, and user-facing Git operations.
- `src/git/status.rs`: byte-oriented porcelain-v2 status parser.
- `src/git/history.rs`: delimiter-safe history parser.
- `src/git/github.rs`: on-demand repository-scoped PR lookup, live checks, a reusable local PR workspace, and per-file Git diff generation; it has no repository-wide PR-list operation.
- `src/git/diff.rs`: unified-diff model and syntax highlighting for working-tree, commit, and PR patches.
- `src/git/worker.rs`: the non-blocking UI/Git boundary.
- `src/watch.rs`: repository watcher and event-storm coalescing.
- `src/ui/`: viewport-only rendering, diff layouts, theme, and mouse hit map.

## Responsiveness Invariants

1. The UI thread mutates only in-memory state and renders visible rows.
2. Every preview, status, history, branch, and pull-request request carries a generation; stale replies are ignored.
3. Fixed coalescing mailboxes isolate status/history, explicit GitHub metadata and check lookup, local change/commit/branch/stash previews, and potentially network-bound PR previews. Key repeat remains constant-space, slow GitHub work cannot block tab switches, and ordered user mutations are serialized by app state. No GitHub command is queued at startup or merely by opening the PR tab.
4. Watcher signals are lossy by design: one full status snapshot subsumes all preceding file events.
5. Git/PR patch output is capped at 8 MiB per selected file. Exact PR/check metadata is capped at 2 MiB, changed-file indexes at 8 MiB / 16,384 entries, repository discovery at 16 identities, and adaptive selected-PR history fetches at 4,096 commits. Complete multi-file patches are never materialized.
6. Potentially large local and PR subprocess output is read through capped pipes. Crossing a cap kills the child rather than first allocating all output and truncating afterward.
7. Git and GitHub CLI receive argv directly, never via a shell. Every explicit PR lookup carries its canonical base-repository identity after lazy remote discovery, avoiding ambient-remote ambiguity.
8. Working-tree groups, commits, branch comparisons, and stashes use one coalesced local workspace. A bounded name/status index produces collapsed headers first; the first path is prefetched silently, and later expansions run path-scoped Git commands whose documents remain cached for that workspace. Periodic status snapshots do not rebuild an unchanged comparison.
9. PR patches first use immutable base/head OIDs already present in the opened repository, which makes local-branch PR previews network-free. Missing objects fall back to fixed refs in one bare workspace under the cache/temp root: base and GitHub PR-head refs are shallow/partial fetched, the merge base is deepened adaptively, and the workspace remains alive while files are selected. Each selection runs a path-scoped Git diff; `Drop` removes the workspace. The opened repository receives no checkout, branch, ref, index, or worktree mutation.
10. The PR file tree is built from a bounded `git diff --name-status -z` index and renders only viewport rows. Its default all-files document reuses that index with collapsed headers and cached path documents; selecting a tree file enters single-file mode, while selecting the Files tab restores the index. Files and directories are selectable, directory rows expose real collapse/expand controls, and hidden descendants never trigger diff work. File-selection requests are generation-tagged and coalesced; there are no changed-file pages.
11. PR checks are requested only after an explicit PR lookup and refresh every ten seconds. Refresh failures preserve the previous check snapshot, so polling causes no empty-state flicker or layout shift.
12. Successful raw `gh` metadata responses are atomically cached under the per-user Quinjet cache root. TTL reads, stale-on-error fallback, private Unix modes, a 32 MiB / 256-entry prune limit, and `QUINJET_CACHE_DIR` keep it useful and bounded. Diffs and credentials are never cached.
13. Read operations set `GIT_OPTIONAL_LOCKS=0`; `gh` runs with prompts, paging, color, and update checks disabled on the worker thread.

## VS Code Research

The ignored `extras/vscode` checkout was explored independently. Its central pattern is a provider-neutral SCM workbench driven by a Git-CLI-backed provider. Quinjet adopts the same high-level boundary in a smaller form: UI/app code works with typed snapshots while `git` owns repository semantics.

Important parity findings incorporated in the first implementation:

- Separate merge, staged, and worktree resource groups with per-file and per-group actions.
- Stash creation variants, listing, preview, apply, pop, drop, and clear flows.
- Read-only current-branch comparison against local or remote-tracking branches.
- Event-driven refresh with noise filtering and snapshot reconciliation.
- Git CLI delegation for hooks, credentials, config, refs, and index semantics.
- Paginated history for an explicit branch revision and lazy commit preview.
- Side-by-side and unified syntax-highlighted diff surfaces.
- Context-sensitive operations, confirmations, keyboard help, and optional mouse support.

## Deliberate Next Steps

Full VS Code parity is an ongoing scope rather than a single release. Highest-priority follow-ups are multi-repository actors, non-UTF-8 path preservation, partial hunk staging, a true three-way merge editor, ref-filtered topology lanes, and configurable keymaps.
