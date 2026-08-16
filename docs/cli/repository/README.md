# `quinjet status`, `diff`, `log`, `show`

These four verbs are the reading group. They answer the four questions a source
control tool is asked most often: what has changed, what exactly changed, what
happened before, and what one commit did. Between them they are the command
line form of the terminal interface's Changes tab, its diff pane and its History
tab, and they are the only verbs in Quinjet that read a repository without
naming a branch, a stash or a pull request.

Nothing here writes. Every Git process this group starts is a read, and each one
runs with `GIT_OPTIONAL_LOCKS=0`, so `quinjet status --watch` in one window
never contends for `index.lock` with a `git commit` in another. Every process
also gets `-C <worktree root>`, `-c core.quotepath=false`, `LC_ALL=C` and
`GIT_TERMINAL_PROMPT=0`. There is no shell: `git` receives an argument array, so
a path containing a space, a quote or a newline is one argument and stays one
argument.

The four verbs sit on three Git reads. `status` is one
`git status --porcelain=v2 --branch -z --untracked-files=all
--ignore-submodules=none`, parsed byte by byte rather than by line, so a path
with a newline in it survives. `log` is one `git log --topo-order
--decorate=short --no-color --skip=N --max-count=L --format=<record>`, whose
format uses unit separator `\x1f` between fields and record separator `\x1e`
between commits so that a subject full of tabs and pipes still parses. `diff`
and `show` both build a diff document: an index read that names the files, then
one `git diff` per file, composed into a single document in Rust.

That composition is the thing worth understanding. Quinjet never prints Git's
patch. It parses it into a line model, one `DiffLine` per row with its old and
new line number and its syntax-highlighted spans, and prints that model back
out. Consequences follow: tabs come out as spaces at four-column stops, the
`diff --git`, `index`, `---` and `+++` lines are gone and replaced by one
label line per file (`src/main.rs  · modified +12 -3`), and the result is
therefore not something `git apply` will accept. It is made to be read, not
replayed.

`diff` has one more trick. `git diff` shows nothing at all for an untracked
file, because Git does not track it, but the Changes tab lists it and so does
`quinjet status`. So for an untracked path Quinjet synthesizes the patch itself
in Rust: it reads the file off the filesystem, emits a `new file mode 100644`
header, one `@@ -0,0 +1,<lines> @@` hunk, and every line prefixed with `+`. A
file containing a NUL byte gets a `Binary files /dev/null and b/<path> differ`
patch instead, and so does anything that is not a regular file.

`show` reaches its commit through `log`, not through `git show`. It resolves the
revision, reads exactly one entry of history at it, and then diffs that commit
against its first parent. A root commit has no parent, so that one case falls
back to `git diff-tree --root` for the file list and `git show --format=` for
each file's patch. Everything else, including a merge, uses the first parent.

## At a glance

| Command | What it does |
| --- | --- |
| `quinjet status` | Prints the branch, its upstream and divergence, and every change grouped into conflicts, staged and unstaged. Takes `--watch`. |
| `quinjet diff` | Prints the working-tree patch, optionally limited to the index, to the worktree, or to a set of path prefixes. |
| `quinjet log` | Prints commits from any revision, newest first, in topological order. |
| `quinjet show` | Prints one commit's metadata and its patch. |

## Commands

- [`quinjet status`](./status.md)
- [`quinjet diff`](./diff.md)
- [`quinjet log`](./log.md)
- [`quinjet show`](./show.md)

## Exit codes

| Code | When this group produces it |
| --- | --- |
| 0 | Any successful read. Also `diff` when no change matched the filters (it prints `No changes match`), `log` when the range is empty (it prints nothing, or `[]` under `--json`), a clean working tree, `--help` on the group or any verb, and any verb whose reader closed the pipe early, which stops the output and prints nothing on stderr. |
| 1 | `-C` did not point into a Git repository (`Not a Git repository: fatal: not a git repository ...`); `log` or `show` was run on an unborn branch (`Git command failed: fatal: bad revision 'HEAD'`); or any Git process failed. |
| 2 | clap rejected the command line: `--staged` together with `--unstaged`, an unknown flag, a positional argument on `status`, or `-n -1`, which clap reads as a flag. |
| 3 | `log` or `show` was given a revision that names nothing (`` `nope` does not name a commit in this repository ``) or one that starts with `-` or is empty (`` refusing to resolve `-n` as a revision ``). Both carry the hint `` run `quinjet log` or `quinjet branch list --all` for what this repository holds ``. `show` adds one more case, the narrow one where the revision resolved but the one-entry history read came back empty, which produces the same sentence without a hint. |

Nothing in this group exits 4. That code belongs to a check run with no readable
log, which is a pull-request concern.

## Notes and gotchas

- Paths are always relative to the worktree root, never to your current
  directory. `-C` and Quinjet's own discovery both resolve to the top of the
  worktree, so running `quinjet diff sync_wiki.py` inside `scripts/` matches
  nothing; `quinjet diff scripts/sync_wiki.py` from anywhere in the checkout
  matches.
- The path filter on `diff` is a component-wise prefix test
  (`Path::starts_with`), not a glob and not a substring. `docs` matches
  `docs/cli/README.md`; `doc` matches nothing; `./docs` matches nothing, because
  the leading `.` is a real path component; an absolute path matches nothing,
  because the change paths are relative. There is no way to spell a glob, so
  narrow with a directory prefix and filter the rest with `jq` or `grep`.
- `status` orders its output the same way every time: conflicts, then staged,
  then unstaged, and inside each group by path as a plain byte-wise string
  comparison. The JSON `changes` array is in exactly that order, so two reads of
  an unchanged tree produce byte-identical documents.
- A file that is modified in the index and modified again in the worktree
  appears twice, once under `Staged Changes` and once under `Changes`, because
  porcelain v2 reports an `X` code and a `Y` code per record and Quinjet emits
  one change for each non-`.` code. `--staged` and `--unstaged` on `diff` are
  how you pick one of the two.
- `--untracked-files=all` means untracked directories are expanded. A new
  directory of 200 files is 200 rows, not one. Anything `.gitignore` covers is
  absent: no verb here passes `--ignored`.
- `--ignore-submodules=none` means a submodule whose HEAD has moved is reported
  as a modified path. Its patch is whatever `git diff` prints for a gitlink,
  which is a one-line `Subproject commit` change, not the submodule's own diff.
- On an unborn branch `status` works and prints `On branch main` with
  `"oid": null`, but `log` and `show` fail with
  `Git command failed: fatal: bad revision 'HEAD'` and exit 1, because there is
  no `HEAD` to resolve. `diff` works, because untracked files do not need one.
- On a detached HEAD `status` prints `HEAD detached at <first 8 characters of
  the oid>` and sets `"detached": true`, and the `head` field carries those
  eight characters rather than a branch name. If the oid is somehow unknown the
  literal string `detached` is used instead.
- `Tracking <upstream> ahead N behind M` is printed only when the branch has an
  upstream, and the counts come from porcelain v2's `# branch.ab` header, which
  Git computes against the remote-tracking ref on disk. They are as stale as
  your last fetch. Nothing in this group fetches.
- `log` is topological, not chronological. `--topo-order` keeps a branch's
  commits contiguous, so after a merge the order can differ from what a
  date-sorted `git log` would print, and `relativeDate` values can appear out of
  sequence.
- `log`'s author and email come from `%aN` and `%aE`, the mailmap-respecting
  forms, so `.mailmap` rewrites what you see. The committer is carried in the
  JSON as `committer` and `committerEmail` but is never printed in the text
  table, which shows only the author.
- Revision resolution is shared by `log`, `show`, `branch create`,
  `cherry-pick` and `revert`. It refuses anything that is empty or starts with
  `-` before Git sees the string, then tries `git rev-parse
  --symbolic-full-name --verify --quiet <rev>` and keeps the answer only if it
  is under `refs/heads/`, `refs/remotes/` or `refs/tags/`, then falls back to
  `git rev-parse --verify --quiet <rev>^{commit}`. `HEAD` is answered without
  running Git at all. So `HEAD`, `main`, `origin/main`, `vX.Y.Z`, `e2d95c2` and
  `HEAD~3` all work. `main` becomes `refs/heads/main`, `origin/main` becomes
  `refs/remotes/origin/main` and `vX.Y.Z` becomes `refs/tags/vX.Y.Z`, because a
  ref keeps its full name; only a short id or a `~`/`^` expression is
  normalized to a full 40 character object id. Whatever fails to resolve exits
  3, not 1, with the hint `` run `quinjet log` or `quinjet branch list --all`
  for what this repository holds ``.
- `cherry-pick` and `revert` are preview-first revision mutations. After
  resolving the revision, each reports what it would do and exits 0 without
  changing `HEAD` unless `--yes` is present. Their process tests compare the
  revision before and after both the preview and confirmed forms.
- `history` then applies its own whitelist and refuses anything that is not
  `HEAD`, a `refs/heads/`, `refs/remotes/` or `refs/tags/` ref, or a full object
  id of 40 or 64 hex characters. Because resolution only ever produces one of
  those, that guard is unreachable from the command line; it exists so no other
  caller can smuggle an option into `git log`.
- The 8 MiB patch cap is per file, not per document. A file whose patch crosses
  it has its `git` process killed at the boundary rather than being buffered and
  trimmed afterwards, the patch is cut back to the last complete line, a
  `… diff truncated to keep Quinjet responsive …` row is appended inside that
  file, and the whole document gains `"truncated": true` plus a trailing
  `[output reached Quinjet's size cap and was truncated]` line. A 300 file diff
  where every file crosses the cap therefore produces roughly 2.4 GiB of output,
  not 8 MiB.
- The 16,384-path index cap applies to `show`, because its file list comes from
  `--name-status`. It does not apply to `diff`, whose file list comes from the
  status parse and is unbounded. The same is true of the 8 MiB cap on the index
  read itself.
- Reading a diff costs one `git diff` process per file plus one status read plus
  at most two `--numstat` reads. `quinjet diff` in a tree with a thousand
  untracked files starts a thousand child processes. Scope it with a path.
- `--expanded` is `--unified=1000000`. It is a very large finite number, not
  infinity, so a file with more than a million lines of context between two
  changes would still be split into hunks. In practice it means whole file.
- The `--numstat` reads exist to fill each file header's `+n -n` before its
  patch has been produced. They are treated as decoration: if one fails or is
  bounded away the header falls back to `+? -?` and nothing else changes. Once a
  file's patch has loaded, the counts shown are the parsed ones from the patch.
- The text form is not a patch. Tabs are expanded to four columns, `diff --git`
  and `index` lines are dropped, `---`/`+++` are consumed into the header, and
  the file header is Quinjet's own label line. `git apply` will refuse it. Use
  `git diff` if you want something to apply.
- Binary files are handled but not rendered: the file's rows become the literal
  `Binary files a/... and b/... differ` or `GIT binary patch` line and every
  line after it in that file is passed through as a `meta` row.
- Syntax highlighting runs on the command line too, and lands in `--json` as RGB
  triples on each span. To recover plain text from a JSON line, concatenate
  `spans[].text`, except on a `file-header` row, where the three spans are
  joined with a single space. Highlighting is skipped for a patch over 512 KiB
  or a line over 32 KiB, which changes the spans but never the text.
- Piping to a program that stops reading, `head` above all, is not an error. A
  broken pipe is recognized while reporting and turns into exit 0 with nothing
  on stderr, so `quinjet show HEAD | head -20` prints its twenty lines and
  returns 0. Output stops where the reader stopped, which means a truncated
  document, not a truncation marker.
- `status --watch` never finishes on its own. Its frames are always marked
  unfinished, so it runs until Ctrl+C. `--interval` has a floor of 1 second; a
  smaller value, including `0`, is silently raised to 1. The other `--watch`
  flags in Quinjet, on `pr checks` and `pr logs`, have floors of 2 and 3 seconds
  and do stop.
- Watching repaints only when stdout is a terminal and `--json` is off. On a
  terminal each frame is preceded by `\x1b[H\x1b[2J` and followed by
  `watching, refreshing every Ns (Ctrl+C to stop)`. Redirected, the frames
  simply append, which makes `quinjet status --watch > log.txt` a readable log
  rather than a file of escape sequences.
- `--watch` exists on `status` alone in this group. `diff`, `log` and `show`
  have no watching form; run them in a loop, or open the terminal interface,
  which refreshes them from a filesystem watcher.
- None of these verbs uses the on-disk cache. Every invocation re-reads the
  repository. The cache holds pull-request material only.
- A verb always beats a directory of the same name. In a repository containing a
  directory called `status`, `quinjet status` is the verb; `quinjet ./status`
  opens the directory in the terminal interface.

## Where to go next

- [Conventions and contracts](../conventions.md) for the `--json` rules, the
  stdout/stderr split and the full exit-code table these pages build on
- [`quinjet stage`, `unstage`, `discard`, `commit`, `resolve`](../changes/README.md)
  for the verbs that act on what `status` and `diff` report
- [`quinjet branch`](../branch/README.md) for `branch compare`, which is the
  same diff machinery pointed at another branch without a checkout
- [`quinjet pr`](../pull-request/README.md) for reading a patch that is not in
  your worktree
- [All `quinjet` commands](../README.md)
