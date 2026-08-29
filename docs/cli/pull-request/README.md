# `quinjet pr`

`quinjet pr` reads and operates on one pull request by number. Its read verbs
cover metadata, commits, changed files, patches, the conversation, checks, one check
run's log, and browser opening. Its write verbs cover the full existing-PR
workflow exposed by GitHub CLI, plus merge-queue removal, notification
subscriptions, and fork maintainer-edit access through GitHub's GraphQL API.
The `reviews` family adds threaded line and file feedback, replies, pending
review state, and thread resolution without replacing the one-shot `review`
verb.
There is no listing, checkout, or creation verb. Reach for this group when you
want the same operations in a pipe, and reach for the pull-request pane of
[the terminal interface](../tui.md) when you want to work interactively.

Every verb begins with the same lookup, so understanding that one step explains
most of the group's behavior. Quinjet first works out which GitHub repositories
this checkout points at by reading `git remote`, then `git remote get-url --all`
and `git remote get-url --push --all` for each of them, canonicalising each URL
and asking `gh repo view <url> --json nameWithOwner,url` for anything it cannot
recognize as `github.com` on its own. It then runs one command for the pull
request itself:

```text
gh api graphql --hostname <host> -f owner=<owner> -f name=<name> \
  -F number=<number> -f query=<pull-request-query> \
  --jq '[...] | @tsv'
```

The query reads content, refs, viewer permissions, review state, merge state,
auto-merge, merge queue, lock, and subscription data in one response. Its `--jq`
program flattens the record to one tab-separated line of exactly 38 fields. The
canonical repository identity supplies the GraphQL host, owner, and name, so a
GitHub Enterprise host stays attached to the number. `--repo owner/name` on the
command line selects among the
repositories discovery found, matching case-insensitively on `owner/name` or on
a URL suffix; when nothing matches, the verb exits 3 and points at
[`quinjet repos`](../remotes/README.md).

Anything the lookup notices that is not fatal is a warning on stderr, never a
silent adjustment. A remote `gh` could not resolve, a repository count that hit
its cap, a selected repository that is no longer configured, and above all a
stale cache used because GitHub was unreachable, all print as
`warning: <message>` while stdout still carries a complete answer. That last one
matters: `quinjet pr view 12` can succeed with metadata that is hours old, and
the only sign is the line
`warning: GitHub is unavailable; showing stale cached metadata for #12`.

`files` and `diff` need commits rather than metadata, and they get them without
touching your repository. When your checkout already contains both `baseRefOid`
and `headRefOid`, Quinjet diffs them in place and spends no network at all.
When it does not, it creates a bare repository under the cache root, fetches
`+refs/heads/<base>:refs/quinjet/base` and `+refs/pull/<n>/head:refs/quinjet/head`
into it with `git fetch --no-tags --filter=blob:none --depth=<n>`, deepens
through 64, 256, 1,024 and 4,096 commits until `git merge-base` answers, and
deletes the whole directory when the process ends. Nothing is ever written to
your index, your worktree or your refs. The consequence is that a command-line
invocation pays that fetch every time while the terminal interface pays it once
and keeps the workspace open, which is why `pr diff` from a script is worth
caching yourself if you call it in a loop.

`checks` and `logs` are the two verbs built for CI rather than for reading.
`checks` shells out to `gh pr checks`, whose exit status is 1 when a run failed
and 8 when one is still pending, so Quinjet judges the response by its content
and accepts either code as long as something came back on stdout. `logs` takes
the `link` of a check run, pulls the Actions job id out of the trailing
`/job/<id>`, and reads `repos/<owner>/<name>/actions/jobs/<id>` for the steps and
`.../logs` for the archive, then attaches each timestamped log line to the step
whose window contains it. `view`, `conversation`, `checks`, and `logs` take
`--watch`. The first two keep refreshing until stopped; the latter two stop
when their check state settles.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet pr view` | Prints one pull request's metadata and its description, optionally refreshing until stopped. |
| `quinjet pr commits` | Lists the pull request's commits in chronological order. |
| `quinjet pr files` | Lists the files the pull request changes, with per-file line counts. |
| `quinjet pr diff` | Prints the whole patch, or one path's patch. |
| `quinjet pr conversation` | Prints the timeline and the inline review comments as one thread, optionally refreshing until stopped. |
| `quinjet pr checks` | Lists the checks, optionally blocking until they settle. |
| `quinjet pr logs` | Prints one check run's steps and its GitHub Actions log. |
| `quinjet pr open` | Hands the pull request URL, or one selected check URL, to the desktop browser. |
| `quinjet pr merge` | Merges the pull request with `--merge`, `--squash`, or `--rebase` after `--yes`. |
| `quinjet pr admin-merge` | Uses administrator privileges to merge despite unmet requirements. |
| `quinjet pr auto-merge` | Enables automatic merging with the selected merge method. |
| `quinjet pr disable-auto-merge` | Disables an active automatic merge request. |
| `quinjet pr dequeue` | Removes the pull request from its merge queue. |
| `quinjet pr ready` / `draft` | Changes whether an open pull request is ready for review. |
| `quinjet pr review` | Approves, comments on, or requests changes on the pull request. |
| `quinjet pr reviews` | Reads review threads and authors, submits, or resolves pending review feedback. |
| `quinjet pr comment` | Adds a conversation comment. |
| `quinjet pr edit-last-comment` / `delete-last-comment` | Updates or removes the viewer's latest conversation comment. |
| `quinjet pr edit` | Changes title, description, base, assignees, labels, projects, reviewers, or milestone. |
| `quinjet pr update-branch` | Merges or rebases the base branch into the pull request branch. |
| `quinjet pr lock` / `unlock` | Changes whether the conversation accepts new comments. |
| `quinjet pr subscribe` / `unsubscribe` | Changes notification subscription state. |
| `quinjet pr allow-maintainer-edits` / `disallow-maintainer-edits` | Changes whether maintainers can edit a fork's head branch. |
| `quinjet pr revert` | Creates a pull request that reverts a merged pull request. |
| `quinjet pr close` | Closes the pull request without merging, after `--yes`. |
| `quinjet pr reopen` | Reopens a closed pull request that has not been merged, after `--yes`. |

## Commands

- [`quinjet pr view`](./view.md)
- [`quinjet pr commits`](./commits.md)
- [`quinjet pr files`](./files.md)
- [`quinjet pr diff`](./diff.md)
- [`quinjet pr conversation`](./conversation.md)
- [`quinjet pr checks`](./checks.md)
- [`quinjet pr gate`](./gate.md)
- [`quinjet pr logs`](./logs.md)
- [`quinjet pr open`](./open.md)
- [`quinjet pr merge`](./merge.md)
- [`quinjet pr admin-merge`](./admin-merge.md)
- [`quinjet pr auto-merge`](./auto-merge.md)
- [`quinjet pr disable-auto-merge`](./disable-auto-merge.md)
- [`quinjet pr dequeue`](./dequeue.md)
- [`quinjet pr ready`](./ready.md)
- [`quinjet pr draft`](./draft.md)
- [`quinjet pr review`](./review.md)
- [`quinjet pr reviews`](./reviews.md)
- [`quinjet pr comment`](./comment.md)
- [`quinjet pr edit-last-comment`](./edit-last-comment.md)
- [`quinjet pr delete-last-comment`](./delete-last-comment.md)
- [`quinjet pr edit`](./edit.md)
- [`quinjet pr update-branch`](./update-branch.md)
- [`quinjet pr lock`](./lock.md)
- [`quinjet pr unlock`](./unlock.md)
- [`quinjet pr subscribe`](./subscribe.md)
- [`quinjet pr unsubscribe`](./unsubscribe.md)
- [`quinjet pr allow-maintainer-edits`](./allow-maintainer-edits.md)
- [`quinjet pr disallow-maintainer-edits`](./disallow-maintainer-edits.md)
- [`quinjet pr revert`](./revert.md)
- [`quinjet pr close`](./close.md)
- [`quinjet pr reopen`](./reopen.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | The verb printed its answer. Also `pr checks --watch` when every check settled and none failed, `pr logs --watch` when the run finished without failing, `--help` on the group or any verb, and any write preview that printed what it would do without `--yes`. |
| 1 | The lookup failed: `gh` is missing or unauthenticated, no configured fetch or push remote resolves to a GitHub repository, the number does not exist, or the number was `0`. Also `files` and `diff` when preparation failed, check and log verdict failures, browser-opening failures, and any write GitHub rejected because of state, permissions, protection rules, or an unsupported repository feature. |
| 2 | clap rejected the command line: a missing `<NUMBER>`, a `<NUMBER>` that is not an unsigned integer, `quinjet pr` with no verb, `pr merge` without exactly one of `--merge` / `--squash` / `--rebase`, or an unknown flag. |
| 3 | `--repo` matched none of the discovered repositories, `pr diff <n> <path>` named a path that is not part of the pull request, or a check name passed to `pr logs` or `pr open --check` matched zero checks or more than one. Every one of these prints a `hint:` listing the valid choices. |
| 4 | `pr logs` found a check with no readable log, in either one-shot or watch mode, or `pr open --check` found a check with no browser URL. |

The exit 1 that `checks --exit-code`, `checks --watch` and `logs --watch`
produce is unlike every other exit 1 here, and worth stating plainly: the answer
is printed in full before the process exits. A red `--exit-code` run writes the
whole listing to stdout and then returns 1; a `--watch` run writes its final
frame, the settled listing or the finished log, and then returns 1. Both are
verdicts on the pull request rather than reports that the command failed, so
`quinjet pr checks 8 --exit-code --json > checks.json` leaves a complete
document behind even when it exits non-zero. Every other exit 1 in this table
leaves stdout empty.

## Notes and gotchas

- Every verb needs `gh` on `PATH` and authenticated, except that `pr open` needs
  it only for the lookup that precedes opening. `gh` runs with the repository as
  its working directory and with `GH_PROMPT_DISABLED=1`, `GH_PAGER=cat`,
  `GH_NO_UPDATE_NOTIFIER=1` and `NO_COLOR=1`, so `GH_TOKEN`, `GH_HOST` and
  `GH_REPO` behave exactly as they do for `gh` itself and a missing credential
  fails rather than prompting.
- Repository discovery is capped: 32 remotes, 64 configured fetch and push URL
  entries, 32 distinct canonical URLs, and 16 distinct GitHub repositories.
  Crossing any of them adds a warning naming the cap that was hit.
- Discovered repositories are sorted with any repository that has a remote
  called `origin` first, then by display name lower-cased. Without `--repo` the
  first of that list is used, so `origin` wins by construction and a fork added
  as a second remote never silently takes over.
- `--repo` selects from the cached repository list and is always read with
  `refresh: false`, even when you also pass `--refresh`. `--refresh` reaches the
  repository list only through the lookup that follows it. If you renamed a
  remote in the last day and `--repo` cannot see it, run
  `quinjet repos --refresh` once.
- The lookup is not a search. `--repo` narrows which repository the number is
  read from; it never changes the number. There is no way to ask Quinjet for a
  list of open pull requests, by design: it reads one pull request at a time.
- A cross-repository pull request from a fork whose head repository was deleted
  still resolves for `view`, `commits`, `conversation`, `checks` and `logs`,
  because those
  read the base repository. `files` and `diff` may not: if
  `refs/pull/<n>/head` has also gone, the fetch fails with
  `the base repository no longer exposes the PR head and its fork was deleted`.
- Reading a pull request never writes to your repository. There is no checkout,
  no ref, no index change and no stash. The temporary workspace lives at
  `<cache root>/tmp/pr-<pid>-<id>.git`, falls back to the system temporary
  directory when the cache root is unusable, and is removed on drop. Leftovers
  from a killed process are cleaned up by the next run that finds them older
  than 24 hours, scanning at most 256 entries.
- The blob-filtered fetch is tried first and retried without `--filter=blob:none`
  if the remote refuses partial clone, which is what makes older GitHub
  Enterprise installations work. Both attempts run at the same depth.
- An unborn branch or a detached HEAD in your checkout changes nothing here.
  These verbs never read HEAD: they read remotes for discovery and object ids
  from GitHub for the diff. A brand new repository with a GitHub remote and no
  commits can still run `quinjet pr view 1`.
- The local fast path requires a full 40 or 64 character hex object id that
  `git cat-file -e <oid>^{commit}` accepts. A shallow clone that lacks the base
  commit takes the temporary-workspace path even though the repository is the
  right one.
- A force push during a read is a race with a visible outcome rather than a
  silent one. Metadata is cached for five minutes, so `pr files` and `pr diff`
  can prepare a workspace for a head commit that has just been replaced; the
  fetch of `refs/pull/<n>/head` then brings the new head and the patches are
  keyed by the new pair of commits. Pass `--refresh` when you know a push just
  landed.
- Caching is shared with the terminal interface and split by whether the key
  already names the content. The immutable keys are `pull-request-commits-v1`
  (keyed by base and head), `pr-files-v1`, `pr-numstat-v1` and `pr-patch-v1`
  (all keyed by merge base and head),
  `conversation-timeline-v1` and `conversation-comments-v1` (keyed by GitHub's
  `updatedAt` stamp), and `check-log-v1` and `check-steps-v1` for a settled job.
  The timed keys are `repository` at 24 hours, `pull-request-v4` at 5 minutes,
  and `checks-v1` at 30 seconds. `--refresh` skips only the timed ones.
- A single file's patch is cached only when it is at most 1 MiB, so one enormous
  generated file cannot evict the rest of a pull request from the cache.
- The whole cache is bounded to 128 MiB and 2,048 entries and pruned oldest
  first, with owner-only permissions. See
  [what is cached](../conventions.md#what-is-cached).
- `--watch` exists on `view`, `conversation`, `checks`, and `logs`. `view`,
  `conversation`, and `checks` default to 5 seconds and never go below 2;
  `logs` defaults to 8 seconds and never goes below 3. Lower values are usage
  errors, and `--interval` requires `--watch`.
- While watching, every read is forced (`refresh: true`), so the 30 second check
  cache is bypassed and the poll interval is what actually governs request rate.
- Watching writes one compact JSON document per read under `--json`, repaints
  the screen with `\x1b[H\x1b[2J` when stdout is a terminal, and simply appends
  when stdout is redirected. A redirected watch is therefore a readable log
  rather than a file of escape sequences.
- `pr checks --watch` never finishes on a pull request that reports no checks at
  all, because "settled" is defined as no pending check *and* a non-empty list.
  That is deliberate: a workflow that has not yet been scheduled looks identical
  to one that does not exist. Use `quinjet pr checks <n> --exit-code` if you
  need a single reading instead.
- `pr checks --watch` always reports the verdict, so clap rejects the redundant
  `--exit-code` combination.
- `pr logs` and `pr checks` disagree about what counts as bad. `checks` treats
  pending as unhappy and returns 1 for it; `logs --watch` returns 1 only for a
  failed run. Skipped, cancelled and unknown runs are 0 for both.
- `pr logs` applies the same unavailable-log check in one-shot and watch modes.
  A check that has no Actions log, or for which GitHub has published neither
  steps nor an archive, exits 4 as soon as that reading is encountered.
- `pr logs` matches its `<CHECK>` argument by exact name first, and only falls
  back to a case-insensitive substring match when no name matches exactly. A
  matrix job called `Format, lint, and test (ubuntu-latest)` is therefore
  reachable by its whole name, while the prefix `Format` matches three of them
  and exits 3.
- Log lines are attached to steps by comparing whole seconds. Runner output
  carries sub-second precision and the steps API reports whole seconds, so
  comparing the two as text would push everything written in a step's final
  second into the previous step. Lines before the first step or after the last
  one are returned loose and printed under a `Runner output` heading, which is
  where provisioning and teardown failures appear.
- Step numbers come from GitHub and are not contiguous. A job that skipped
  conditional steps prints `5.` and then `9.`, because those are the numbers
  GitHub assigned.
- The two empty states of `pr logs` are different. `unavailable` means there is
  nothing to read at all and exits 4. `logPending` means the steps are known but
  the runner has not written a line yet, and prints
  `Waiting for the runner to write its first output` while exiting 0.
- GitHub answers the log endpoint with 404 before a job has written anything and
  with 410 once retention expires. Neither is treated as an error: the steps are
  still shown, and the log simply reads as empty.
- `pr conversation` merges two endpoints, dedupes on `html_url`, sorts by
  timestamp and caps at 500 entries. When it truncates it drops the oldest
  entries but restores the opening post, so the description is never lost.
- `pr diff` restricts each `git diff` to the file's new path, so a rename is
  reported by `pr files` as `R path (from old)` with its true counts but is
  printed by `pr diff` as a whole-file addition. The two are consistent with
  Git, not with each other.
- Size caps are enforced by killing the child process rather than by allocating
  everything first: 8 MiB per patch read, 8 MiB and 16,384 paths for a changed
  file listing, 2 MiB for metadata, 8 MiB and 200,000 lines for a check log, and
  500 entries with 64 KiB per body and 8 KiB per quoted hunk for a conversation.
  Crossing a cap prints an explicit notice rather than looking complete.
- Titles longer than 16 KiB and descriptions longer than 256 KiB are cut at a
  character boundary and end in `…`.
- `pr open` hands a URL to a platform opener (`open` on macOS, `explorer` on
  Windows, `xdg-open` elsewhere) and does not wait on the child. Every write
  verb is preview-first and needs `--yes`. Existing pull-request actions use
  non-interactive `gh pr` commands. Queue, subscription, and maintainer-edit
  changes use GraphQL mutations keyed by the node identity from the same lookup.
  Checkout and new pull-request creation remain outside this group.
- `pr open --check <name>` resolves the check by exact name first and then by a
  unique case-insensitive substring, using the same selection rule as
  `pr logs`. It opens the check's `link`; a selected check with no link exits 4.
- Every one of these verbs is a `cli::Command` the terminal interface also
  issues, so a reading on the command line and a reading on screen come from the
  same code path and the same cache. See
  [one command layer, two callers](../conventions.md#one-command-layer-two-callers).

## Where to go next

- [Conventions and contracts](../conventions.md) for the `--json` guarantee, the
  stdout and stderr split, and the shared exit-code table
- [`quinjet repos`](../remotes/README.md) for the repository list `--repo`
  selects from, and for `--refresh` on that list
- [The terminal interface](../tui.md) for the pull-request pane these verbs
  mirror, including its live checks and foldable logs
- [`quinjet diff`](../repository/README.md) for the working-tree patch, which
  shares the diff model but never touches GitHub
- [All `quinjet` commands](../README.md)
