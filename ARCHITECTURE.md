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
webhook listener (loopback, opt-in)
```

- `src/app.rs`: focus, selection, modal, command, lazy-file, live-check, progress, and generation state.
- `src/git/mod.rs`: safe argv construction, reusable index-first local diff workspaces, branch/stash workflows, branch-scoped history reads, and user-facing Git operations.
- `src/git/status.rs`: byte-oriented porcelain-v2 status parser.
- `src/git/history.rs`: delimiter-safe history parser.
- `src/git/github/mod.rs`: on-demand repository-scoped PR lookup, a reusable local PR workspace, and per-file and batched Git diff generation; it has no repository-wide PR-list operation.
- `src/git/github/checks.rs`: check runs, their GitHub Actions job steps, and raw run logs reduced to timestamped, severity-tagged lines.
- `src/git/github/conversation.rs`: the pull-request timeline and inline review comments flattened into one ordered thread.
- `src/git/diff.rs`: unified-diff model and syntax highlighting for working-tree, commit, and PR patches.
- `src/git/worker.rs`: the non-blocking UI/Git boundary.
- `src/watch.rs`: repository watcher and event-storm coalescing.
- `src/webhook.rs`: optional loopback listener that turns a forwarded GitHub delivery into a refresh signal.
- `src/ui/`: viewport-only rendering, diff layouts, theme, and mouse hit map.

## Responsiveness Invariants

1. The UI thread mutates only in-memory state and renders visible rows.
2. Every preview, status, history, branch, and pull-request request carries a generation; stale replies are ignored.
3. Fixed coalescing mailboxes isolate status/history, GitHub metadata (lookup, checks, conversation, check logs), local change/commit/branch/stash previews, and potentially network-bound PR previews. Background diff prefetch occupies its own mailbox slot behind the preview slot, so a queued batch can never displace the preview a reader is waiting for. Key repeat remains constant-space, slow GitHub work cannot block tab switches, and ordered user mutations are serialized by app state. No GitHub command is queued at startup or merely by opening the PR tab.
4. Watcher signals are lossy by design: one full status snapshot subsumes all preceding file events.
5. Git/PR patch output is capped at 8 MiB per read. Exact PR/check metadata is capped at 2 MiB, changed-file indexes at 8 MiB / 16,384 entries, conversations at 2,000 entries, check logs at 8 MiB / 200,000 lines, repository discovery at 16 identities, and adaptive selected-PR history fetches at 4,096 commits. Background prefetch stops after 400 files.
6. Potentially large local and PR subprocess output is read through capped pipes. Crossing a cap kills the child rather than first allocating all output and truncating afterward. Syntax grammar parsing stops at 512 KiB per patch or 32 KiB per row, and collapsed cached patches are not cloned into the combined document, so post-processing remains bounded after Git returns.
7. Git and GitHub CLI receive argv directly, never via a shell. Every explicit PR lookup carries its canonical base-repository identity after lazy remote discovery, avoiding ambient-remote ambiguity.
8. Working-tree groups, commits, branch comparisons, and stashes use one coalesced local workspace. A bounded name/status index produces collapsed headers first; the first path is prefetched silently, and later expansions run path-scoped Git commands whose documents remain cached for that workspace. Periodic status snapshots do not rebuild an unchanged comparison.
8a. Every index also reads `git diff --numstat` over the same range, so a header shows its real `+n -n` before that file has a patch. A file's totals never depend on whether its patch has loaded.
9. PR patches first use immutable base/head OIDs already present in the opened repository, which makes local-branch PR previews network-free. Missing objects fall back to fixed refs in one bare workspace under the cache/temp root: base and GitHub PR-head refs are shallow/partial fetched, the merge base is deepened adaptively, and the workspace remains alive while files are selected. Each selection runs a path-scoped Git diff; `Drop` removes the workspace. The opened repository receives no checkout, branch, ref, index, or worktree mutation.
10. The PR file tree is built from a bounded `git diff --name-status -z` index and renders only viewport rows. Its default all-files document reuses that index with collapsed headers and cached path documents; selecting a tree file enters single-file mode, while selecting the Files tab restores the index. Files and directories are selectable, directory rows expose real collapse/expand controls, and hidden descendants never trigger diff work. File-selection requests are generation-tagged and coalesced; there are no changed-file pages.
10a. Remaining patches arrive through batched background reads keyed to the prepared workspace rather than to a preview generation, so they can never invalidate a reader's own request. One Git invocation answers for a batch of paths and the combined patch is split back apart at its `diff --git` boundaries.
11. PR checks, the conversation, and metadata refresh on one adaptive poll: 5 seconds while a check is running, 20 seconds once they settle, 2 minutes from another view. Each read is issued and validated independently, refresh failures preserve the previous snapshot, and a poll never clears the loaded pull request, its section, or its cursors. A changed head OID reindexes only the diff. An opt-in loopback webhook listener runs the same refresh immediately; a delivery is a signal, never a source of displayed data.
11a. Check logs are read from the GitHub Actions job endpoint and attached to steps by comparing whole-second instants, because runner lines are sub-second while the steps API reports seconds. Output before the first step or after the last is kept separate, where provisioning and teardown failures live.
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
