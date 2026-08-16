# Getting started

Quinjet ships as one binary called `quinjet`. There is no daemon, no
configuration file to write before the first run, and nothing to initialize in a
repository. Install it, stand in a Git checkout, and run it. With no verb it
opens the terminal interface; with a verb it answers one question on stdout and
exits. Both faces are the same executable and the same command layer, so
nothing you learn here is specific to one of them.

The only hard runtime requirement is `git` on `PATH`. Quinjet never links
libgit2: every local operation is a real `git` subprocess with an argument
array, which is why your hooks, your credential helper, your signing
configuration and your `.gitattributes` all behave exactly as they do when you
type `git` yourself. The pull-request verbs additionally need the GitHub CLI,
`gh`, authenticated. Everything that is not a `pr` verb works without it.

This page covers installing, the shape of an invocation, the first handful of
commands worth running, how to explore the rest of the tree, and what a script
needs to know. The rules those commands obey are on
[conventions and contracts](./conventions.md), and this page links there rather
than repeating them.

## Installing

The installers download a prebuilt binary from GitHub Releases, verify its
SHA-256 checksum against the release's `SHA256SUMS`, and place it somewhere on
your `PATH`. Neither one needs Rust or Cargo.

On Linux or macOS:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh
```

On Windows PowerShell:

```text
powershell -c "irm https://quinjet.pulkit.page/install.ps1 | iex"
```

Where the binary lands:

| Platform | Directory |
| --- | --- |
| Linux, macOS | `$QUINJET_INSTALL_DIR`, else `$XDG_BIN_HOME`, else `~/.local/bin` |
| Windows | `$QUINJET_INSTALL_DIR`, else `%LOCALAPPDATA%\Programs\Quinjet\bin`, else `%HOME%\.local\bin` |

The shell installer takes `-v`/`--version`, `-b`/`--bin-dir`,
`--no-modify-path` and `-h`/`--help`, and reads the same three choices from
`QUINJET_VERSION`, `QUINJET_INSTALL_DIR` and `QUINJET_NO_MODIFY_PATH`. Pass them
after `sh -s --`:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh -s -- --version v0.0.6
curl --proto '=https' --tlsv1.2 -LsSf https://quinjet.pulkit.page/install.sh | sh -s -- --bin-dir /usr/local/bin
```

The PowerShell script takes `-Version`, `-BinDir` and `-NoModifyPath`, and reads
the same three environment variables.

From source, if you would rather build it:

```bash
cargo install quinjet
cargo install --git https://github.com/pulkitxm/quinjet --locked
```

Quinjet is edition 2024 and declares `rust-version = "1.85"`, so an older
toolchain will refuse the build rather than fail halfway through it.

### What the installers check, and what they do not

- They detect the operating system and CPU architecture themselves and pick one
  of `quinjet-linux-x86_64`, `quinjet-linux-aarch64`, `quinjet-macos-x86_64`,
  `quinjet-macos-aarch64` or `quinjet-windows-x86_64.exe`. On an Intel shell
  running under Rosetta the shell installer notices
  (`sysctl.proc_translated`), says `info: Rosetta detected; selecting the native
  Apple Silicon build`, and installs the `aarch64` build instead. On Windows
  ARM64 there is no native build, so the PowerShell script warns and installs
  the x64 one.
- The checksum step is not optional. The shell installer needs `curl` or
  `wget`, `awk`, and one of `sha256sum`, `shasum` or `openssl`, and it exits
  before writing anything if the recorded hash is missing, is not 64 hex
  characters, or does not match. The download is staged into a temporary file
  next to the destination and moved into place, so an interrupted install never
  leaves a half-written `quinjet` on your `PATH`.
- `PATH` is only edited when the destination is exactly `~/.local/bin` and
  `--no-modify-path` was not passed. The line goes into `config.fish` for fish
  (`fish_add_path "$HOME/.local/bin"`), `$ZDOTDIR/.zshrc` for zsh, `~/.bashrc`
  for bash, and `~/.profile` for anything else, under the comment
  `# Added by the Quinjet installer`. Choose your own `--bin-dir` and the
  installer will tell you the directory is not on `PATH` and leave your startup
  files alone. On Windows the entry is prepended to the user `Path` and needs a
  new terminal.
- **Both installers check for `git` and warn when it is missing. Neither one
  checks for `gh`.** A machine that installed cleanly can still fail the first
  `quinjet pr view`. See
  [what the pull-request verbs need](#what-the-pull-request-verbs-need).

## One binary, two faces

```bash
quinjet                      open the terminal interface in the current repository
quinjet ~/code/project       open it somewhere else
quinjet tui --no-mouse       the same thing, spelled as a verb, with the mouse released
quinjet status               print what its Changes tab shows, then exit
```

The verb-less form needs a real terminal on both stdin and stdout. Redirect
either one and it refuses rather than emitting escape sequences into a file:

```console
$ quinjet > out.txt
error: Quinjet requires an interactive terminal
```

Verbs have no such requirement. They are the form to use over SSH without a
pty, in a `Makefile`, in CI, or anywhere a pipe is on the other end.

## The shape of an invocation

```bash
quinjet [--json] [-C <DIR>] <verb> [<subverb>] [arguments] [flags]
```

Two flags are global, declared once on the root and inherited by every verb, so
they can be typed before or after it. These are the same invocation:

```bash
quinjet -C ~/code/project --json status
quinjet status --json -C ~/code/project
```

`-C`, spelled `--path` in full, is the repository a verb reads and defaults to
`.`. Quinjet asks Git for the worktree root, so pointing it at a subdirectory is
the same as pointing it at the top: `quinjet -C src/cli status` reports on the
whole repository.

The bare positional path is a different argument that belongs to the terminal
interface. Do not reach for it when you are running a verb: with a verb present
it is parsed and then unused, and the verb still reads `-C`. So
`quinjet ~/code/project status` reports on the directory you are standing in,
quietly and with exit 0. Spell it `quinjet -C ~/code/project status`.

A verb always wins over a directory of the same name, so `quinjet status` in a
repository containing a directory called `status` runs the status verb; write
`quinjet ./status` for the directory. The inverse is worth knowing too: a word
that is not a verb is read as a repository path for the terminal interface, so a
typo like `quinjet status` does not produce a usage error, it tries to open a
directory that does not exist.

## At a glance

The first commands worth running, in order:

| Command | What it tells you |
| --- | --- |
| `quinjet --version` | That it installed, and which release you have. |
| `quinjet status` | The branch, its upstream, and every change in the index and the working tree. |
| `quinjet log -n 5` | The recent history of the branch you are on. |
| `quinjet diff` | The patch for what you have changed, one file at a time. |
| `quinjet branch list` | Local branches, their tips and their upstreams. |
| `quinjet repos` | Which GitHub repository this checkout points at, and how it worked that out. |
| `quinjet pr checks <n>` | Whether a pull request is green, without opening a browser. |
| `quinjet --help` | Everything else. |

## The first commands worth running

Start by proving the binary is there:

```console
$ quinjet --version
quinjet 0.0.6
```

Then stand in a repository and ask for its state. The first line is the branch,
a `Tracking` line follows only when the branch has an upstream, and the changes
are grouped `Merge Changes`, `Staged Changes`, `Changes`, each with a count:

```console
$ quinjet status
On branch feat/cli-command-surface

Changes (6)
  M   .github/labeler.yml
  U   .github/workflows/wiki.yml
  M   README.md
  U   docs/cli/README.md
  U   docs/cli/conventions.md
  U   scripts/sync_wiki.py
```

The codes are Quinjet's own single letters, not Git's two-column porcelain:
`A` added, `M` modified, `D` deleted, `R` renamed, `C` copied, `T` type changed,
`U` untracked, `!` conflicted. A clean tree prints `Working tree clean` instead
of any group.

History next. `log` defaults to `HEAD` and to 30 commits, so ask for fewer:

```console
$ quinjet log -n 5
e2d95c2  6 minutes ago  Pulkit            test: pin the command line's contract  (HEAD -> feat/cli-command-surface)
629a805  8 minutes ago  Pulkit            feat: give every operation a subcommand
fe6a382  15 minutes ago  Pulkit            feat: name every operation once, in one command layer
32a089f  20 minutes ago  Pulkit            feat: make every value the app renders serializable
6ce4acd  5 hours ago  github-actions[…  chore: release v0.0.6  (tag: v0.0.6, origin/main, main)
```

Author names are truncated to sixteen characters with a `…`, and the
parenthesized tail is Git's own ref decoration.

Then read the patch. `quinjet diff` prints every changed file, with a header
line per file carrying its status and line counts, and three lines of context
around each hunk. Positional paths narrow it, and `--expanded` prints whole
files:

```console
$ quinjet diff .github/labeler.yml
.github/labeler.yml  · modified +3 -0
@@ -13,3 +13,6 @@ ui:
 git:
   - changed-files:
       - any-glob-to-any-file: ['src/git/**', 'src/watch.rs']
+cli:
+  - changed-files:
+      - any-glob-to-any-file: ['src/cli/**', 'src/main.rs', 'docs/cli/**']
```

Branches, with the current one marked and the upstream after an arrow:

```console
$ quinjet branch list
  chore/ci-hardening           f83fcd6    5 minutes ago  -> origin/chore/ci-hardening
* feat/cli-command-surface     e2d95c2    6 minutes ago
  main                         6ce4acd    5 hours ago  -> origin/main
```

Before touching a pull request, check that Quinjet can see the repository the
number would belong to. `quinjet repos` reads your remotes, strips any embedded
credentials from the URLs, and says which remote each answer came from:

```console
$ quinjet repos
pulkitxm/quinjet                         remote origin https://github.com/pulkitxm/quinjet
```

Then read a pull request by number. Nothing here is listed or prefetched: you
name the number, and that is the only one Quinjet asks GitHub about.

```console
$ quinjet pr view 8
#8  Read pull requests, watch their checks, and index diffs up front
MERGED · @pulkitxm · opened 2026-08-14T19:51:15Z · updated 2026-08-15T13:19:57Z
Source       feat/pr-conversation-live-checks
Destination  pulkitxm/quinjet:main
Changes      14 files, +6284 -602
URL          https://github.com/pulkitxm/quinjet/pull/8

Adds a pull-request pane holding the description, conversation and check logs, keeps it live, and resolves every changed file's line counts while the index is built.
```

```console
$ quinjet pr checks 8
+  passed    Format, lint, and test (macos-latest)        CI  49s
+  passed    Format, lint, and test (ubuntu-latest)       CI  40s
+  passed    Format, lint, and test (windows-latest)      CI  2m 0s
+  passed    Minimum supported Rust                       CI  21s
+  passed    Package validation                           CI  39s
+  passed    label                                        Label PRs  5s
+  passed    lychee                                       Link check  7s

7 passed, 0 pending, 0 failed
```

The glyphs are ASCII on purpose: `+` passed, `x` failed, `o` pending, `-`
skipped, `/` cancelled, `?` unknown. From there, `quinjet pr logs 8 "Minimum
supported Rust"` prints that run's steps with each step's output attached to it,
and a name that matches one check without being equal to it is enough, so
`quinjet pr logs 8 msrv` would work if only one check contained that text.

## Discovering the rest with `--help`

The tree is three levels deep at most, and `--help` works at every level, prints
on stdout and exits 0.

```bash
quinjet --help                 the root: every group and verb, plus the global flags
quinjet pr --help              one group: its verbs
quinjet pr checks --help       one verb: its arguments, flags and defaults
```

The root help is the map:

```console
$ quinjet --help
A fast, live, keyboard-first Git source-control interface for the terminal

Usage: quinjet [OPTIONS] [PATH]
       quinjet [OPTIONS] [PATH] <COMMAND>

Commands:
  tui          Open the terminal interface
  status       Show the working tree, the index and the branch
  diff         Print the working-tree diff
  stage        Stage paths, or everything
  unstage      Unstage paths, or everything
  discard      Throw away changes to paths
  commit       Record the staged changes
  fetch        Fetch every remote and prune deleted refs
  pull         Pull the current branch
  push         Push the current branch
  sync         Pull, then push
  log          List commits
  show         Show one commit and its patch
  branch       Work with branches
  stash        Work with stashes
  cherry-pick  Apply a commit onto the current branch
  revert       Record a commit that undoes another
  resolve      Take one side of a merge conflict
  repos        List the GitHub repositories this checkout points at
  pr           Read a pull request, its files, its conversation and its checks
  help         Print this message or the help of the given subcommand(s)
```

A verb's help is the authority on its defaults, and every default in it is real:

```console
$ quinjet pr checks --help
List a pull request's checks

Usage: quinjet pr checks [OPTIONS] <NUMBER>

Arguments:
  <NUMBER>  Pull-request number

Options:
      --repo <OWNER/NAME>   Repository the number belongs to, as owner/name
      --refresh             Ask GitHub again instead of answering from the cache
      --watch               Keep reading until every check has settled
      --interval <SECONDS>  Seconds between reads while watching [default: 5]
      --exit-code           Exit 1 when a check has not passed
  -C, --path <DIR>          Repository to run a subcommand against [default: .]
      --json                Print one JSON document on stdout instead of text
  -h, --help                Print help
```

`quinjet help pr` and `quinjet help pr checks` are the same thing spelled the
other way. There is no `-j` and no `--no-` inversions: boolean flags are
presence only.

## What the pull-request verbs need

Everything under `quinjet pr`, and `quinjet repos` for a host it cannot
recognize locally, shells out to the GitHub CLI. Install it from
[cli.github.com](https://cli.github.com/) and authenticate once:

```bash
gh auth login
gh auth status
```

Without it, the first `pr` verb you run says so and exits 1:

```console
$ quinjet pr view 8
error: failed to execute GitHub CLI (`gh`) in /home/you/code/project; install it and run `gh auth login`: No such file or directory (os error 2)
```

Quinjet runs `gh` with the repository as its working directory and with
`GH_PROMPT_DISABLED=1`, `GH_PAGER=cat`, `GH_NO_UPDATE_NOTIFIER=1` and
`NO_COLOR=1`, so it never blocks on a prompt, never opens a pager, and never
colors what it hands back. Because it is your `gh`, your `GH_TOKEN`, `GH_HOST`
and `GH_REPO` apply, and a GitHub Enterprise host configured in `gh` works with
no extra setting here. `gh` is the only thing that ever sees a credential:
Quinjet does not read your token, and strips credentials embedded in a remote
URL before that URL becomes a `gh` argument.

Reading a pull request never writes to your repository. Nothing is checked out,
no ref is created, and your index and worktree are untouched. See
[reading a pull request never writes to your repository](./conventions.md#reading-a-pull-request-never-writes-to-your-repository)
for how the patches are produced.

## Where Quinjet keeps its cache

Quinjet keeps GitHub answers and pull-request patches on disk so that a second
read is free. The root is the first of these that is set:

| Order | Root |
| --- | --- |
| 1 | `$QUINJET_CACHE_DIR` |
| 2 | `%LOCALAPPDATA%\quinjet\cache` (Windows only) |
| 3 | `$XDG_CACHE_HOME/quinjet` |
| 4 | `~/Library/Caches/quinjet` (macOS only) |
| 5 | `~/.cache/quinjet` |

Under that root, `github/` holds the cache entries and `tmp/` holds the
disposable bare repositories a pull-request diff sometimes needs. Directories
and files are created with owner-only permissions on Unix, because patches are
repository content sitting outside the repository. The whole thing is bounded to
128 MiB and 2,048 entries and pruned oldest first, and it is shared with the
terminal interface, so a pull request you opened on screen is already warm on
the command line.

Set `QUINJET_CACHE_DIR` when you want the cache somewhere specific, for example
inside a CI job's own scratch directory so it is discarded with the job:

```text
QUINJET_CACHE_DIR="$RUNNER_TEMP/quinjet-cache" quinjet pr checks 8 --json
```

An empty value is ignored rather than treated as a path. If no root can be
worked out at all, because none of those variables is set, Quinjet simply does
not cache and every read goes to GitHub. Credentials are never written there.
What lives how long is in
[what is cached](./conventions.md#what-is-cached), and `--refresh` on any `pr`
verb or on `repos` skips the timed entries.

## Using it from a script

Two things make Quinjet scriptable, and both are contracts rather than
conveniences.

`--json` prints one document on stdout and nothing else. It is global, so it
attaches to any verb, and the keys are camelCase with lower-case hyphenated enum
values. Optional values are emitted as `null` rather than dropped, so the key
set does not change between runs:

```console
$ quinjet status --json
{
  "branch": {
    "head": "feat/cli-command-surface",
    "oid": "e2d95c224418b5568e27d705e9539daf191519b8",
    "upstream": null,
    "ahead": 0,
    "behind": 0,
    "detached": false
  },
  "changes": [
    {
      "path": ".github/labeler.yml",
      "originalPath": null,
      "area": "unstaged",
      "status": "modified"
    }
  ]
}
```

Exit codes are the other half. Errors, hints and warnings go to stderr, never
stdout, and a verb that fails writes nothing on stdout, so
`quinjet pr view 8 --json > out.json` either writes a whole document or writes
nothing at all. The exceptions are `quinjet pr checks --exit-code` and
`quinjet pr checks --watch`, which print their whole document and then exit 1
because a check failed or is still pending.

The pattern for CI is `--watch`, which blocks until every check has settled and
then exits with the verdict:

```bash
quinjet pr checks 8 --watch || echo "not green"
```

`--exit-code` is the same verdict without the waiting: it reads once and exits 1
if anything failed or is still pending. Under `--json --watch` each read is one
compact line instead of one pretty document, so it pipes into `jq` a reading at
a time. `quinjet status --watch` has nothing to settle, so it repeats until you
stop it.

Two small things worth knowing before you parse anything:

- A list verb with nothing to list prints nothing on the human path and exits 0.
  `quinjet stash list` in a repository with no stashes writes zero bytes. Under
  `--json` the same call prints `[]`, which is easier to test.
- A missing `--yes` is not an error. `quinjet discard README.md` with no `--yes`
  reports what it would discard, changes nothing, and exits 0. A script that
  means it must pass `--yes`, and a script that checks exit codes must not read
  0 as "it happened".

## Exit codes

The first three are the ones a new install meets. The full table, including
what each one means per verb, is on
[conventions and contracts](./conventions.md#exit-codes).

| Code | What you did |
| --- | --- |
| 0 | It worked. Also a `--yes`-less destructive verb that deliberately changed nothing, and `--help` anywhere. |
| 1 | It could not finish: not a Git repository, `git` or `gh` missing, `gh` unauthenticated, a red or pending run under `--watch`, or `quinjet` with no verb and no terminal. |
| 2 | The command line was wrong in clap's terms: an unknown flag, a missing argument, a value that will not parse. |
| 3 | You named something that does not exist, or that matches more than one thing. The message always lists what you could have named. |
| 4 | The thing exists but cannot be read: a check run that publishes no GitHub Actions log, or one GitHub has not published anything for yet. |

The two you are most likely to see on day one:

```console
$ quinjet status
error: Not a Git repository: fatal: not a git repository (or any of the parent directories): .git
```

```console
$ quinjet status --bogus
error: unexpected argument '--bogus' found

Usage: quinjet status [OPTIONS]

For more information, try '--help'.
```

The first is exit 1, because Quinjet understood the command and could not carry
it out. The second is exit 2, because clap refused the command line before
Quinjet saw it.

## Notes and gotchas

- Every Git child runs with `GIT_OPTIONAL_LOCKS=0`, so `quinjet status` in a
  loop never creates `index.lock` and never fights a `git` command you are
  running in another window. A write still takes Git's own index lock, because
  it is changing the index. `GIT_TERMINAL_PROMPT=0` means a fetch that needs
  credentials fails instead of blocking on a prompt nobody can answer, and
  `LC_ALL=C` with `-c core.quotepath=false` keeps parsing locale-independent and
  paths raw.
- An unborn branch, a repository where you have run `git init` and not yet
  committed, reads as `"oid": null` with the branch name still present. On a
  detached HEAD the human output says `HEAD detached at <8 hex characters>`,
  `"detached"` is `true`, and `"head"` is that short id rather than a name.
- `quinjet --version` prints the crate version and exists on the root only.
  `quinjet status --version` is a usage error.
- There is no progress bar and no spinner on the command line. The terminal
  interface has one because it can repaint; a pipe cannot, and a half-written
  line in a log is worse than silence.
- The command line has no color to disable. Diffs print with `+`, `-` and a
  leading space in the manner of `git diff`, and check states are the ASCII
  glyphs above, so output is safe to store and to compare.
- Windows is supported and the installers publish an `x86_64` build for it. The
  shell installer only handles Windows when run under MSYS, MinGW or Cygwin; use
  `install.ps1` from PowerShell otherwise.
- Nothing Quinjet reads is unbounded. A read that crosses a cap kills its child
  process and says so in the output rather than looking complete. The caps are
  listed in [size caps](./conventions.md#size-caps).

## Where to go next

- [Conventions and contracts](./conventions.md) for the `--json` guarantee, the
  full exit-code table, the caching rules and the size caps
- [The terminal interface](./tui.md) for the verb-less form and the map from
  each key on screen to the verb behind it
- [`quinjet status`, `diff`, `log`, `show`](./repository/README.md) for reading
  a repository in depth
- [`quinjet pr`](./pull-request/README.md) for everything that needs `gh`,
  including `--watch`
- [All `quinjet` commands](./README.md)
