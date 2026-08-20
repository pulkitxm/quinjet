# Plumbing and Porcelain: Machine Interfaces to Git

Quinjet never links a Git library. Every repository fact on screen came out of a spawned `git`
process, and every one of those processes was asked a question in one of Git's machine dialects:
porcelain v2 status records, NUL-terminated diff listings, custom log formats delimited with ASCII
control bytes, and single-purpose plumbing probes. This page explains those dialects from the bytes
up, then walks the exact Quinjet code that speaks them: the fixed argv-and-environment substrate,
the byte-oriented parsers in `src/git/status.rs` and `src/git/history.rs`, the one-flag-swap trick
that keeps a listing and its totals describing the same diff, and the capped pipe reader that kills
a child the moment its output crosses a byte budget.

## Contents

- [Two layers of one tool](#two-layers-of-one-tool)
- [Bytes on the wire: quoting, locales, NUL](#bytes-on-the-wire-quoting-locales-nul)
- [Spawning Git safely](#spawning-git-safely)
- [Porcelain v2 status, field by field](#porcelain-v2-status-field-by-field)
- [The machine diff family](#the-machine-diff-family)
- [Delimited logs and ref listings](#delimited-logs-and-ref-listings)
- [Plumbing probes](#plumbing-probes)
- [Capped pipes and kill-on-cap](#capped-pipes-and-kill-on-cap)
- [The invocation catalog](#the-invocation-catalog)
- [Where plumbing meets scheduling](#where-plumbing-meets-scheduling)
- [Design alternatives and why they lost](#design-alternatives-and-why-they-lost)
- [Where to go next](#where-to-go-next)

## Two layers of one tool

Git ships as one binary but presents two very different surfaces. The
[git manual page](https://git-scm.com/docs/git) divides its subcommands into *porcelain*, the
high-level commands meant for human fingers and eyes, and *plumbing*, the low-level commands meant
to be composed by scripts and other programs. The metaphor is bathroom hardware: porcelain is the
polished fixture a person touches, plumbing is the piping underneath that actually moves things.

The split is not cosmetic. It is a compatibility contract, and everything on this page follows from
taking that contract seriously.

### What plumbing promises

Plumbing commands such as `git rev-parse`, `git cat-file`, `git merge-base`, `git diff-tree`,
`git for-each-ref`, `git hash-object`, and `git update-index` promise three things:

**1. Stable output.** The format a plumbing command prints is part of its interface. New Git
releases may add new options, but the existing output shape does not change underneath a caller. A
script written against `git cat-file -t` in 2010 still parses today.

**2. Configuration independence.** Plumbing output does not react to a user's cosmetic
configuration. `color.ui`, custom pagers, aliases, and display preferences are porcelain concerns;
plumbing prints the same bytes for every user.

**3. Composability.** Plumbing commands do one narrow thing and communicate through exit codes and
parseable stdout, so they can be chained. `git rev-parse --verify` answers "does this name resolve"
with its exit status alone; `git cat-file -e` answers "does this object exist" the same way,
printing nothing at all.

### What porcelain does not promise

Porcelain commands such as `git status`, `git log`, `git branch`, `git stash list`, and `git diff`
in their default modes are explicitly allowed to change their human-facing output between releases.
Three properties make their default output hostile to parsers:

**1. Localization.** Porcelain messages pass through gettext. `git status` run under a German
locale prints `Auf Branch main` instead of `On branch main`. Any parser matching English literals
breaks the moment `LANG` changes.

**2. Configuration sensitivity.** Column layouts, colors, relative date styles, rename thresholds,
and pager behavior all shift with user configuration. The same command prints different bytes in
different home directories.

**3. Evolving wording.** Git's maintainers rewrite porcelain hints and summaries freely. The
`git status` advice text has been reworded many times; nothing prevents the next release from doing
it again.

The consequence: a program may *invoke* porcelain commands, but it must never parse their default
output. It must either use a plumbing command instead or ask the porcelain command for one of its
stable machine formats.

### Porcelain output formats

The naming gets confusing here, and it is worth untangling because Quinjet's most important parser
consumes a format literally called "porcelain". Several porcelain commands grew dedicated
machine-readable output modes, and Git named the flag after the *audience*: `--porcelain` means
"output suitable for building a porcelain on top of", that is, for a tool like Quinjet that presents
its own user interface over Git.

The stable machine formats that matter on this page:

| Command | Machine format | Stability promise |
|---|---|---|
| `git status` | `--porcelain=v1`, `--porcelain=v2` | frozen per version, config-independent |
| `git status`, `git diff` | `-z` | NUL termination, no path quoting |
| `git diff` | `--name-status`, `--numstat`, `--raw` | stable listing shapes |
| `git log` | `--format=<custom>` | caller defines the exact bytes |
| `git for-each-ref` | `--format=<atoms>` | caller defines the exact bytes |
| `git worktree list` | `--porcelain [-z]` | attribute-per-line stanzas |
| `git blame`, `git push` | `--porcelain` | stable line-oriented records |

Two distinct strategies appear in that table. The first is *opt-in frozen formats*: `git status
--porcelain=v2` prints a documented record grammar that Git promises not to break. The second is
*caller-defined formats*: `git log --format` and `git for-each-ref --format` let the caller write a
template, so the output is exactly as stable as the placeholders the caller chose. Quinjet uses
both, and the second strategy is where its 0x1f/0x1e delimiter trick lives.

Version 2 of the status porcelain format arrived in Git 2.11 and is the one Quinjet reads: unlike
v1 it carries branch headers, ahead/behind counts, file modes, and object names in one pass, which
is what lets one subprocess answer everything the status pane needs.

### Quinjet's position: subprocess plumbing, never a library

Quinjet could have linked `libgit2` or `gitoxide` and read the object database in-process. It
deliberately does not. `ARCHITECTURE.md` states the goal as "immediate input response,
authoritative Git behavior, and predictable memory use", and *authoritative* is the operative word:
the `git` binary on the user's `PATH` is the single source of truth for repository semantics.
Hooks fire, `.gitattributes` and `.gitignore` apply, credential helpers run, config cascades
resolve, and index format extensions behave exactly as they would for the user's own commands,
because they are the user's own Git executing them. The trade-offs of this choice, and the
alternatives it beat, are covered in
[Design alternatives and why they lost](#design-alternatives-and-why-they-lost).

The cost of the subprocess model is that every answer arrives as bytes on a pipe, so the quality of
the whole system rests on which dialects those bytes are requested in. Quinjet's rule, visible in
every invocation cataloged on this page, is:

- Ask porcelain commands only for their frozen machine formats, never their default output.
- Prefer plumbing when a plumbing command answers the question (`rev-parse`, `cat-file -e`,
  `for-each-ref`, `merge-base`, `diff-tree`, `check-ref-format`).
- Define custom formats with delimiters that cannot occur in the data.
- Force the environment so localization, prompts, and optional locking cannot vary the contract.
- Bound every read in bytes and kill the child when the bound is crossed.

The rest of this page takes those rules one at a time.

## Bytes on the wire: quoting, locales, NUL

Before any record grammar matters, there is a lower-level question: what bytes can a path or a
commit subject contain, and how does Git protect its own output framing from them? Getting this
wrong is the classic way Git-parsing tools corrupt themselves, so Quinjet settles it globally, in
the process-spawning substrate, before any individual command is even chosen.

### Default path quoting

A POSIX file name is an arbitrary byte string; the only bytes that can never appear in a path are
NUL (the C string terminator) and, within a single component, the `/` separator. Newlines, tabs,
spaces, quotes, backslashes, and invalid UTF-8 are all legal. That single fact breaks the naive
model of "one path per line": a file named `a\nb` produces two lines, and a parser splitting on
newline silently invents a file named `a` and a file named `b`.

Git's default defense is C-style quoting, controlled by `core.quotePath`
(see [git-config](https://git-scm.com/docs/git-config)). When a listed path contains a byte above
0x7f or a control character, Git wraps the path in double quotes and escapes the offending bytes
octally. A file named `héllo.txt` stored as UTF-8 renders like this in default `git status` or
`git diff --name-status` output:

```text
"h\303\251llo.txt"
```

A parser now needs a full unquoting layer: recognize the surrounding quotes, decode `\303` style
octal escapes, handle `\"`, `\\`, `\t`, `\n` shorthands, and cope with the fact that quoting only
happens *sometimes*, depending on the bytes in the path and on configuration. Every tool that
parses quoted Git output carries that decoder and its bugs.

There are two ways to make the decoder unnecessary, and Quinjet uses both at once:

**1. `-z` termination.** In `-z` mode Git terminates records with NUL instead of newline and,
because NUL is the one byte a path cannot contain, prints paths verbatim with no quoting at all.
The framing byte and the data are guaranteed disjoint.

**2. `core.quotepath=false`.** The `-z` flag only exists on listing commands. Patch text has no
`-z` mode, and a unified diff embeds paths in its `diff --git a/... b/...` header lines, where Git
would still C-quote unusual paths. Setting `core.quotepath=false` per invocation makes Git emit
raw path bytes there too, so the patch parser in `src/git/diff.rs` reads header paths byte-exact
without an unquoting step.

### The NUL termination contract

`-z` is not one flag but a family of per-command contracts, and the details differ in ways a parser
must know:

| Command | What `-z` changes |
|---|---|
| `git status --porcelain=v2 -z` | records end with NUL; a rename's original path becomes its own following NUL-terminated field |
| `git diff --name-status -z` | the status field and each path are separate NUL-terminated records; renames emit three records |
| `git diff --numstat -z` | records end with NUL; a rename emits an empty path field then pre-image and post-image records |
| `git worktree list --porcelain -z` | attribute lines end with NUL; stanzas are separated by an empty record (double NUL) |

The common thread: `-z` does not merely swap the terminator byte. It also restructures multi-path
records so that each path is a whole field, and it disables quoting. A parser therefore splits the
entire output on the single byte 0x00 and treats the result as a flat record stream, consuming a
variable number of records per logical entry. That is exactly the shape of every listing parser in
Quinjet, as the walkthroughs below show.

### Locale-proof output

Machine formats are unlocalized by design, but two localization hazards remain. First, any error
path prints translated text: `fatal: not a git repository` becomes something else under another
locale, and Quinjet surfaces Git stderr verbatim in its own error messages, so a stable locale
keeps those messages consistent and greppable. Second, a few outputs that Quinjet consumes are
formatted-by-convention rather than formally frozen, and locale is one less axis of variation to
audit.

Setting `LC_ALL=C` on every child forces the C locale for all categories: messages are
untranslated, and no locale-dependent formatting applies anywhere in the output. It is one
environment variable, and it converts "should be stable under any locale" into "is stable, there is
no locale".

### How Quinjet combines the three

Every Git child Quinjet spawns gets the same treatment, applied in `Repository::run`
(`src/git/mod.rs:1292`) and mirrored in `Repository::checked_bounded` (`src/git/mod.rs:1258`) and
in the PR workspace runner `run_repository_git` (`src/git/github/mod.rs:2192`):

- `-c core.quotepath=false` so paths are raw bytes everywhere, including patch headers.
- `-z` on every listing command that supports it, so framing is NUL and quoting never triggers.
- `LC_ALL=C` so no output or error text is translated.

The final byte-to-string step is uniform too. Every parser converts field bytes with
`String::from_utf8_lossy`, wrapped in tiny helpers named `text` (`src/git/mod.rs:1576` and
`src/git/history.rs:97`) and `bytes_to_path` (`src/git/status.rs:284`). From `src/git/status.rs`:

```rust
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
```

Lossy conversion is a deliberate policy: a path with invalid UTF-8 degrades to replacement
characters in the UI instead of failing the whole status read. Field *boundaries* are still exact,
because splitting happened on NUL before any string conversion; only the display of a
non-UTF-8 name degrades. `ARCHITECTURE.md` lists full non-UTF-8 path preservation as a deliberate
next step rather than a current guarantee, and this is the seam where it would land.

There is a subtle correctness consequence worth spelling out. Because unquoting logic does not
exist anywhere in the codebase, there is no code path in which it can be applied twice, applied to
an already-raw path, or skipped. A whole class of quoting bugs is structurally absent rather than
carefully avoided. The same pattern repeats across this page: Quinjet prefers formats that make an
error inexpressible over code that handles the error.

## Spawning Git safely

Machine formats are only half of the interface. The other half is the call itself: how arguments
reach the child, what environment it runs under, and what its exit status means. Quinjet
concentrates all of it in three methods on `Repository`, so every one of the dozens of Git
invocations in the catalog below inherits the same guarantees.

### No shell, ever

There are two ways to start a subprocess. The first hands a command *string* to a shell
(`sh -c "git diff $path"`), which then performs word splitting, glob expansion, variable
substitution, and metacharacter interpretation before `git` ever runs. The second passes an argv
*vector* directly to the operating system's exec machinery, where each element arrives in the
child's `argv` byte-for-byte.

The string form is how command injection happens: a file named `$(rm -rf ~)` or `; curl evil |
sh` becomes shell code the moment it is interpolated into the command string. Quoting can defend
it in principle, but the defense has to be perfect at every call site, for every shell, forever.

Quinjet only uses the vector form. `std::process::Command::new("git")` followed by `.arg()` and
`.args()` calls performs no interpretation of any kind; a path containing spaces, quotes,
newlines, or shell metacharacters is one argv element and nothing more. ARCHITECTURE.md invariant
7 pins it: "Git and GitHub CLI receive argv directly, never via a shell." The base runner, from
`src/git/mod.rs:1292`:

```rust
fn run<I, S>(&self, args: I) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("git");
    let _ = command
        .arg("-C")
        .arg(&self.root)
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0");
    command
        .output()
        .with_context(|| format!("failed to execute Git in {}", self.root.display()))
}
```

Note the generic bound: arguments are `AsRef<OsStr>`, not `&str`. Paths flow from `PathBuf` into
argv as platform-native OS strings without a UTF-8 round trip, so even a path that cannot be
represented as a Rust `String` still reaches Git intact as an argument.

### The fixed environment, variable by variable

Five settings ride on every invocation, and each one closes a specific failure mode.

**1. `-C <root>`.** The working directory is an argument to Git, not an ambient property of the
process. Quinjet's worker threads share one process; changing the process-wide current directory
from one thread would race every other thread's relative-path operations. `-C` makes the target
repository explicit per invocation, so six worker lanes can run Git against the same root, or a
disposable PR workspace, concurrently and without coordination. `Repository` itself
(`src/git/mod.rs:242`) is nothing but `{ root: PathBuf }`; the handle *is* the `-C` argument.

**2. `-c core.quotepath=false`.** Covered above: raw path bytes in every output, including patch
headers where `-z` cannot reach. Passed as a per-invocation `-c` override rather than written to
any config file, so the user's repository configuration is never touched.

**3. `LC_ALL=C`.** Covered above: no translated output, ever. `LC_ALL` outranks both `LANG` and
the per-category `LC_*` variables, so it wins regardless of what the inherited environment says.

**4. `GIT_OPTIONAL_LOCKS=0`.** The subtlest of the five. Several nominally read-only Git commands
opportunistically write. The flagship case is `git status`: while computing its answer it refreshes
stat information in the index, and by default it takes `index.lock` and writes the refreshed index
back, purely as an optimization for later commands. That optional write has two consequences a
background poller cannot accept. If the user's own `git commit` holds `index.lock` at that moment,
the status poll fails or stalls on the lock; conversely the poll's own lock can make the user's
command fail with "index.lock already exists". Setting `GIT_OPTIONAL_LOCKS=0` (equivalent to the
`--no-optional-locks` global flag, see the [git manual page](https://git-scm.com/docs/git)) tells
Git to skip any action that merely requires an optional lock: status still answers, but it no
longer writes the index at all. Quinjet polls status on a 10 second tick and after every watcher
event, so this variable is the difference between an invisible background read and a background
process fighting the user for their own repository. ARCHITECTURE.md invariant 13 records it: "Read
operations set `GIT_OPTIONAL_LOCKS=0`". A pleasant side effect: because the poll never writes
`.git/index`, it never retriggers the filesystem watcher that scheduled it, which would otherwise
produce a self-sustaining refresh loop. The lock file itself and the index format it protects are
covered in [refs, index, and worktrees](./refs-index-and-worktrees.md).

**5. `GIT_TERMINAL_PROMPT=0`.** Git shells out for credentials when a remote needs them and no
helper answers. In a terminal application that has taken over the screen with an alternate-screen
TUI, a child process writing `Username for 'https://github.com':` to the terminal and blocking on
a read would wedge a worker thread forever and corrupt the display. With the variable set, Git
fails the operation immediately instead of prompting, the error surfaces through the normal error
path, and the worker thread moves on. The `gh` substrate applies the same policy through its own
switch, `GH_PROMPT_DISABLED=1`.

The identical five-part setup appears twice in the codebase: once in `Repository::run` /
`Repository::checked_bounded` for the opened repository, and once in `run_repository_git`
(`src/git/github/mod.rs:2192`) for Git run inside the disposable PR workspace. Both construct the
command the same way, so there is no invocation path that forgets a variable.

### Option injection and the double dash

Argv-direct spawning kills shell injection, but one injection class survives it: *option
injection*. If user-influenced text becomes an argv element and that text starts with `-`, Git
parses it as a flag. A branch named `--output=/tmp/owned` or a file named `-n` does not need a
shell to cause damage; it only needs to be placed where Git expects an option.

Git's own defense is the `--` separator: everything after it is a path, never an option, and most
commands accept it. Quinjet applies it mechanically:

- `with_paths` (`src/git/mod.rs:1248`) is the single helper through which every path-taking
  mutation flows, and it unconditionally inserts `--` before the paths.
- Every diff, show, and log argv in the codebase ends with `--` before any paths, including
  `history()` where no paths follow at all: the trailing `--` there disambiguates the revision
  from a path of the same name, so a file named `HEAD` cannot confuse the parse.
- Refs passed to mutations sit after `--` where the command supports it: `switch -- <branch>`,
  `branch --move -- <old> <new>`, `branch --delete -- <branch>`, `stash push ... -- <paths>`.

### Validation before anything reaches an argv

Positional separators do not cover everything; revisions genuinely must appear in option position
sometimes. For those, Quinjet validates shape before Git ever sees the value:

**1. `resolve_revision` (`src/git/mod.rs:299`).** The gate for user-typed revisions from the CLI.
From the source:

```rust
pub(crate) fn resolve_revision(&self, revision: &str) -> Result<String> {
    let revision = revision.trim();
    if revision.is_empty() || revision.starts_with('-') {
        bail!("refusing to resolve `{revision}` as a revision");
    }
    if revision == "HEAD" {
        return Ok(revision.to_owned());
    }
    if let Some(reference) =
        self.rev_parse(["--symbolic-full-name", "--verify", "--quiet", revision])
        && (reference.starts_with("refs/heads/")
            || reference.starts_with("refs/remotes/")
            || reference.starts_with("refs/tags/"))
    {
        return Ok(reference);
    }
    self.rev_parse(["--verify", "--quiet", &format!("{revision}^{{commit}}")])
        .ok_or_else(|| anyhow!("`{revision}` does not name a commit in this repository"))
}
```

Anything starting with `-` is refused outright, before Git could mistake it for a flag. Everything
else is normalized by Git itself into either a full ref under `refs/heads/`, `refs/remotes/`, or
`refs/tags/`, or a commit object id. The value that continues into later argvs is therefore always
one of a handful of known-safe shapes, never the raw user string.

**2. `history` re-validates (`src/git/mod.rs:330`).** Even though callers resolve first, the
history reader independently rejects anything that is not `HEAD`, a full ref in the three safe
namespaces, or a full hex object id (`is_full_oid`, `src/git/mod.rs:1588`: length 40 or 64, all
ASCII hex, covering both SHA-1 and SHA-256 repositories). A unit test pins that
`history("--all", ...)` fails. Defense in depth: a future caller that forgets to resolve cannot
smuggle a flag into `git log`.

**3. Narrow shape checks.** `validate_history_reference` (`src/git/mod.rs:1388`) requires the
`refs/heads/` or `refs/remotes/` prefix before branch comparison diffs. `valid_stash_reference`
(`src/git/mod.rs:1396`) accepts exactly `stash@{<digits>}` and nothing else, both when parsing
`stash list` output and before every stash operation, so a crafted reflog selector cannot ride
back into an argv. `has_commit` (`src/git/mod.rs:790`) only accepts full object ids before probing
`cat-file -e`.

**4. Delegated validation.** Branch names have famously intricate rules (no `..`, no leading
`-`, no `@{`, no trailing `.lock`, and a dozen more). Quinjet does not reimplement them:
`validate_branch_name` (`src/git/mod.rs:1235`) trims, rejects empty, and then asks
[git check-ref-format](https://git-scm.com/docs/git-check-ref-format) `--branch <name>` to make
the authoritative call.

**5. Filesystem containment.** For untracked files Quinjet touches the filesystem directly (there
is nothing in the object database to diff against), so `safe_worktree_path`
(`src/git/mod.rs:1528`) rejects absolute paths and any path containing parent-directory, root, or
drive-prefix components before joining onto the repository root. A status record can never direct
a read or a discard outside the repository.

### Exit codes and error surfaces

Git's exit status conventions are loose but usable: 0 is success; most fatal errors exit 128;
usage errors exit 129; a handful of commands encode a domain answer in the code (`git diff
--exit-code` exits 1 when differences exist, `git merge-base` exits 1 when no common ancestor
exists, `git cat-file -e` exits non-zero when the object is missing). Quinjet's policy has three
tiers, one per runner method:

**1. `run` (`src/git/mod.rs:1292`)** returns the raw `Output` and leaves interpretation to the
caller. The probes that encode answers in exit codes use it: `has_head` checks
`status.success()` on `rev-parse --verify HEAD`, `has_commit` checks `cat-file -e`, and
`try_merge_base` in the PR module treats a non-zero `merge-base` as "deepen the shallow history
and try again" rather than as an error.

**2. `checked` (`src/git/mod.rs:1280`)** converts any non-zero exit into an error built by
`command_error` (`src/git/mod.rs:1517`), which prefers trimmed stderr, falls back to trimmed
stdout, and falls back to `"Git command failed (exit status ...)"`. Every command whose only
legitimate outcome is success uses it.

**3. `checked_bounded` (`src/git/mod.rs:1258`)** adds the byte cap, and with it a deliberate
wrinkle in the exit-code logic:

```rust
let output = run_bounded_command(&mut command, limit, MAX_GIT_ERROR_BYTES)
    .with_context(|| format!("failed to execute Git in {}", self.root.display()))?;
if !output.status.success() && !output.stdout_truncated {
    bail!("{}", bounded_command_error("Git command failed", &output));
}
Ok((output.stdout, output.stdout_truncated))
```

A non-zero exit is only a failure when stdout was *not* truncated. When the cap is hit, Quinjet
kills the child mid-write (see [Capped pipes and kill-on-cap](#capped-pipes-and-kill-on-cap)), and
a killed process necessarily reports an unsuccessful exit status. Treating that as an error would
turn every capped read into a failure; instead the truncated bytes are the answer, flagged as
truncated so the parser and the UI can say so. Stderr is capped separately at
`MAX_GIT_ERROR_BYTES` (128 KiB), enough for any real diagnostic while bounding a pathological
child that floods its error stream.

The three tiers give every call site exactly the exit-code semantics its command needs, and no
call site interprets a status code ad hoc: raw probes, hard failures, and capped reads are the
only three vocabularies in the module.

## Porcelain v2 status, field by field

`git status --porcelain=v2` is the highest-traffic machine format in Quinjet. It runs on every
watcher event, every 10 second periodic tick, and after every mutation, and its answer drives the
sidebar change list, the branch header, staged counts, and the working-tree diff index. The full
grammar is documented in [git-status](https://git-scm.com/docs/git-status); this section walks it
byte by byte and then through the parser in `src/git/status.rs`.

### Why version 2 exists

The v1 porcelain format is the old two-column short format frozen in time: two status letters, a
space, a path, with renames as `old -> new` on one line. It is stable, but it answers only half of
what a status-driven UI needs. It has no branch information at all (a separate `git symbolic-ref`
or `--branch` header bolt-on is needed), no ahead/behind counts, no file modes, no object names,
and no structured way to distinguish an ordinary change from a rename without parsing an arrow that
could also legally appear inside a file name.

Version 2 redesigns the record grammar instead of freezing an accident. Each line starts with a
record-type tag, each record type has a fixed field layout, and a `--branch` request interleaves
structured header lines. One invocation now answers: what branch is checked out, what commit it is
on, what it tracks, how far ahead or behind it is, and the full three-tree state of every changed
path. That is exactly the shape of Quinjet's `RepoStatus`:

```rust
pub(crate) struct RepoStatus {
    pub branch: BranchState,
    pub changes: Vec<Change>,
}
```

The exact invocation, from `Repository::status` (`src/git/mod.rs:287`):

```rust
pub(crate) fn status(&self) -> Result<RepoStatus> {
    let output = self.checked([
        OsString::from("status"),
        OsString::from("--porcelain=v2"),
        OsString::from("--branch"),
        OsString::from("-z"),
        OsString::from("--untracked-files=all"),
        OsString::from("--ignore-submodules=none"),
    ])?;
    Ok(parse_porcelain_v2(&output))
}
```

Each flag earns its place:

- `--porcelain=v2`: the frozen grammar this whole section describes.
- `--branch`: the `# branch.*` headers, saving a second subprocess for branch state.
- `-z`: NUL framing, no quoting, rename original paths as separate records.
- `--untracked-files=all`: by default Git collapses an entirely-untracked directory into one
  entry with a trailing slash. Quinjet stages, discards, and diffs untracked files individually,
  so it needs each file listed as its own record.
- `--ignore-submodules=none`: overrides any `submodule.<name>.ignore` or `diff.ignoreSubmodules`
  configuration so submodule state changes always appear, keeping the change list complete
  regardless of repository config.

Combined with `GIT_OPTIONAL_LOCKS=0` from the substrate, this status read takes no lock and
writes nothing: it is a pure snapshot that can run on any cadence without side effects.

### Header lines

With `--branch`, the output opens with up to four header records, each starting `# `:

| Header | Value | Notes |
|---|---|---|
| `# branch.oid <oid>` | current commit id | the literal `(initial)` on an unborn branch |
| `# branch.head <name>` | current branch short name | the literal `(detached)` when detached |
| `# branch.upstream <name>` | upstream short name | present only when an upstream is configured |
| `# branch.ab +<n> -<m>` | ahead and behind counts | present only when the upstream is resolvable |

A fifth header, `# stash <n>`, exists behind `--show-stash`; Quinjet does not request it because
stashes have their own richer listing (see
[Stash listings through reflog selectors](#stash-listings-through-reflog-selectors)).

`parse_branch_header` (`src/git/status.rs:172`) consumes these with `strip_prefix` per header
name. Three details are load-bearing:

**1. `(initial)` is absence, not a value.** On an unborn branch there is no commit, so the parser
maps `(initial)` to `oid: None` rather than storing the placeholder string. Downstream code that
keys on the oid (for example deciding the unstage strategy, which differs on an unborn branch)
gets a properly typed absence.

**2. Detached HEAD synthesizes a display name.** When `branch.head` is `(detached)` the parser
sets the `detached` flag and substitutes the first 8 characters of the previously parsed oid as
the display head, falling back to the literal string `detached` when even the oid is missing.
This works because Git prints `branch.oid` before `branch.head`; the parser exploits the
documented header order rather than buffering.

**3. Ahead/behind parse failures degrade to zero.** The `+n`/`-m` parts go through
`parse().unwrap_or_default()`; a malformed count renders as 0 rather than failing the snapshot.
Counts are decoration, presence of changes is the contract.

### Ordinary records

An ordinary changed path is a `1` record with eight fields before the path:

```text
1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>
```

| Field | Meaning |
|---|---|
| `1` | record type: ordinary change |
| `XY` | two status letters: staged state, then unstaged state |
| `sub` | submodule state: `N...` for a plain file, `S<c><m><u>` for a submodule |
| `mH` | octal file mode in HEAD |
| `mI` | octal file mode in the index |
| `mW` | octal file mode in the worktree |
| `hH` | object name in HEAD |
| `hI` | object name in the index |
| `path` | the path, raw bytes in `-z` mode |

The `XY` pair is the semantic core, and its v2 form fixes a genuine ambiguity from v1: unmodified
positions are `.` rather than space, so the two letters are always visibly two letters. `M.` means
staged-modified with a clean worktree copy; `.M` means modified but unstaged; `MM` means staged
changes with further unstaged edits on top. The letters themselves are `M` modified, `T` type
changed (file to symlink and similar), `A` added, `D` deleted, `R` renamed, `C` copied, `U`
updated-but-unmerged.

`MM` is why Quinjet's parser can emit two `Change` entries for one record. The staged half and the
unstaged half are different facts about different trees: one describes index-versus-HEAD, the
other worktree-versus-index, and the UI presents them in different sections with different
available actions (unstage versus stage or discard). `push_xy_changes`
(`src/git/status.rs:236`) makes the split explicit:

```rust
if x != b'.' {
    changes.push(Change {
        path: path.clone(),
        original_path: original_path.clone(),
        area: ChangeArea::Staged,
        status: status_from_code(x),
    });
}
if y != b'.' {
    changes.push(Change {
        path,
        original_path,
        area: ChangeArea::Unstaged,
        status: status_from_code(y),
    });
}
```

The status parser deliberately ignores the mode and object-name fields. Quinjet does not render
modes, and object names for individual index entries are not needed because diffs are always
recomputed by Git itself. Positional splitting still has to account for them, which is exactly
what `splitn_bytes(record, b' ', 9)` does in `parse_ordinary` (`src/git/status.rs:201`): split on
spaces into at most 9 fields, so fields 0 through 7 are the fixed columns and field 8 is the path,
*including any spaces the path contains*. The `splitn` bound is the whole path-with-spaces
defense; there is no escaping to undo because `-z` plus `quotepath=false` already guaranteed raw
bytes.

### Rename and copy records

A rename or copy is a `2` record with one extra fixed field and, in newline mode, a compound path
field:

```text
2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path><sep><origPath>
```

`<X><score>` is the similarity: `R100` is an exact rename, `C75` a 75 percent similar copy. In
newline mode `<sep>` is a tab, which is parseable only because paths containing tabs would have
been quoted. In `-z` mode the design is cleaner: `<sep>` becomes NUL, which means the original
path is simply *the next record in the stream*. A parser must know that a `2` record consumes its
successor:

```rust
b'2' => {
    let (original_path, rest) = remaining
        .split_first()
        .map_or_else(|| (b"".as_slice(), remaining), |(path, rest)| (*path, rest));
    remaining = rest;
    parse_renamed(record, original_path, &mut status.changes);
}
```

That excerpt is the heart of `parse_porcelain_v2` (`src/git/status.rs:123`): the record walk holds
the remaining record slice and lets a record type pull additional records when its grammar says
so. A missing successor (torn output) degrades to an empty original path instead of a panic or a
misparse of the next entry.

`parse_renamed` (`src/git/status.rs:212`) then splits on spaces into at most 10 fields (one more
than ordinary, for the score column) and takes field 9 as the new path. Both resulting `Change`
entries carry `original_path`, so the UI can render "renamed from" and diff commands can pass both
names.

### Unmerged records

Conflicts get their own record type because they have their own tree shape: up to three competing
versions (common ancestor, ours, theirs) instead of the usual two.

```text
u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>
```

The `XY` letters here describe conflict flavors (`UU` both modified, `AA` both added, `DU`
deleted by us, and so on), and the four modes and three object names describe stages 1 through 3
of the index plus the worktree. Quinjet collapses all of that deliberately: `parse_unmerged`
(`src/git/status.rs:223`) splits into at most 11 fields, takes field 10 as the path, and emits a
single `Change` with `area: Conflict, status: Conflicted`. The UI's job is to route the user to
resolution actions (`checkout --ours`, `checkout --theirs`, stage the merged result), not to
render stage-level metadata, so the parser keeps only what the actions need.

The `ChangeArea` enum ordering makes conflicts jump the queue. From `src/git/status.rs`:

```rust
pub(crate) enum ChangeArea {
    Conflict,
    Staged,
    Unstaged,
}
```

The derive order *is* the sort order (`Ord` on a Rust enum follows declaration order), and the
final sort in `parse_porcelain_v2` uses it:

```rust
status.changes.sort_by(|left, right| {
    left.area
        .cmp(&right.area)
        .then_with(|| left.display_path().cmp(&right.display_path()))
});
```

Conflicts first, then staged, then unstaged, lexicographic within each area: a deterministic
presentation order computed once at parse time, so the render path never sorts.

### Untracked and ignored records

The two remaining record types are single-field:

```text
? <path>
! <path>
```

`?` is an untracked file; because Quinjet passes `--untracked-files=all`, each untracked file is
its own record rather than a collapsed directory. The parser reads bytes from offset 2 (skipping
the marker and the space) and emits `area: Unstaged, status: Untracked`. Untracked entries later
take a completely different diff path: there is nothing in the object database to compare against,
so `untracked_patch` (`src/git/mod.rs:1118`) synthesizes a unified diff directly from the
filesystem without invoking Git at all.

`!` records are ignored files and appear only under `--ignored`, which Quinjet never passes. The
parser's final dispatch arm ignores unknown markers entirely, so even if a future Git version
introduced a new record type, the status snapshot would degrade by omission rather than fail. One
notational near-collision is worth flagging: Quinjet's own `ChangeStatus::Conflicted` renders as
the badge `!` in the UI (`ChangeStatus::code`, `src/git/status.rs:38`), which is unrelated to the
porcelain `!` record type.

### A worked example, byte by byte

The parser's unit test at `src/git/status.rs:296` feeds a literal byte string that exercises every
record family at once. Reformatted with one record per line (the `\x00` markers are the real NUL
terminators, shown symbolically):

```text
# branch.oid 0123456789abcdef                                          \x00
# branch.head feature/live                                             \x00
# branch.upstream origin/feature/live                                  \x00
# branch.ab +2 -3                                                      \x00
1 M. N... 100644 100644 100644 aaaaaaa bbbbbbb src/staged.rs           \x00
1 .M N... 100644 100644 100644 aaaaaaa aaaaaaa src/live.rs             \x00
1 MM N... 100644 100644 100644 aaaaaaa bbbbbbb src/both.rs             \x00
? notes with spaces.txt                                                \x00
u UU N... 100644 100644 100644 100644 aaaaaaa bbbbbbb ccccccc conflict.rs \x00
```

Walking it as the parser does:

1. Four headers fill `BranchState`: head `feature/live`, upstream `origin/feature/live`, ahead 2,
   behind 3, oid `0123456789abcdef`.
1. `1 M. ...` has staged `M`, unstaged `.`: one Staged/Modified change for `src/staged.rs`.
1. `1 .M ...` is the mirror image: one Unstaged/Modified change for `src/live.rs`. Note its two
   object names are equal (`aaaaaaa aaaaaaa`): the index still matches HEAD, only the worktree
   moved, which is exactly what `.M` encodes.
1. `1 MM ...` fans out into two changes for `src/both.rs`, one per area.
1. `? notes with spaces.txt` shows why byte framing matters: the path contains two spaces and
   parses intact because untracked records take everything after offset 2, with no splitting.
1. `u UU ...` contributes the single Conflict entry; note it carries one more mode and one more
   object name than an ordinary record, which is why its `splitn` bound is 11.

The test asserts 6 changes from 5 change records, a staged count of 2, and exactly one conflict:
the `MM` fan-out and the area classification in one assertion set. The rename test just below it
(`src/git/status.rs:323`) feeds a `2` record followed by its NUL-separated original path,
`new name.rs` then `old name.rs`, both containing spaces, and asserts both come through verbatim.

### The Quinjet parser, annotated

The complete algorithm of `parse_porcelain_v2` (`src/git/status.rs:123`), now that all the pieces
have appeared:

1. Split the whole output on the byte 0x00 into a record list. No other framing exists; a record
   is whatever lies between NULs.
1. Walk with `split_first` over a shrinking slice, so any record arm can consume the following
   record. Only the `2` arm uses this power, but the loop structure makes such grammars cheap to
   support.
1. Route `# `-prefixed records to the branch-header parser, then dispatch on the first byte:
   `1`, `2`, `u`, `?`, everything else ignored.
1. Extract fields positionally with `splitn_bytes` and a bounded field count per record type, so
   the last field (always the path) absorbs any remaining separators.
1. Convert path bytes with `from_utf8_lossy` only at the edge, after all splitting.
1. Sort once: area order, then rendered path.

Total code: about 160 lines including the data model, with no regular expressions, no quoting
logic, no localization concerns, and no allocation beyond the output vectors and path strings.
Byte-oriented parsing here is not a micro-optimization; it is what makes the parser *correct* for
arbitrary path bytes, with speed as a side effect.

### Failure modes the parser survives

A status parser sits on the hottest refresh path in the application, so its behavior under
malformed input matters as much as its happy path:

**1. Short records return instead of panicking.** Every positional destructure is a `let ... else
{ return; }` on the expected field count. A truncated record contributes nothing rather than
shifting fields into the wrong slots.

**2. A torn rename degrades.** A `2` record with no following record gets an empty original path,
producing a rename entry without its source rather than consuming a header or another change as
its "original path".

**3. Unknown markers are skipped.** Future record types, or the `!` records of an `--ignored` run,
fall through the dispatch silently.

**4. Invalid UTF-8 degrades visibly, not fatally.** Lossy conversion turns undecodable path bytes
into replacement characters in the UI while the snapshot as a whole stays usable.

**5. Malformed counts become zero.** Ahead/behind and any numeric decoration default rather than
error.

The unifying principle: the status snapshot is rebuilt from scratch on every read, so any single
bad record costs at most one entry in one snapshot, and the next watcher event replaces it
entirely. ARCHITECTURE.md invariant 4 leans on the same property from the other side: "Watcher
signals are lossy by design: one full status snapshot subsumes all preceding file events."
Snapshot semantics make both the parser's error handling and the watcher's coalescing safe.

## The machine diff family

Where status answers "what changed in the working tree", the diff family answers "what changed
between any two trees", and Quinjet asks it in three escalating levels of detail: names and
statuses, per-file line totals, and full patch text. The three levels have wildly different costs,
and the whole local and PR diff architecture is built on requesting the cheapest level that
answers the current question. The formats are documented in
[git-diff](https://git-scm.com/docs/git-diff); this section covers each level's byte format, then
the Quinjet machinery that consumes it.

### Listing names and statuses

`git diff --name-status` prints one entry per changed file: a status field, then the path or
paths. The status letters are `A` added, `C` copied, `D` deleted, `M` modified, `R` renamed, `T`
type changed, `U` unmerged, plus the rare `X` (a Git bug indicator) and `B` (pairing broken, only
with `-B`). Rename and copy statuses carry a similarity score suffix: `R100`, `C68`.

In `-z` mode the entry is framed as separate NUL-terminated records: the status record, then one
path record, or two path records (old, then new) for renames and copies:

```text
M    \x00 src/app.rs \x00
R100 \x00 old/name.rs \x00 new/name.rs \x00
A    \x00 assets/icon.svg \x00
```

(Spacing added for readability; the real stream is contiguous.) This three-record rename shape is
the single most important fact about parsing the listing, and it is the reason Quinjet's index
parser walks records with a cursor instead of mapping one record to one file.

Cost model: `--name-status` computes tree-level differences and, with `--find-renames`, content
similarity for rename pairing, but it never *prints* patch text. On a partial clone it may still
need blob data for the similarity pass, a nuance that matters for the PR workspace and is picked
up in [the pull-request side of the same commands](#the-pull-request-side-of-the-same-commands).
For local diffs it is dramatically cheaper than patch generation, which is why it is the first
read of every diff workspace: collapsed file headers can render from it alone (ARCHITECTURE.md
invariant 8).

### Numstat and its rename shape

`git diff --numstat` prints per-file added and deleted line counts without patch bodies:

```text
12   3    src/app.rs
-    -    assets/logo.png
```

Each record is `<additions> TAB <deletions> TAB <path>`. Binary files print `-` in both count
columns; that is a *cannot count lines* marker, not a zero. In newline mode a rename renders the
path as a combined `old => new` notation; in `-z` mode it uses a stranger but far more parseable
encoding: the path field after the second tab is *empty*, and the pre-image and post-image paths
follow as two separate NUL-terminated records:

```text
7 \t 2 \t \x00 old/name.rs \x00 new/name.rs \x00
```

Quinjet's `parse_numstat` (`src/git/diff.rs:147`) handles exactly this, and its doc comment states
the rule: "Renames emit an empty path field followed by the pre-image and post-image records, so
the scanner has to consume those two extra records instead of assuming one." From the source:

```rust
let binary = additions == b"-" || deletions == b"-";
let entry = DiffLineCounts {
    additions: parse_count(additions),
    deletions: parse_count(deletions),
    binary,
};
if path.is_empty() {
    let Some(new_path) = records.get(cursor + 1) else {
        break;
    };
    cursor += 2;
    let _ = counts.insert(record_path(new_path), entry);
} else {
    let _ = counts.insert(record_path(path), entry);
}
```

Renamed entries are keyed by the post-image path, matching how the name-status index keys its
entries, so the two listings join on the same key. Binary files get `binary: true` and zero
counts (`parse_count` maps `-` to 0), and the UI renders a `binary` marker instead of `+0 -0`.

### Rename detection scores

Both listings above pass `--find-renames` (the long spelling of `-M`), and it is worth being
precise about what that buys and costs, because Quinjet passes it on *every* diff invocation, from
index listings to patch reads.

Git does not record renames; a rename is inferred at diff time. Detection runs in two passes over
the deleted-and-added path pairs: an exact pass that matches identical blob contents by object id
(nearly free, since equal ids need no content comparison), then a similarity pass that scores
remaining candidate pairs by shared content and pairs those above the threshold (50 percent by
default; `--find-renames=<n>` tunes it). The candidate matrix is bounded by `diff.renameLimit` to
keep the quadratic pass from exploding on huge diffs.

Why always on: a moved file that is not paired renders as a full deletion plus a full addition,
doubling the apparent size of the change and destroying the reviewer's ability to see what
actually changed inside the file. Consistency also matters across read levels: if the name-status
index paired a rename but the patch read did not (or vice versa), the header and its body would
describe different file sets. Passing the same `--find-renames` everywhere keeps every level of
the pipeline describing the same diff. The pairing algorithm itself and its interaction with diff
algorithms are covered in [diff algorithms](../diff/algorithms.md).

One consequence surfaces in patch reads: for a renamed file, the single-file patch command must
restrict the diff to *both* names, or Git cannot form the pair inside the path-limited diff.
`append_diff_file_paths` (`src/git/mod.rs:1368`) pushes the old path before the new one whenever
the index entry carries one, so path-scoped rename patches stay renames.

### One argv, two listings

Quinjet's headers show real `+n -n` counts before any patch exists (ARCHITECTURE.md invariant 8a:
"Every index also reads `git diff --numstat` over the same range, so a header shows its real
`+n -n` before that file has a patch"). That requires running *two* listing commands over the
*same* diff, and "same" is a stronger requirement than it looks. A diff is defined by its revision
range, its rename detection settings, and its path limits; if the name-status read and the numstat
read disagreed on any of them, a header could show totals for a subtly different file set: a
rename paired in one listing and split into add-plus-delete in the other, for example.

Quinjet makes disagreement inexpressible by deriving one argv from the other. The canonical
listing argv comes from `diff_index_args` (`src/git/mod.rs:1312`):

```rust
fn diff_index_args(base: &str, head: &str) -> Vec<OsString> {
    vec![
        OsString::from("diff"),
        OsString::from("--name-status"),
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from(base),
        OsString::from(head),
        OsString::from("--"),
    ]
}
```

And the totals argv is that exact vector with exactly one token swapped, by `numstat_args`
(`src/git/mod.rs:1326`), whose doc comment states the intent: "Reuse an index command's own
revision range for its totals by swapping the listing option. This keeps the two reads describing
exactly the same diff."

```rust
fn numstat_args(args: &[OsString]) -> Option<Vec<OsString>> {
    let name_status = OsStr::new("--name-status");
    args.iter().any(|arg| arg == name_status).then(|| {
        args.iter()
            .map(|arg| {
                if arg == name_status {
                    OsString::from("--numstat")
                } else {
                    arg.clone()
                }
            })
            .collect()
    })
}
```

The function returns `None` when the argv has no `--name-status` token (some index sources, like
the working tree, build counts differently), and otherwise clones the argv wholesale. Revisions,
`--find-renames`, `-z`, path limits, ordering: everything except the listing mode is byte-identical
by construction. There is no second command template to keep synchronized, so there is no way for
a future edit to desynchronize it. This is the same "make the bug inexpressible" pattern as the
absent unquoting layer, applied one level up.

A test at `src/git/mod.rs:1961` pins the user-visible consequence: "a branch index must know every
file's totals before any patch is read".

### The index parser walkthrough

`diff_index_files` (`src/git/mod.rs:519`) turns the two listings into the `DiffFileIndexEntry`
vector every diff view is built from. Step by step:

**1. Totals first.** `numstat_args` derives the counts argv, and `numstat_counts`
(`src/git/mod.rs:513`) runs it into a `HashMap<PathBuf, DiffLineCounts>`. Its doc comment sets the
error policy: "Counts are a rendering enhancement, never a correctness requirement, so a failed or
bounded read simply leaves the affected headers unresolved."

```rust
fn numstat_counts(&self, args: Vec<OsString>) -> HashMap<PathBuf, DiffLineCounts> {
    self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)
        .map(|(output, _)| parse_numstat(&output))
        .unwrap_or_default()
}
```

A numstat failure produces an empty map, headers render `+·· -··` placeholders, and nothing else
is affected. The file *list* is the correctness-critical read; the totals are decoration with a
graceful absence.

**2. The bounded listing.** The name-status command runs through
`checked_bounded(args, MAX_DIFF_INDEX_BYTES)` with the 8 MiB index cap. If the child was killed on
the cap, the buffer is cut back to the byte after the last NUL, so only whole records survive:

```rust
let (mut output, command_truncated) = self.checked_bounded(args, MAX_DIFF_INDEX_BYTES)?;
let mut truncated = command_truncated || truncate_diff_index(&mut output);
if command_truncated && !output.ends_with(&[0]) {
    let boundary = output
        .iter()
        .rposition(|byte| *byte == 0)
        .map_or(0, |index| index + 1);
    output.truncate(boundary);
}
```

**3. The cursor walk.** Records split on NUL, empties dropped, then a cursor consumes one status
record plus one or two path records per entry:

```rust
let status_code = status.first().copied().unwrap_or_default();
let rename_or_copy = matches!(status_code, b'R' | b'C');
```

The first byte of the status record decides the record count: `R` and `C` (whose records are
actually `R100`-style with a score suffix; only the first byte matters here) consume an old path
and a new path, everything else consumes one path. A missing expected record sets
`truncated = true` and stops cleanly, which is exactly the failure shape a capped read can
produce even after the NUL-boundary repair, since the cap can land between an `R100` record and
its second path.

**4. Admission control.** The loop stops at `MAX_DIFF_INDEX_FILES` (16,384) entries and marks the
index truncated. The cap is not about parse cost alone: every index entry becomes UI state, tree
nodes, and prefetch candidates, so the bound caps the whole downstream pipeline. ARCHITECTURE.md
invariant 5 lists it among the global caps.

**5. Assembly.** Each entry gets its status label via `diff_status_label` (`src/git/mod.rs:1355`:
`A` added, `M` modified, `D` deleted, `R` renamed, `C` copied, `T` "type changed", `U` unmerged,
anything else "changed"), its optional `counts` looked up by path from the numstat map, and its
optional `old_path` for renames. The result is `(Vec<DiffFileIndexEntry>, bool)`, the bool being
the honest truncation flag that the UI surfaces instead of silently showing a partial list as
complete.

### Working-tree totals in two calls

The `Changes` variant of a local diff (the working tree view) never runs `--name-status` at all:
the file list is already known from the porcelain v2 status snapshot, so running a listing command
again would be a second subprocess to learn the same facts. Only the totals are missing, and
`apply_worktree_counts` (`src/git/mod.rs:472`) fills them with at most two numstat calls
regardless of how many files changed. Its doc comment: "Working-tree changes are already known
from the status snapshot, so the index needs only their totals. One `--numstat` read per populated
area keeps that to at most two extra Git calls regardless of file count." From the source:

```rust
let counts_for = |staged: bool| {
    let mut args = vec![OsString::from("diff"), OsString::from("--numstat")];
    if staged {
        args.push(OsString::from("--cached"));
    }
    args.extend([
        OsString::from("-z"),
        OsString::from("--find-renames"),
        OsString::from("--"),
    ]);
    self.numstat_counts(args)
};
```

The two calls map to Git's two implicit comparisons: bare `git diff --numstat` compares worktree
against index (unstaged totals), and `--cached` compares index against HEAD (staged totals). Each
runs only if its area actually has changes, and each file's counts come from the map matching its
own area, so a path with both staged and unstaged edits shows the right totals in each section.
Untracked files get no counts here at all; there is nothing for `git diff` to compare an untracked
file against, and a test at `src/git/mod.rs:1876` pins that their headers stay unresolved rather
than showing a fabricated zero.

The arithmetic is worth stating because it is the difference between constant and linear
subprocess cost: a working tree with 500 changed files costs one status read plus at most two
numstat reads, not 500 per-file probes. Batching by *area* instead of by *file* is the same
economics that later drives PR patch batching.

### Patch reads and their flags

The third level is real patch text, and Quinjet defers it as long as possible: patches are read
per file, on selection, or in background batches, never eagerly for a whole index. The canonical
single-file patch argv, from `revision_diff_file` (`src/git/mod.rs:618`):

```text
git diff --no-color --no-ext-diff --find-renames --patch
    --unified=3 <base> <head> -- [old_path] <path>
```

Flag by flag:

- `--no-color`: color is for terminals; escape sequences in patch text would have to be stripped
  before parsing. Belt and braces alongside the machine-format defaults, since a user's
  `color.diff=always` configuration would otherwise inject ANSI bytes.
- `--no-ext-diff`: Git can be configured to run an arbitrary external program instead of its
  internal diff engine (`diff.external`, per-path `diff=<driver>` attributes). An external driver
  produces output in no particular format and runs a program Quinjet did not choose. This flag
  guarantees the internal engine and therefore the unified format the parser expects, and it
  keeps a background read from executing configured third-party tools.
- `--find-renames`: consistency with the index listings, as covered above.
- `--patch`: explicit patch output, making the intent unambiguous regardless of diff defaults.
- `--unified=3` or `--unified=1000000`: the context radius. The expanded diff view is implemented
  entirely by this one number: 1,000,000 context lines means "the whole file is context", so
  expanded mode needs no second command shape, no `git show` of full blobs, and no client-side
  merging of context. One flag value turns the same pipeline into a whole-file viewer.
- `--` then the paths, with the rename's old path included per `append_diff_file_paths`.

Variants reuse the shape rather than inventing new ones: root commits go through `git show
--format= ...` (a diff against the empty tree, with `--format=` suppressing the commit header so
only patch text remains); working-tree files add `--cached` for the staged copy or `--cc` for
conflicts (the combined format that shows both parents of an unmerged path at once); stashes diff
`stash@{n}^1` against `stash@{n}` and append a second read of the stash's third parent (where
`git stash` stores untracked files) when it exists, both halves sharing a single 8 MiB budget.
Untracked files skip Git entirely and synthesize a valid unified diff in Rust, down to the
`\ No newline at end of file` marker, so the downstream parser sees one format for every source.

Every one of these reads is capped at `MAX_DIFF_BYTES` (8 MiB) through the bounded runner, and a
truncated patch is trimmed back to a complete line before parsing. What the parser builds from
the bytes, and how collapsed headers and lazy bodies compose into one document, is the subject of
[the diff pipeline](../diff/pipeline.md).

### The pull-request side of the same commands

The PR module runs the same three levels with the same flags, but inside a different repository
(the disposable bare workspace described in [shallow and partial clone](./shallow-and-partial-clone.md))
and with one crucial substitution. The file listing is identical in shape,
`changed_files_in_repository` (`src/git/github/mod.rs:1981`):

```text
git diff --name-status -z --find-renames <merge_base> <head> --
```

with its own equivalents of the caps (`MAX_PR_PATH_BYTES` 8 MiB, `MAX_PR_PATHS` 16,384). The
substitution is in the totals. The PR workspace is fetched with `--filter=blob:none`, so file
*contents* are not local; a local `--numstat` there would force Git to fetch every changed blob
just to count its lines, defeating the entire partial-clone strategy. The doc comment on
`pull_request_file_counts_from_api` (`src/git/github/mod.rs:1235`) states it: "In the blob-less
disposable workspace a local `--numstat` would download every changed blob just to count lines;
GitHub already knows the totals." So on the workspace path, per-file counts come from the GitHub
pulls files endpoint (change #49), and the local numstat runs only on the fast path where the
opened repository already has both commits and blobs. The count source is selected in one line
(`src/git/github/mod.rs:1996`): `api_counts.unwrap_or_else(|| numstat_counts(...))`. The API
mechanics live in [the GitHub API strategy](../github/api-strategy.md); the scheduling
consequences of having counts at all are picked up in
[Where plumbing meets scheduling](#where-plumbing-meets-scheduling).

Patch reads on the PR side add one more trick: batching. `diff_selected_paths`
(`src/git/github/mod.rs:2141`) issues the same patch argv but with *many* paths after the `--`,
producing one combined patch that is split back into per-file documents at its `diff --git`
boundaries. Process spawn overhead, not diff computation, dominates a wide PR read, and the doc
comment on the batch method says exactly that: "Spawning one Git process per file dominates the
cost of a wide pull request, so batching is what lets the whole diff arrive while the reader is
still reading the first file." The splitting mechanics are covered in
[Splitting a batched patch apart](#splitting-a-batched-patch-apart).

## Delimited logs and ref listings

The third dialect family is caller-defined formats: `git log --format` and
`git for-each-ref --format`, where the caller writes a template and Git substitutes values. The
stability story inverts here. Git freezes nothing about the *output* because the caller defines
it; what Git freezes is the *placeholder language*. The caller's design burden is choosing
delimiters that the substituted values cannot contain, and this is where Quinjet reaches below
printable ASCII.

### The pretty-format language

[git-log](https://git-scm.com/docs/git-log) documents the pretty-format placeholders. The twelve
Quinjet uses, in the order they appear in its format:

| Placeholder | Value |
|---|---|
| `%H` | full commit hash |
| `%h` | abbreviated commit hash |
| `%P` | parent hashes, space separated |
| `%aN` | author name, mailmap-applied |
| `%aE` | author email, mailmap-applied |
| `%aI` | author date, strict ISO 8601 |
| `%cN` | committer name, mailmap-applied |
| `%cE` | committer email, mailmap-applied |
| `%cI` | committer date, strict ISO 8601 |
| `%ar` | author date, relative ("2 hours ago") |
| `%s` | subject (first line of the message) |
| `%D` | ref names pointing here, comma separated ("decorations") |

Three selection details carry weight. The mailmap-applied name forms (`%aN`, `%cN`) mean a
repository's `.mailmap` canonicalization applies exactly as it would in `git log` for the user,
keeping Quinjet's history pane consistent with Git's own identity resolution. The strict ISO
forms (`%aI`, `%cI`) are fixed-shape timestamps that downstream code can parse or compare
lexically, while `%ar` delegates human-friendly relative dating to Git instead of reimplementing
it against the same clock. And `%D` provides decorations (branch tips, tags, HEAD) as data, so
the UI colors them itself rather than parsing `git log --decorate`'s parenthesized display form.

The format string also uses `%x1f` and `%x1e`, the hex-byte escape: `%x` followed by two hex
digits emits that raw byte into the output. This is the mechanism that lets a format place
arbitrary control bytes as delimiters.

### Unit and record separators

The delimiter problem: a commit subject can contain almost anything. Tabs, pipes, semicolons,
quotes, and any printable character all appear in real subjects, so any printable delimiter will
eventually collide and shift every following field. Newlines are no better as record separators,
because although `%s` cannot contain one (the subject is by definition the first line), a
multi-value field like `%D` plus future format evolution makes line-based framing fragile, and
bodies (`%b`, which Quinjet does not request) freely contain them.

ASCII solved this problem in 1963. Code points 0x1c through 0x1f are *information separators*,
designed to structure serial data streams: FS (0x1c) file separator, GS (0x1d) group separator,
RS (0x1e) record separator, US (0x1f) unit separator. They are non-printing control characters
with no keyboard representation, which is precisely why they essentially never occur in commit
metadata: nothing a person types into a commit message editor or a name field produces them. Git
additionally forbids the characters `<`, `>`, and newline in ident (name/email) fields outright.

`LOG_FORMAT`, from `src/git/history.rs:22`:

```rust
pub(crate) const LOG_FORMAT: &str =
    "%H%x1f%h%x1f%P%x1f%aN%x1f%aE%x1f%aI%x1f%cN%x1f%cE%x1f%cI%x1f%ar%x1f%s%x1f%D%x1e";
```

Fields separated by US (0x1f), records terminated by RS (0x1e). The doc comment above it is
precise about the claim: the separators "avoid ambiguity with spaces, tabs, and most text that
can occur in names or commit subjects." *Most*, not *all*: a commit subject is an arbitrary byte
string in principle, and a hostile committer could embed a 0x1f. The parser's tolerance for that
case is part of the design and covered below; the point of the choice is that the collision
probability drops from "happens in normal repositories" (tabs, pipes) to "requires deliberately
crafted binary bytes in a subject line".

A worked example from the parser's own test (`src/git/history.rs:107`), one merge commit followed
by a root commit, with the control bytes shown symbolically and the record split onto lines for
readability:

```text
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa                US
aaaaaaa                                                 US
bbbbbbbb...bbbb cccccccc...cccc                         US
Ada Lovelace                                            US
ada@example.com                                         US
2026-01-02T03:04:05Z                                    US
Linus Torvalds                                          US
linus@example.com                                       US
2026-01-02T04:05:06Z                                    US
2 hours ago                                             US
Merge a fast thing                                      US
HEAD -> main, origin/main, tag: v1                      RS
```

The `%P` field carries two space-separated parents (a merge), and `%D` carries three decorations
that the parser later splits on commas. The second record in the test has an *empty* `%P` (a root
commit) and an empty `%D`, both of which must round-trip as empty vectors, not as one-element
vectors holding an empty string.

### Pages of 300 commits

History is unbounded; terminals are not. Quinjet never reads a repository's full history in one
command. The history argv, from `Repository::history` (`src/git/mod.rs:330`):

```text
git log --topo-order --decorate=short --no-color
    --skip=<n> --max-count=<limit> --format=<LOG_FORMAT> <revision> --
```

`--skip` and `--max-count` implement paging, and the page size is pinned in two constants that
agree by design: `DEFAULT_HISTORY_PAGE = 300` in `src/git/mod.rs:29` (substituted when a caller
passes `limit == 0`) and `HISTORY_PAGE_SIZE = 300` in `src/app.rs:32` (what the terminal actually
requests as the reader scrolls). Each page is one subprocess whose output is bounded by the page
size itself, which is why this is the one read in the module that uses the unbounded `checked`
runner rather than a byte cap: 300 records of bounded-size fields cannot meaningfully balloon.

The supporting flags: `--topo-order` gives parents-before-children ordering so the history pane
reads as a coherent graph rather than raw date order interleaving unrelated branches;
`--decorate=short` populates `%D` with short ref names; `--no-color` guards against
`log.decorate` and color configuration leaking display formatting into `%D`; the trailing `--`
closes the revision-versus-path ambiguity.

One honest cost note: `--skip=<n>` is implemented by walking and discarding, so page N of history
costs a walk over N pages of commits inside Git. The page *output* stays constant-size, and pages
are only requested as the reader actually scrolls deeper, so the growing walk cost is paid rarely
and by explicit demand. Commit-graph acceleration for these walks is discussed in
[merge bases and history](./merge-bases-and-history.md).

The revision guard above this argv re-validates that `<revision>` is `HEAD`, a full ref in an
allowed namespace, or a full object id, as covered in
[Validation before anything reaches an argv](#validation-before-anything-reaches-an-argv): the
revision sits in option position, so shape validation is its only injection defense.

### The history parser, annotated

`parse_log` (`src/git/history.rs:25`) is four lines: split on 0x1e, parse each record, drop
failures. `parse_record` (`src/git/history.rs:32`) does the real work:

```rust
let record = trim_ascii(record);
if record.is_empty() {
    return None;
}
let fields: Vec<&[u8]> = record.split(|byte| *byte == 0x1f).collect();
```

The `trim_ascii` matters because `git log` prints a newline after each record's terminator; the
byte between one RS and the next field is `\n`, and trimming ASCII whitespace at record edges
removes it without a special case. Then a slice pattern destructures exactly twelve leading
fields with a trailing `..` rest pattern:

- Fewer than twelve fields: the record returns `None` and is dropped. A crafted 0x1e in a
  subject splits one record into two; the fragments fail the field count and vanish, costing one
  history row rather than shifting fields across every following commit.
- More than twelve fields: the extras are ignored by the `..`. A crafted 0x1f in a *decoration*
  (the final field) is absorbed harmlessly; one in an earlier field would shift that one record's
  later fields, a bounded, single-record corruption with no panic and no cross-record damage.

Field post-processing is minimal and total: `%P` splits on ASCII whitespace into `parent_ids`
(empty field, empty vector), `%D` splits on commas with trimming and empty-filtering into
`decorations` (preserving compound items like `HEAD -> main` and `tag: v1` intact), and every
field passes through lossy UTF-8 conversion at the edge, same as the status parser. The
`Commit` struct stores the two ISO timestamps and the relative date as the strings Git printed;
no date parsing happens on this path at all.

### Branch listings without git branch

`git branch` output is porcelain with no machine mode worth the name: columnar, localized
markers, subject truncation. Quinjet skips it entirely and uses
[git-for-each-ref](https://git-scm.com/docs/git-for-each-ref), the plumbing iterator over refs,
with the same delimiter strategy as the log format. From `Repository::branches`
(`src/git/mod.rs:801`):

```rust
let output = self.checked([
    OsString::from("for-each-ref"),
    OsString::from("--sort=-committerdate"),
    OsString::from(
        "--format=%(refname:short)%1f%(HEAD)%1f%(upstream:short)%1f%(committerdate:iso-strict)%1f%(objectname:short)%1e",
    ),
    OsString::from("refs/heads"),
])?;
```

Note the escape syntax difference: for-each-ref's format language writes a hex byte as `%1f`
directly, where the log format required `%x1f`. Two format languages, one delimiter design. The
atoms: `%(refname:short)` is the branch name as a user would type it; `%(HEAD)` prints `*` for
the currently checked-out branch and a space otherwise, so `current` parses as a byte equality
(`*head == b"*"`); `%(upstream:short)` is the tracking branch or empty; the iso-strict committer
date and short object name complete the row. `--sort=-committerdate` pushes the ordering into
Git: the newest-active branch lists first, and Quinjet does no sorting of its own on this path.

`history_branches` (`src/git/mod.rs:833`) runs the same shape over both `refs/heads` and
`refs/remotes` with one extra atom, `%(symref)`, which is non-empty only for symbolic refs. That
one field cleanly removes `origin/HEAD` (a symref to the remote's default branch) from the
listing, an entry that is not a real branch and would otherwise duplicate its target:

```rust
if !trim_ascii(symref).is_empty() {
    continue;
}
```

Records outside the two allowed namespaces are skipped, `remote` derives from the full
`%(refname)` prefix, and one final in-process sort layers Quinjet's presentation policy on top of
Git's date ordering: `branches.sort_by_key(|branch| (!branch.current, branch.remote))` puts the
current branch first, then local branches, then remote-tracking branches, each group preserving
committer-date order. Ref storage itself (loose refs, packed-refs, symrefs) is covered in
[refs, index, and worktrees](./refs-index-and-worktrees.md).

### Stash listings through reflog selectors

Stashes are commits recorded on the `refs/stash` reflog, and
[git-stash](https://git-scm.com/docs/git-stash) list is a `git log` over that reflog, which means
it accepts the same custom format language. From `Repository::stashes` (`src/git/mod.rs:876`):

```text
git stash list --format=%gd%x1f%gs%x1f%cI%x1f%h%x1e
```

`%gd` is the shortened reflog selector, the `stash@{0}` name every stash operation takes; `%gs`
is the reflog subject. Two defensive layers wrap the parse. First, every selector is checked by
`valid_stash_reference` (exactly `stash@{<digits>}`) before it is stored, and invalid records are
skipped; the selector will later be passed back into `stash apply`, `stash drop`, and
`stash show` argvs, so its shape is validated at the trust boundary where it enters the system.
Second, `parse_stash_subject` (`src/git/mod.rs:1411`) recognizes the two subject conventions Git
writes, `WIP on <branch>: <message>` and `On <branch>: <message>`, splitting on the first `": "`;
a subject in neither shape (possible with `git stash store`) degrades to an empty branch and the
whole subject as the message rather than a misparse.

### Worktree listings in porcelain form

[git-worktree](https://git-scm.com/docs/git-worktree) list has a `--porcelain` mode with a
different framing again: attribute-per-line stanzas, one stanza per worktree, in the style of
`key value` lines with bare flag keys. With `-z` each attribute is NUL-terminated and stanzas are
separated by an empty record (a double NUL). A three-worktree repository looks like:

```text
worktree /home/user/project        \x00
HEAD 1261472...                    \x00
branch refs/heads/main             \x00
                                   \x00
worktree /home/user/project-wt/fix \x00
HEAD b753d26...                    \x00
branch refs/heads/fix/panel        \x00
                                   \x00
worktree /home/user/project.git    \x00
HEAD 56f4154...                    \x00
bare                               \x00
                                   \x00
```

`parse_worktrees` (`src/git/mod.rs:1423`) groups NUL-separated fields, treating an empty field as
the stanza boundary, and `worktree_from_fields` (`src/git/mod.rs:1446`) reads the attribute
prefixes: `worktree `, `HEAD `, `branch ` (reduced to a short name by stripping the
`refs/heads/` prefix), the bare flags `detached` and `bare`, and `locked`/`prunable` whose
optional remainder becomes the reason string. Two Quinjet-specific decisions sit on top:

**1. `current` is computed, not parsed.** Git does not know which worktree the *session* is in,
so `same_path` (`src/git/mod.rs:1503`) compares each stanza's path against the session root, raw
first and then through `fs::canonicalize` for both sides, making the flag robust to symlinked
paths. The current-worktree marker is per-session state layered onto Git's per-repository data.

**2. Windows paths are normalized.** `parse_worktree_path` (`src/git/mod.rs:1491`) rewrites `/`
to `\` on Windows, because Git prints forward slashes while the rest of the application compares
native paths.

A unit test (`src/git/mod.rs:2159`) feeds a literal porcelain byte string, double NULs and all,
pinning the framing against regressions. The worktree architecture this listing describes, and
the common-directory watch that keeps linked worktrees fresh, are in
[refs, index, and worktrees](./refs-index-and-worktrees.md).

## Plumbing probes

Not every question needs a record stream. A large share of Quinjet's Git traffic is single-fact
probes: does this object exist, what does this name resolve to, where is the repository root. The
probes are where classic plumbing shines, because their entire interface is an exit code and at
most one line of stdout.

### rev-parse: from names to objects

[git-rev-parse](https://git-scm.com/docs/git-rev-parse) is Git's name resolution service: it maps
the whole revision grammar (`main`, `v1.2`, `HEAD~3`, `abc123`, `branch@{upstream}`,
`rev^{commit}`, documented in [gitrevisions](https://git-scm.com/docs/gitrevisions)) onto object
ids and full ref names, and it exposes repository geometry. Quinjet uses five of its modes:

**1. `--show-toplevel`: discovery.** `Repository::discover` (`src/git/mod.rs:248`) runs
`git -C <path> rev-parse --show-toplevel` and stores the trimmed answer as the repository root.
This delegates the entire "am I inside a repository, and where does it start" walk to Git,
including every edge case Quinjet would otherwise reimplement: `GIT_DIR` overrides, `.git` files
pointing elsewhere (submodules, linked worktrees), and `ceilingDirectories`. A failure becomes
the "Not a Git repository" error. Every subsequent invocation passes this root back through
`-C`, so discovery happens exactly once per opened repository.

**2. `--verify [--quiet]`: existence and normalization.** `--verify` demands that the argument
resolve to a single object and exits non-zero otherwise; `--quiet` suppresses the error message
so a failed probe is silent. This pair is the workhorse of `resolve_revision` (shown earlier),
of `has_head` (`rev-parse --verify HEAD` distinguishes an unborn branch, which changes the
unstage strategy from `restore --staged` to `rm --cached`), and of the stash third-parent probe
(`rev-parse --verify --quiet <stash>^3` asks whether a stash recorded untracked files, deciding
whether the stash patch needs its second read).

**3. `--symbolic-full-name`: classification.** Given a symbolic input, it prints the full ref
name (`main` becomes `refs/heads/main`); given a raw object id, it prints nothing. Combined with
the prefix allowlist in `resolve_revision`, this is how user input gets *classified* into "safe
ref namespace" versus "must be an object id", rather than merely resolved.

**4. `^{commit}` peeling.** The suffix dereferences to a commit: a tag object peels to its
target, and anything that is not commit-ish fails verification. `resolve_revision` uses it as the
fallback so an annotated tag or a raw hash both normalize to something history and diff commands
accept, and `has_commit` and the PR module's `preferred_fetched_commit` use it so "the object
exists" always means "exists *as a commit*", not as a stray blob with a matching prefix.

**5. `--git-common-dir`: worktree geometry.** `git_common_dir` (`src/git/mod.rs:923`) resolves
the shared `.git` directory behind all linked worktrees, canonicalized. Three subsystems key on
it: the recent-projects list deduplicates by common directory (two worktrees of one repository
are one project), the filesystem watcher adds a second watch on the common directory when it
lies outside the worktree (so a commit made in a sibling worktree still refreshes the UI), and
the PR workspace lends the common directory's object store to its disposable repository through
an alternates file.

### cat-file: existence without output

[git-cat-file](https://git-scm.com/docs/git-cat-file) is the object database's front door:
`-t` prints an object's type, `-s` its size, `-p` its pretty-printed content. Quinjet needs none
of those; it needs exactly one bit, and `-e` provides it: exit 0 if the object exists and is
valid, non-zero otherwise, with *no stdout at all*. `has_commit` (`src/git/mod.rs:790`):

```text
git cat-file -e <oid>^{commit}
```

gated on `is_full_oid` so only full 40- or 64-character hex ids are ever probed. There is nothing
to parse, no pipe to bound, no truncation to consider; the exit status is the entire answer.

This tiny probe carries a disproportionate load: it is the hinge of the PR fast path. When a pull
request's base and head object ids both pass `has_commit` in the opened repository (typical when
previewing a PR for a branch built locally), `prepare_pull_request_diff`
(`src/git/github/mod.rs:767`) skips the network entirely: no metadata fetch for objects, no
disposable workspace, no `git fetch` at all. Two exit codes decide between zero network traffic
and the whole fetch ladder. ARCHITECTURE.md invariant 9 phrases the property from the user's
side: "PR patches first use immutable base/head OIDs already present in the opened repository,
which makes local-branch PR previews network-free." The object model that makes an OID's
presence a permanent, cache-safe fact is the subject of [the object model](./object-model.md).

### The batch mode Quinjet does not use

`cat-file` has a famous amortization mode: `--batch` and `--batch-check` read object names from
stdin and stream answers back, one resident process serving thousands of lookups without
per-probe spawn cost. Tools that hammer the object database (servers, indexers, large-scale
linters) rely on it. Quinjet deliberately does not, and the reasoning is a good case study in
matching machinery to actual load:

**1. The probe count is tiny.** Quinjet's existence checks arrive in pairs (base and head, once
per PR preparation) or singly (one `^3` probe per stash preview). There is no loop of thousands
of lookups anywhere in the design; the fast path needs two probes per PR open.

**2. A resident child is a lifecycle liability.** A `--batch` process must be spawned, owned by
some thread, guarded against crashes, restarted after repository changes that invalidate its
view, and killed on shutdown. Quinjet's worker model runs six independent lanes, each of which
would need its own resident child or synchronized access to a shared one. That is a real chunk
of state machinery.

**3. It cuts against the statelessness invariant.** Every Git interaction in Quinjet is a
stateless request-response with no process outliving its answer. That property is what makes
kill-on-cap safe (killing a one-shot child loses nothing), makes worker threads independent, and
makes the CLI and TUI able to share one code path. A resident helper would be the single
stateful exception, purchased to optimize a probe that runs twice per PR.

The general lesson: batch plumbing exists for batch workloads, and adopting it for a two-probe
workload buys measurable complexity for unmeasurable gain. Where Quinjet *does* have a batch
workload (wide PR patch reads), it batches through argv width instead, which amortizes the same
spawn cost with zero resident state.

### merge-base as a tri-state signal

[git-merge-base](https://git-scm.com/docs/git-merge-base) computes the best common ancestor of
two commits, the commit a three-dot `base...head` diff actually diffs against. Its exit code is
meaningful: 0 with the ancestor on stdout, 1 when no common ancestor exists, higher on real
errors. Quinjet reads that vocabulary in two places with two different interpretations:

- On the local fast path, `git merge-base <base_oid> <head_oid>` in the opened repository is
  expected to succeed, and its answer becomes the diff base for the whole PR index.
- In the shallow disposable workspace, `try_merge_base` (`src/git/github/mod.rs:1967`) treats a
  non-zero exit not as failure but as "the truncated history does not reach the ancestor yet":
  the fetch ladder deepens both sides (64, 256, 1,024, 4,096, 16,384) and retries. Only
  exhausting the ladder is an error, with a hard ceiling that refuses an unbounded history
  fetch.

A shallow clone genuinely changes the *semantics* of merge-base: the walk stops at the shallow
boundary, so absence of an answer means "not within this depth", a tri-state (found, deepen,
error) rather than a boolean. Why the API-resolved merge base usually skips this ladder
entirely, and what criss-cross histories with multiple merge bases mean for the answer, is
covered in [merge bases and history](./merge-bases-and-history.md).

### check-ref-format: delegated validation

[git-check-ref-format](https://git-scm.com/docs/git-check-ref-format) exists purely to answer
"is this a well-formed ref name", and its `--branch` mode answers it for branch shorthand,
printing the normalized name. Git's ref-name rules are a genuine minefield (no component may
begin with `.` or end with `.lock`, no `..`, no ASCII control characters, no `?`, `*`, `[`, no
leading or trailing `/`, no `@{`, not the single character `@`, and more), and they have grown
over time. `validate_branch_name` (`src/git/mod.rs:1235`) does the only future-proof thing:
after trimming and rejecting empty input, it hands the candidate to
`git check-ref-format --branch <name>` and lets Git rule. Every `CreateBranch` and
`RenameBranch` operation passes through it before any mutating command runs, so the mutation
itself can never be the place where a bad name is discovered.

This is the purest expression of the page's recurring theme: where Git offers an authoritative
predicate, Quinjet spawns the predicate instead of approximating it.

## Capped pipes and kill-on-cap

Everything above assumed the child's output arrives and fits in memory. That assumption fails
exactly when it hurts most: a generated bundle in a diff, a vendored dependency update, a
minified asset, a runaway log. This section covers the mechanism that makes every read on this
page safe against unbounded output: `run_bounded_command` (`src/git/github/mod.rs:2222`), the
universal child runner for both `git` and `gh`, and the repair helpers that make its truncated
output parseable.

### Pipes, buffers, and the two-stream deadlock

A Unix pipe is a kernel buffer of finite capacity (commonly 64 KiB on Linux). A writer that
fills it blocks until a reader drains it. A child process with two piped streams can therefore
deadlock its parent in a well-known pattern: the parent reads stdout to completion before
touching stderr; the child, mid-run, fills the stderr pipe with warnings; the child blocks in a
stderr write, so it never finishes writing stdout; the parent blocks in a stdout read that will
never complete. Neither side is buggy in isolation. The protocol between them is.

Rust's `std::process::Command::output()` avoids this by draining both streams concurrently, and
Quinjet's unbounded `run` relies on that. But `output()` reads *everything*, with no way to stop
a child mid-stream, so the bounded runner has to reimplement concurrent draining itself:

- The calling thread reads stdout in 64 KiB chunks, enforcing the byte budget.
- A spawned thread runs `read_and_drain` (`src/git/github/mod.rs:2280`) on stderr with its own
  limit, and, critically, it keeps *reading to EOF even after its limit is reached*, discarding
  the excess. Capping stderr by simply not reading it would recreate the deadlock; the child
  must always find its stderr pipe drainable.

```rust
let remaining = limit.saturating_sub(collected.len());
collected.extend_from_slice(buffer.get(..read.min(remaining)).unwrap_or(&buffer));
```

That excerpt from `read_and_drain` is the whole trick: `collected` stops growing at the limit,
but the read loop does not stop. Memory is bounded while the pipe stays live.

### Why kill beats truncate-after

There are two ways to bound a subprocess read, and they are not equivalent:

**1. Read everything, then truncate.** Simple, and wrong twice. Memory first: the parent
materializes the full output before discarding the excess, so a 4 GiB patch costs 4 GiB of
allocation to produce an 8 MiB answer. Time second: the child runs to completion, so Git spends
the full cost of computing and formatting output that is thrown away, on a worker lane that
could be serving the next request.

**2. Stop reading at the cap and kill the child.** The parent's buffer never exceeds the cap
plus one read chunk, and, because the child dies mid-write, Git stops *producing*. The cap
becomes a bound on the child's work, not just the parent's memory.

ARCHITECTURE.md invariant 6 mandates the second: "Potentially large local and PR subprocess
output is read through capped pipes. Crossing a cap kills the child rather than first allocating
all output and truncating afterward." The enforcement, from `run_bounded_command`
(`src/git/github/mod.rs:2254`):

```rust
let remaining = stdout_limit.saturating_sub(collected.len());
if read > remaining {
    collected.extend_from_slice(buffer.get(..remaining).unwrap_or(&buffer));
    truncated = true;
    drop(child.kill());
    break;
}
```

The chunk that would cross the limit is trimmed to exactly fill it, the truncation flag is set,
and the child is killed on the spot. The `drop(...)` wrappers acknowledge that the kill itself
can fail (the child may already have exited); either way the loop exits and cleanup proceeds.

### The implementation, annotated

The full lifecycle of `run_bounded_command(command, stdout_limit, stderr_limit)`:

1. Pipe both streams, spawn the child, and take both handles; a child that exposes neither is
   an immediate error.
1. Spawn the stderr drain thread with `stderr_limit`. From this point stderr can never block the
   child.
1. Loop reading stdout into a 64 KiB stack buffer (the buffer is deliberately "one page of
   stack" per the in-source lint annotation; the collection vector pre-allocates
   `stdout_limit.min(64 KiB)` so a small cap does not reserve a large buffer).
1. Per iteration: `Ok(0)` is EOF, break. `ErrorKind::Interrupted` retries the read. Any other
   read error kills the child, waits on it, joins the stderr thread, and returns the error, so
   even the failure path leaves no zombie process and no leaked thread.
1. Enforce the cap as shown above.
1. Drop the stdout handle, `child.wait()` to reap the process (preventing a zombie), then join
   the stderr thread; a panic inside it surfaces as a real error ("stderr reader thread
   panicked") rather than vanishing.
1. Return `BoundedOutput { status, stdout, stderr, stdout_truncated }`.

The struct, from `src/git/github/mod.rs:2211`:

```rust
pub(crate) struct BoundedOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
}
```

The `stdout_truncated` flag is what lets `checked_bounded` implement its "killed on the cap is
not a failure" exit-code rule, covered in
[Exit codes and error surfaces](#exit-codes-and-error-surfaces): a process killed mid-write
reports an unsuccessful status by definition, and the flag is the evidence that the non-zero
status was self-inflicted and the collected bytes are the intended answer.

One more property falls out for free: because the same runner serves `git` and `gh`, every
bound in the application, from an 8 MiB patch cap to a 2 MiB metadata cap to a 128 KiB fetch
log, is enforced by one audited code path. There is no second, subtly different bounded reader
to keep correct. Cached bytes even replay through the same shape: the PR module fabricates a
`BoundedOutput` with a synthesized success status (`successful_status()`,
`src/git/github/mod.rs:2124`) so disk-cached listings flow through the identical parsing path
as live output.

### Truncation repair at record boundaries

A killed child stops writing wherever it happens to be, so a truncated buffer usually ends
mid-record: half a patch line, or a status record without its path. Handing that tail to a
parser invites edge cases in every parser, so Quinjet repairs the buffer *before* parsing, with
one helper per framing:

**1. Line framing (patch text).** `truncate_to_complete_line` (`src/git/mod.rs:1554`) pops bytes
until the buffer ends with a newline:

```rust
fn truncate_to_complete_line(bytes: &mut Vec<u8>) {
    while bytes.last().is_some_and(|byte| *byte != b'\n') {
        let _ = bytes.pop();
    }
}
```

Its wrapper `truncate` (`src/git/mod.rs:1545`) is the cap-then-repair combination used when
Quinjet itself assembles output (the synthesized untracked patch). A unit test at
`src/git/mod.rs:1677` pins the semantics precisely: `b"first\nsecond\nthird\n"` capped at 15
bytes becomes `b"first\nsecond\n"`; the 15-byte prefix ends mid-`third`, and the repair drops
the partial line entirely.

**2. NUL framing (listings).** `truncate_diff_index` (`src/git/mod.rs:1341`) and the in-line
repair in `diff_index_files` cut a truncated buffer back to the byte after the last NUL, so
only whole records remain. The PR module applies the same NUL-boundary cut to its own listing
reads.

The division of labor is clean: the runner guarantees a byte bound, the repair helpers
guarantee a *record* bound, and the parsers then only ever see well-framed input plus one
honest boolean saying whether the tail was dropped. That boolean propagates all the way to the
UI, which renders an explicit truncation notice instead of presenting a partial diff as
complete.

### The cap table

Every bound the Git substrate enforces, with its constant and scope
(`src/git/mod.rs:25` unless noted):

| Constant | Value | Bounds |
|---|---|---|
| `MAX_DIFF_BYTES` | 8 MiB | every single-file or batched patch read, local and PR |
| `MAX_DIFF_INDEX_BYTES` | 8 MiB | every `--name-status` / `--numstat` listing |
| `MAX_DIFF_INDEX_FILES` | 16,384 | entries admitted into a local diff index |
| `MAX_GIT_ERROR_BYTES` | 128 KiB | stderr kept from any Git child |
| `DEFAULT_HISTORY_PAGE` | 300 | log records per history page when a caller passes 0 |
| `MAX_PR_PATH_BYTES` | 8 MiB | the PR changed-file listing (`src/git/github/mod.rs`) |
| `MAX_PR_PATHS` | 16,384 | entries admitted into a PR file index |

The `gh` substrate mirrors the design with its own set (2 MiB metadata, 256 KiB stderr, 8 MiB
check logs), detailed in [the GitHub API strategy](../github/api-strategy.md). Two properties
make the table more than a list of numbers. Every cap is enforced by the kill-on-cap runner or
by admission control during parsing, never by post-hoc truncation of a fully materialized
buffer. And the caps compose: an index is bounded in bytes *and* entries, a patch read is
bounded in bytes *and* repaired to line boundaries, so no single pathological input shape slips
between two bounds. What the caps protect downstream, syntax highlighting budgets and the
32 MiB parsed-document budget, is covered in
[intraline and highlighting](../diff/intraline-and-highlighting.md) and
[progressive loading](../rendering/progressive-loading.md).

## The invocation catalog

This section is the complete reference of external commands Quinjet spawns in non-test code,
organized by purpose, with the reasoning behind each flag set. Every `git` entry runs under the
substrate from [Spawning Git safely](#spawning-git-safely): `-C <root>`,
`-c core.quotepath=false`, `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`. The
per-flag rationales given once in earlier sections are not repeated per row; the "avoids" notes
call out what is specific to each command.

### Discovery and validation commands

| Invocation | Where | Purpose |
|---|---|---|
| `git rev-parse --show-toplevel` | `Repository::discover` | find the repository root once |
| `git rev-parse --git-common-dir` | `git_common_dir` | worktree identity, watcher root, alternates source |
| `git rev-parse --symbolic-full-name --verify --quiet <rev>` | `resolve_revision` | classify user input into a full ref |
| `git rev-parse --verify --quiet <rev>^{commit}` | `resolve_revision` | fall back to a commit object id |
| `git rev-parse --verify HEAD` | `has_head` | detect an unborn branch |
| `git rev-parse --verify --quiet <stash>^3` | `stash_diff_file` | does this stash carry untracked files |
| `git cat-file -e <oid>^{commit}` | `has_commit` | object existence with zero stdout |
| `git check-ref-format --branch <name>` | `validate_branch_name` | authoritative branch-name validation |
| `git merge-base <a> <b>` | PR fast path, `try_merge_base` | diff base; non-zero can mean "deepen" |

What this group avoids as a whole: any reimplementation of repository discovery, revision
grammar, ref-name rules, or ancestry computation. Each probe is one fact, answered mostly
through exit codes, so this is the cheapest traffic in the application.

### Status, history, and listings

```text
git status --porcelain=v2 --branch -z --untracked-files=all --ignore-submodules=none
```

The hot snapshot: branch state plus every change in one lock-free read, dissected field by field
in [Porcelain v2 status, field by field](#porcelain-v2-status-field-by-field). Avoids: localized
or v1 output, path quoting, a second call for branch state, directory-collapsed untracked
entries, config-hidden submodule changes, and (via the substrate) the optional index write.

```text
git log --topo-order --decorate=short --no-color --skip=<n> --max-count=300
    --format=<US/RS format> <revision> --
```

One 300-commit page per invocation, delimiter-framed, covered in
[Delimited logs and ref listings](#delimited-logs-and-ref-listings). Avoids: unbounded history
materialization, date-order interleaving, decoration parsing, and revision/path ambiguity.

```text
git for-each-ref --sort=-committerdate --format=<US/RS format> refs/heads [refs/remotes]
```

The two branch listings (sidebar and history-branch picker). Avoids `git branch` entirely:
porcelain-unstable, localized, columnar. Sorting is delegated to Git; the symref atom drops
`origin/HEAD`.

```text
git stash list --format=%gd%x1f%gs%x1f%cI%x1f%h%x1e
git worktree list --porcelain -z
```

Reflog selectors validated on the way in; porcelain stanzas parsed at byte level. Avoids:
trusting selector strings that later re-enter argvs, and parsing localized worktree summaries.

### The local diff reads

The three-level ladder from [The machine diff family](#the-machine-diff-family), with every
variant it takes:

| Level | Invocation | Source |
|---|---|---|
| files | `git diff --name-status -z --find-renames <base> <head> --` | commits, branch compare |
| files | `git diff-tree --root --no-commit-id --name-status -z -r --find-renames <oid> --` | root commits |
| files | `git stash show --name-status -z --include-untracked <ref> --` | stashes |
| totals | the files argv with `--name-status` swapped to `--numstat` | `numstat_args` |
| totals | `git diff --numstat [--cached] -z --find-renames --` | working tree, per area |
| patch | `git diff --no-color --no-ext-diff --find-renames --patch --unified=<n> <base> <head> -- [old] <path>` | commits, branches |
| patch | `git show --format= --no-color --no-ext-diff --find-renames --patch --unified=<n> <id> -- [old] <path>` | root commits |
| patch | `git diff --no-color --no-ext-diff --find-renames [--cached] [--cc] --unified=<n> -- <path>` | working tree |
| patch | the commit patch argv over `<ref>^1 <ref>`, plus `git show ... <ref>^3 --` when present | stashes |
| patch | no subprocess at all: synthesized in `untracked_patch` | untracked files |

Notes on the less obvious rows. `git diff-tree` is the plumbing sibling of `git diff` for
comparing tree objects directly; the `--root` flag makes it diff a parentless commit against the
empty tree, and `--no-commit-id -r` suppress the header line and recurse into subtrees, leaving
pure listing records. `git stash show --include-untracked` folds the stash's untracked third
parent into the listing, while the patch-level read handles that parent as a second bounded
invocation sharing the 8 MiB budget. `--format=` (an empty format) on `git show` strips the
commit header block so the output is patch text only, keeping one parser for all patch sources.

### The mutation table

Mutations run through `Repository::perform` (`src/git/mod.rs:945`), one argv per
`GitOperation` variant, every path list behind `--` via `with_paths`:

| Operation | Command(s) |
|---|---|
| Stage / StageAll | `git add -- <paths>` / `git add -A` |
| Unstage | `git restore --staged -- <paths>`, or `git rm --cached --ignore-unmatch -- <paths>` when unborn |
| UnstageAll | `git reset --mixed --quiet HEAD --`, or `git rm --recursive --cached .` when unborn |
| Discard | untracked: filesystem removal; unstaged: `git restore --worktree -- <paths>`; staged: `git restore --staged --worktree --source=HEAD -- <paths>` |
| Commit | `git commit [--amend] --message <message>` |
| Fetch / Pull / Push | `git fetch --all --prune` / `git pull` / `git push`, or `git push --set-upstream origin HEAD` after checking `git remote get-url origin` |
| Sync | `git pull` then the push logic |
| Checkout / CreateBranch | `git switch -- <branch>` / `git switch --create <name> [<start>]` |
| RenameBranch / DeleteBranch | `git branch --move -- <old> <new>` / `git branch --delete -- <branch>` |
| StashPush | `git stash push [--include-untracked] [--staged] [--message <m>] [-- <paths>]` |
| StashApply / Pop / Drop / Clear | `git stash apply --index <ref>` / `pop --index [<ref>]` / `drop <ref>` / `clear` |
| ResolveConflict | `git checkout --ours` or `--theirs -- <path>`, then `git add -- <path>` |
| CherryPick / Revert | `git cherry-pick <oid>` / `git revert --no-edit <oid>` |

Several rows encode Git subtleties worth naming. `restore` (split from the overloaded
`checkout` precisely to make these intents unambiguous) expresses "unstage" and "discard" as
distinct tree-source operations instead of `checkout` puns. `stash apply --index` restores the
staged/unstaged split that a plain `apply` would flatten. `branch --move` preserves the
branch's tracking configuration and reflog where a delete-and-recreate would not, and a test at
`src/git/mod.rs:1790` pins that behavior. `rm --cached --ignore-unmatch` is the unborn-branch
unstage: with no HEAD there is nothing to `restore` from, so entries are removed from the index
directly. `revert --no-edit` keeps a background mutation from ever invoking an editor, the
mutation-side cousin of `GIT_TERMINAL_PROMPT=0`.

Messages are passed as separate argv elements after `--message`, never concatenated into a
command string, so a commit message is byte-clean by construction: quoting a message for a
shell is a bug class Quinjet structurally does not have.

### The disposable workspace fetches

The PR workspace (built and torn down in `src/git/github/mod.rs`, described in depth in
[the PR workspace](../github/pr-workspace.md) and
[shallow and partial clone](./shallow-and-partial-clone.md)) adds a small set of commands run
against the temporary bare repository rather than the opened one:

```text
git init --bare --quiet <cache_root>/tmp/pr-<pid>-<counter>.git
git remote add origin <base_repo_url>
git fetch --quiet --force --no-tags [--filter=blob:none] --depth=<n> <remote> <refspec>
```

The fetch flags in one breath: `--filter=blob:none` defers all blob transfer to lazy on-demand
fetches (with an automatic retry without the filter for servers that refuse it); `--depth`
bounds history per the deepening ladder; `--no-tags` stops tag-following from dragging
unrelated history into a shallow clone; `--force` keeps the fixed `refs/quinjet/*` refspecs
updatable across force-pushes; `--quiet` keeps progress chatter out of the capped stderr.
Refspecs are fixed templates over API-validated object ids and ref names, so nothing
user-shaped reaches these argvs either.

### The gh substrate

GitHub traffic goes through spawned `gh` with the same discipline and its own environment
quartet (`run_gh_bounded`, `src/git/github/mod.rs:1337`): `GH_PROMPT_DISABLED=1` (never block a
worker on a question), `GH_PAGER=cat` (no pager process), `GH_NO_UPDATE_NOTIFIER=1` (no update
check noise on stderr), `NO_COLOR=1` (parseable bytes). ARCHITECTURE.md invariant 13 covers
both halves: "Read operations set `GIT_OPTIONAL_LOCKS=0`; `gh` runs with prompts, paging,
color, and update checks disabled on the worker thread."

The output dialect differs from Git's: `gh` can emit JSON, but Quinjet asks its `--jq` flag to
flatten responses into TSV records instead (`@tsv` escapes tabs, newlines, and backslashes, and
`unescape_tsv` reverses exactly that), so the hot metadata path parses delimited bytes just
like every Git listing, and cached entries stay greppable on disk. The command inventory
(`gh pr view`, `gh pr checks`, `gh api` with validated ETag reads and Link-header paging, the
compare and pulls-files endpoints, Actions job steps and logs) belongs to
[the GitHub API strategy](../github/api-strategy.md) and
[conversation and checks](../github/conversation-and-checks.md); it is listed here only to
complete the census of what the application spawns.

## Where plumbing meets scheduling

Machine formats are not just parsing hygiene; they feed the scheduler. The clearest case is the
per-file counts: a listing-level fact (`--numstat` or its API substitute) that ends up deciding
the order, size, and shape of every background patch read on a huge pull request. This section
traces that thread through the three changes that built it, because the current design is best
understood through what it replaced.

### Counts from the API instead of blobs

Change #49 moved PR per-file counts from local numstat to the GitHub pulls files endpoint on
the workspace path, for the partial-clone reason covered in
[the pull-request side of the same commands](#the-pull-request-side-of-the-same-commands): in
a `blob:none` workspace, counting lines locally means fetching blobs, so the counts that
GitHub already computed are read from metadata instead (up to 64 pages of 100 files, cached
immutably under the OID pair). The immediate benefit was header rendering, per invariant 8a:
every file's `+n -n` appears before any patch exists. The structural benefit came next: once
every index entry carries counts, the counts can price things.

### Smallest-first tiers: an evolution step

Change #50 was the first scheduler built on those counts. On very large pull requests (past
100,000 total lines or 1,000 files), background prefetch ordered files into size tiers and
fetched the smallest files first. The reasoning: small patches are cheap and numerous, so
front-loading them maximizes the number of files that become instantly openable per unit of
work, while the handful of giant files, which would each consume a whole batch's budget, sink
to the end where they cannot starve everything else.

Smallest-first was a real improvement over index-order fill, and it is preserved here as a
documented evolution step, but it optimized a proxy. The reader does not experience "how many
files have patches"; the reader experiences "does the file I am looking at have its patch".
A global size ordering ignores where the reader actually is: scrolled into a directory of
large files, the reader could wait behind hundreds of small patches for other directories.

### Viewport-anchored batches: the current design

Change #55 replaced the size-tier ordering with the current scheduler: prefetch starts at the
file the Files tree is actually showing and wraps around the rest of the index in order,
pricing each batch with the same per-file counts. The anchor, from `prefetch_anchor_index`
(`src/app.rs:5912`), whose doc comment states the policy: "Where background fill should start:
the first file visible in the Files tree, so patches land where the reader is looking and then
wrap around the rest of the index in order."

The batch builder, `request_pull_request_prefetch` (`src/app.rs:5930`), walks
`from_anchor.iter().chain(before.iter())` (the wrap-around, as an iterator chain over the two
halves of a `split_at`) and admits files into the batch under two limits at once, from the
constants at `src/app.rs:33`:

```rust
const PULL_REQUEST_PREFETCH_BATCH: usize = 32;
const PULL_REQUEST_PREFETCH_BYTE_BUDGET: usize = 6 * 1024 * 1024;
const PULL_REQUEST_PATCH_FALLBACK_ESTIMATE: usize = 512 * 1024;
const PULL_REQUEST_PATCH_LINE_ESTIMATE: usize = 80;
const MAX_PREFETCHED_PULL_REQUEST_FILES: usize = 4_096;
```

A batch closes at 32 files or when the next file's estimate would push it past 6 MiB, whichever
comes first, and the whole prefetch walk stops at 4,096 files. The estimate is where the
listing-level counts become bytes, `estimated_patch_bytes` (`src/app.rs:7052`):

```rust
fn estimated_patch_bytes(counts: Option<DiffLineCounts>) -> usize {
    counts.map_or(PULL_REQUEST_PATCH_FALLBACK_ESTIMATE, |counts| {
        counts
            .additions
            .saturating_add(counts.deletions)
            .saturating_mul(PULL_REQUEST_PATCH_LINE_ESTIMATE)
            .saturating_add(4_096)
    })
}
```

Eighty bytes per changed line plus 4,096 bytes of header overhead; a file whose counts are
unknown (binary, or a count the API could not report) is priced at a conservative 512 KiB. The
6 MiB budget deliberately undershoots the 8 MiB hard cap on the actual read: the estimate is a
heuristic, and the gap between budget and cap absorbs estimation error, so a batch that runs
heavier than priced still fits under the kill-on-cap limit instead of losing its tail. One
guard in the loop is easy to miss and important: the budget check runs only when the batch
already has a member (`!paths.is_empty()`), so a single file priced above 6 MiB still ships as
a batch of one rather than deadlocking the walk.

The relationship between the two designs is worth stating exactly, because both facts are true:
PR 50 introduced smallest-first size-tiered ordering, and PR 55 removed it in favor of
viewport-anchored wrap-around order. What survived from #50 is everything except the ordering:
counts-driven byte pricing, batch budgets, and the principle that the scheduler must know
patch sizes before reading patches. What #55 changed is the objective function, from "most
files ready soonest" to "the reader's current screen ready first, then everything else,
fairly". The full scheduler context (mailbox slots, workspace-keyed replies, the retry policy)
is in [prefetch](../github/prefetch.md), and the end-to-end progressive loading story with the
measured before/after on the benchmark pull request is in
[progressive loading](../rendering/progressive-loading.md) and [benchmarking](../benchmarking.md).

### Splitting a batched patch apart

The plumbing detail that makes batching workable at all: one `git diff` over 32 paths returns
one combined patch, and Quinjet must split it back into per-file documents. The splitter
(`split_patch_by_file`, `src/git/diff.rs:618`) scans for lines beginning `diff --git `,
`diff --cc `, or `diff --combined `, the three header forms a patch section can open with, and
keys each section by the old and new paths parsed from its header. This is the one place the
`-z`-less patch format's path embedding is load-bearing, and it is exactly why
`core.quotepath=false` sits in the substrate: the splitter matches header paths against index
paths byte for byte, which only works because neither side is quoted.

Truncation interacts with splitting carefully: when a capped batch read is cut, only the final
section can be incomplete, so only that section is marked truncated; a retry with a smaller
batch can then recover it. Complete sections are written into the per-file patch cache as a
side effect, so any later single-file open of those paths is served from disk. Batch replies
are keyed to the prepared workspace rather than any preview generation (ARCHITECTURE.md
invariant 10a), which is what makes a background batch structurally unable to clobber the file
a reader explicitly requested; the concurrency machinery behind that claim is in
[concurrency](../rendering/concurrency.md).

## Design alternatives and why they lost

Reimplementing Git's object traversal, rename detection, revision parser, configuration stack,
credential integration, or index transaction rules would create a second and inevitably
incomplete Git implementation. Human-oriented porcelain output is equally unsuitable because
color, localization, quoting, and layout are presentation contracts. Quinjet instead combines
stable porcelain protocols for machine records with plumbing commands for precise object and ref
questions, always passing argv directly and bounding the bytes it accepts.

## Where to go next

Continue with [the diff pipeline](../diff/pipeline.md) for the transformation from Git output to
rendered documents, [the pull-request workspace](../github/pr-workspace.md) for the isolated
fetch path, and [concurrency](../rendering/concurrency.md) for the worker boundary around every
command described here.
