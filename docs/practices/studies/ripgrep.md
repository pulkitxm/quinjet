# BurntSushi/ripgrep (67319 stars)

## 1. What the project is and what the clone measures

ripgrep is a line-oriented search tool that recursively searches a directory
tree for a regex pattern while respecting gitignore rules. The root manifest
states the mission directly, in `extras/ripgrep/Cargo.toml`:

```toml
description = """
ripgrep is a line-oriented search tool that recursively searches the current
directory for a regex pattern while respecting gitignore rules. ripgrep has
first class support on Windows, macOS and Linux.
"""
```

Industry adoption follows from two properties visible in the clone itself.
First, it is a single static binary with first-class support on every major
platform, produced for 14 targets by `extras/ripgrep/.github/workflows/release.yml`.
Second, it is not a monolith: the hard parts are published as reusable
library crates (`globset`, `ignore`, `grep-searcher`, `grep-printer`, and so
on), each with its own `docs.rs` link and README. `extras/ripgrep/crates/core/README.md`
says so explicitly:

```text
much of the heavy lifting of ripgrep is done via its constituent crates,
which can be reused independent of ripgrep.
```

Scale indicators measured directly from the clone (HEAD is commit
`3fce3b5bb0236da2df6d99672afb8a719642eca7`, package version `15.2.0`):

| Metric | Value |
|---|---|
| Rust source files | 110 |
| Total Rust LOC | 56,386 |
| LOC under `crates/` | 50,356 |
| LOC under `tests/` (integration suite) | 5,777 |
| Workspace member crates | 10 (plus the root `ripgrep` package) |
| Standalone packages outside the workspace | 1 (`fuzz/`) |
| `unsafe` sites across all crates | 5 |
| Largest single file | `crates/core/flags/defs.rs` at 8,161 lines |

The 10 workspace members are listed in `extras/ripgrep/Cargo.toml`:
`crates/globset`, `crates/grep`, `crates/cli`, `crates/index`,
`crates/matcher`, `crates/pcre2`, `crates/printer`, `crates/regex`,
`crates/searcher`, `crates/ignore`.

## 2. Repository layout

```text
ripgrep/
|-- Cargo.toml              root package (the rg binary) + [workspace]
|-- build.rs                embeds git hash, Windows manifest linking
|-- rustfmt.toml            3 lines of formatting policy
|-- .cargo/config.toml      per-target rustflags (static CRT)
|-- CHANGELOG.md            90 KB of release notes
|-- GUIDE.md                40 KB user guide
|-- FAQ.md                  42 KB frequently asked questions
|-- RELEASE-CHECKLIST.md    human release runbook
|-- ci/                     shell helpers used by workflows
|   |-- test-complete       zsh script diffing --help vs completions
|   `-- ubuntu-install-packages
|-- .github/
|   |-- ISSUE_TEMPLATE/     bug_report.yml, feature_request.md, config.yml
|   `-- workflows/          ci.yml, release.yml
|-- crates/
|   |-- core/               binary source, no own Cargo.toml
|   |   |-- main.rs         entry point (root [[bin]] points here)
|   |   |-- flags/          the entire CLI surface
|   |   `-- index/          feature-gated indexing (enabled.rs/disabled.rs)
|   |-- matcher/            grep-matcher: the Matcher trait
|   |-- regex/              grep-regex: default engine
|   |-- pcre2/              grep-pcre2: optional engine
|   |-- searcher/           grep-searcher: line-oriented search executor
|   |-- printer/            grep-printer: standard/JSON/summary output
|   |-- cli/                grep-cli: CLI plumbing utilities
|   |-- globset/            glob matching (widely reused)
|   |-- ignore/             gitignore + parallel directory walker
|   `-- grep/               facade crate re-exporting the above
|-- tests/                  one integration test binary against the real rg
|-- fuzz/                   cargo-fuzz package, excluded from the workspace
|-- benchsuite/             Python benchmark runner + committed result runs
|-- pkg/
|   |-- brew/ripgrep-bin.rb Homebrew formula (HomebrewFormula symlinks here)
|   `-- windows/Manifest.xml long-path-aware manifest linked by build.rs
`-- scripts/copy-examples   keeps doc code blocks and examples in sync
```

The split works because the dependency arrows only point one way: `core` is
pure glue over the published crates, and each library crate has a single
responsibility with its own README, LICENSE pair, and version. An unusual
detail: the binary's source lives in `crates/core/` but that directory has no
manifest. The root package claims it via `extras/ripgrep/Cargo.toml`:

```toml
[[bin]]
bench = false
path = "crates/core/main.rs"
name = "rg"
```

So the repository root is the binary crate, and `crates/` holds both its
source and its libraries in one uniform place.

## 3. Cargo manifest practices

Workspace inheritance is used for exactly the two keys that must stay in
lockstep, in `extras/ripgrep/Cargo.toml`:

```toml
[workspace.package]
edition = "2024"
rust-version = "1.96"
```

Member crates opt in with `edition.workspace = true` and
`rust-version.workspace = true`. Crucially, inheritance is not forced where it
would be wrong: `extras/ripgrep/crates/globset/Cargo.toml` and
`extras/ripgrep/crates/ignore/Cargo.toml` both pin their own
`rust-version = "1.88"`, because those crates are consumed by third parties
with older toolchains than the binary requires. MSRV is a per-crate contract,
not a workspace-wide slogan, and CI enforces the binary's MSRV with a
`pinned` build using `rust: 1.96.0` in `extras/ripgrep/.github/workflows/ci.yml`.

Other notable manifest practices:

- Version lines carry a machine-readable marker, e.g.
  `version = "0.4.20"  #:version` in `extras/ripgrep/crates/globset/Cargo.toml`.
  The `#:version` comment is the anchor for the `cargo-up` release tool named
  in `extras/ripgrep/RELEASE-CHECKLIST.md`.
- `autotests = false` plus an explicit `[[test]] name = "integration"`
  pointing at `tests/tests.rs` collapses the whole end-to-end suite into one
  test binary, which shares one harness and links once.
- Dependencies that need trimming get the long form with explicit features,
  as in `extras/ripgrep/crates/globset/Cargo.toml`:

  ```toml
  [dependencies.regex-automata]
  version = "0.4.18"
  default-features = false
  features = ["std", "perf", "syntax", "meta", "nfa", "hybrid"]
  ```

- A renamed dependency documents a fork migration in place:
  `memmap = { package = "memmap2", version = "0.9.0" }` in
  `extras/ripgrep/crates/searcher/Cargo.toml`.
- Platform-conditional dependencies are the norm:
  `[target.'cfg(windows)'.dependencies.winapi-util]` in
  `extras/ripgrep/crates/cli/Cargo.toml`, and the allocator swap in the root
  manifest applies only to
  `cfg(all(target_env = "musl", target_pointer_width = "64"))`.
- Feature flags are additive and forwarded: the root `pcre2` feature maps to
  `grep/pcre2`, which maps to the optional `grep-pcre2` crate. The risky
  in-development feature is named honestly: `unstable-index = ["dep:grep-index"]`.
  Deprecated features are kept as documented no-ops rather than removed
  (`simd-accel = []` with a `DEPRECATED` comment in several manifests),
  which preserves downstream builds.
- Profiles are layered. The everyday release profile keeps `debug = 1` so
  backtraces from users are useful. Shipping builds use a dedicated profile:

  ```toml
  [profile.release-lto]
  inherits = "release"
  opt-level = 3
  debug = "none"
  strip = "symbols"
  lto = "fat"
  panic = "abort"
  codegen-units = 1
  ```

  and `[profile.deb]` inherits `release-lto` for `cargo deb`.
- `package.metadata.deb` in the root manifest declares the full Debian asset
  map, including generated man pages and completions.
- `extras/ripgrep/.cargo/config.toml` sets `-C target-feature=+crt-static`
  for MSVC targets and `link-self-contained=yes` for musl, so distributed
  binaries are truly static.
- The fuzz package (`extras/ripgrep/fuzz/Cargo.toml`) sets `publish = false`
  and its own `[workspace]` table so it never pollutes the main lockfile.

There are no `[lints]` tables anywhere in the repository.

## 4. Formatting

`extras/ripgrep/rustfmt.toml` is three lines:

```toml
max_width = 79
use_small_heuristics = "max"
edition = "2024"
```

- `max_width = 79`: the classic terminal width, stricter than rustfmt's 100
  default; it keeps side-by-side diffs readable.
- `use_small_heuristics = "max"`: all the width-based heuristics (when to
  break a struct literal, a chain, an argument list) are allowed to use the
  full `max_width`, producing denser, more horizontal code.
- `edition = "2024"`: keeps rustfmt parsing in sync with the workspace
  edition even when invoked standalone.

Enforcement is a dedicated CI job in
`extras/ripgrep/.github/workflows/ci.yml` running
`cargo fmt --all --check`. There is no `.editorconfig` and no formatter for
non-Rust files; shell and Python under `ci/` and `benchsuite/` are formatted
by hand. One interesting committed editor file, `extras/ripgrep/.nvim.lua`,
configures rust-analyzer to check with `features = 'all'`, so anyone opening
the repo in Neovim analyzes the same feature set CI builds.

## 5. Linting

The headline finding: ripgrep uses no clippy at all. There is no
`clippy.toml`, no `[lints]` table, no clippy CI job, and zero `clippy::`
attributes in the source tree. The lint strategy is instead built from three
narrow, high-signal gates:

1. Every published library crate denies missing documentation at the crate
   root: `#![deny(missing_docs)]` appears in
   `extras/ripgrep/crates/cli/src/lib.rs`,
   `extras/ripgrep/crates/matcher/src/lib.rs`,
   `extras/ripgrep/crates/regex/src/lib.rs`,
   `extras/ripgrep/crates/pcre2/src/lib.rs`,
   `extras/ripgrep/crates/printer/src/lib.rs`,
   `extras/ripgrep/crates/ignore/src/lib.rs`,
   `extras/ripgrep/crates/globset/src/lib.rs`, and
   `extras/ripgrep/crates/searcher/src/lib.rs`. The one exception is the
   explicitly unstable crate: `extras/ripgrep/crates/index/src/lib.rs` opens
   with `#![allow(warnings)]`, an honest marker for code still in flux.
2. Rustdoc is a lint pass. The `docs` job in
   `extras/ripgrep/.github/workflows/ci.yml` runs
   `RUSTDOCFLAGS: -D warnings` with
   `cargo doc --no-deps --document-private-items --workspace`, so broken
   intra-doc links and malformed docs fail CI, including on private items.
3. Invariants are checked by purpose-built tests rather than generic lints.
   `extras/ripgrep/crates/core/flags/defs.rs` contains an inventory test that
   walks the global `FLAGS` slice and prints which ASCII short flags remain
   unclaimed, and CI runs it visibly
   (`cargo test --bin rg ... flags::defs::tests::available_shorts -- --nocapture`).
   `extras/ripgrep/ci/test-complete` parses `rg --help` output with ripgrep
   itself and diffs the flag list against the hand-written zsh completion in
   `extras/ripgrep/crates/core/flags/complete/rg.zsh`, failing CI when the
   two drift.

Suppressions are correspondingly rare: only 16 occurrences of `allow(` exist
across roughly 50,000 lines under `crates/`, and each is targeted, such as
`#[allow(dead_code)] // unused on Windows` in `extras/ripgrep/tests/util.rs`.
The philosophy is legible: invest in documentation completeness, doc
correctness, and domain-specific consistency checks; skip style lawyering.

## 6. CI/CD

`extras/ripgrep/.github/workflows/ci.yml` triggers on `pull_request`, on
pushes to `master`, and on a nightly cron (`00 01 * * *`). The first thing in
the file after the triggers is a least-privilege block with an unusually
thorough justification comment:

```yaml
# By specifying any permission explicitly all others are set
# to none. By using the principle of least privilege the damage a compromised
# workflow can do (because of an injection or compromised third party tool or
# action) is restricted.
permissions:
  # to fetch code (actions/checkout)
  contents: read
```

The `test` job is an 18-entry include matrix with `fail-fast: false`:
channel coverage (pinned `1.96.0`, `stable`, `beta`, `nightly`) plus target
coverage via `cross` (musl, i686, aarch64 gnu and musl, three armv7 flavors,
powerpc64, s390x, riscv64gc), plus macOS, two Windows toolchains, and
`windows-11-arm`. Cross-compiled targets run the full test suite under qemu,
so ripgrep's integration tests execute the real binary even on big-endian
s390x. `cross` itself is pinned (`CROSS_VERSION: v0.2.5`) and installed from
a prebuilt release tarball, with the reason recorded inline:

```yaml
# In the past, new releases of 'cross' have broken CI. So for now, we
# pin it. We also use their pre-compiled binary releases because cross
# has over 100 dependencies and takes a bit to compile.
```

Tests run twice per platform where affordable: once with
`--features unstable-index` and once with `--features pcre2`; under emulation
the PCRE2 pass is skipped with a comment explaining the runtime cost. Debug
aids are built into the pipeline: a step dumps the newest `build.rs` stderr
file from the target directory, and two `--nocapture` test invocations print
the detected hostname and the free short flags.

Four more jobs: `wasm` (build for `wasm32-wasip1`), `rustfmt`
(`cargo fmt --all --check`), `docs` (rustdoc with `-D warnings`), and
`fuzz_testing` (installs `cargo-fuzz`, then `cargo check` on the fuzz
package so targets can never rot).

Notably absent: any caching (no `actions/cache`, no sccache) and any merge
queue configuration. Every build is from scratch, trading minutes for
reproducibility. Actions are referenced at three pinning levels:
`actions/checkout@v4` (major tag), `dtolnay/rust-toolchain@master`
(deliberately floating, it is a toolchain installer), and, in the release
workflow, full commit-SHA pinning for the supply-chain-sensitive step:

```yaml
- name: Attest build provenance
  uses: actions/attest-build-provenance@977bb373ede98d70efdf65b84cb5f73e068dcc2a # v3.0.0
```

`extras/ripgrep/.github/workflows/release.yml` triggers only on tags
matching `"[0-9]+.[0-9]+.[0-9]+"` and escalates permissions explicitly
(`contents: write`, `id-token: write`, `attestations: write` for provenance
signing). A `create-release` job verifies the tag equals the manifest
version before anything builds:

```yaml
if ! grep -q "version = \"$VERSION\"" Cargo.toml; then
  echo "version does not match Cargo.toml" >&2
  exit 1
fi
```

then creates a draft release with `gh release create $VERSION --draft
--verify-tag`. The `build-release` job fans out over 14 targets, builds with
`--profile release-lto --features pcre2` and `PCRE2_SYS_STATIC=1`, strips
foreign-arch binaries by running the target's strip tool inside the
`ghcr.io/cross-rs` Docker image, and generates the man page and all four
shell completions by executing the just-built binary, under qemu when the
architecture demands it. Archives get sha256 sums and provenance
attestations before `gh release upload`. A third job builds a `.deb` with
`cargo-deb`, working around its inability to reference build-time assets by
generating the man page and completions into `deployment/deb/` first.

## 7. Testing

The layout is a textbook unit/integration split. Unit tests live inline in
`#[cfg(test)] mod tests` blocks next to the code, including one test per flag
directly under each `impl Flag` in `extras/ripgrep/crates/core/flags/defs.rs`
(`parse_low_raw(["-A5"])` style assertions covering every spelling of every
flag). Integration tests are one binary, mapped in
`extras/ripgrep/tests/tests.rs` with a comment-documented module list:
`binary` (binary file handling), `feature` (1,174 lines, per-feature tests),
`json`, `misc`, `multiline`, and `regression` (1,744 lines of tests named
after issue numbers), plus infrastructure modules `macros`, `hay` (a shared
Sherlock Holmes corpus), and `util`.

The harness in `extras/ripgrep/tests/util.rs` is the pattern worth stealing:
`setup(test_name)` returns a `(Dir, TestCommand)` pair, where `Dir` creates
an isolated scratch directory using a global `AtomicUsize` counter and
`TestCommand` wraps `std::process::Command` pointed at the compiled `rg`
with its working directory set to the scratch dir. Every end-to-end test
therefore runs the real user-facing binary. The `rgtest!` macro in
`extras/ripgrep/tests/macros.rs` then doubles coverage for free:

```rust
macro_rules! rgtest {
    ($name:ident, $fun:expr) => {
        #[test]
        fn $name() {
            let (dir, cmd) = crate::util::setup(stringify!($name));
            $fun(dir, cmd);

            if cfg!(feature = "pcre2") {
                let (dir, cmd) = crate::util::setup_pcre2(stringify!($name));
                $fun(dir, cmd);
            }
        }
    };
}
```

Each of the 334 `rgtest!` invocations runs once per regex engine. A
companion `eqnice!` macro prints expected and actual output between tilde
rulers, a hand-rolled substitute for snapshot tooling that keeps failures
readable without any dev-dependency.

Other layers:

- Library-internal harness: `extras/ripgrep/crates/searcher/src/testutil.rs`
  provides a `RegexMatcher` whose line-terminator optimization can be forced
  on or off, so searcher tests exercise both fast and slow paths on the same
  inputs.
- Data-driven fixtures: `extras/ripgrep/crates/ignore/tests/` pairs test
  files with real `.gitignore` fixtures such as
  `gitignore_matched_path_or_any_parents_tests.gitignore`.
- Fuzzing with property assertions: `extras/ripgrep/fuzz/fuzz_targets/fuzz_glob.rs`
  is a libFuzzer target that asserts round-trip invariants
  (`Glob::new` equals `Glob::from_str`; `glob.glob()` reproduces the input),
  enabled by an optional `arbitrary` feature with derive support declared in
  `extras/ripgrep/crates/globset/Cargo.toml`. CI compiles the fuzz targets on
  every run so they cannot bit-rot.
- Benchmarks at two levels: micro-benchmarks in
  `extras/ripgrep/crates/globset/benches/bench.rs`, and a full comparative
  macro-benchmark suite, `extras/ripgrep/benchsuite/benchsuite` (a Python 3
  runner over multi-gigabyte subtitle corpora), with historical results
  committed under `extras/ripgrep/benchsuite/runs/` going back to 2016.
- The public CLI surface is additionally cross-checked by
  `extras/ripgrep/ci/test-complete`, which diffs flags parsed out of
  `rg --help` against the zsh completion spec.

## 8. Error handling and API design

The split is disciplined: the binary uses `anyhow` (declared in
`extras/ripgrep/Cargo.toml`), while every library defines a hand-written
error type; neither `thiserror` nor any other derive-based error crate
appears anywhere. `extras/ripgrep/crates/globset/src/lib.rs` has a `struct
Error` wrapping a `pub enum ErrorKind` whose variants document themselves,
including deprecated variants kept for compatibility with an explanation:

```rust
/// **DEPRECATED**.
///
/// This error used to occur for consistency with git's glob specification,
/// but the specification now accepts all uses of `**`. ...
InvalidRecursive,
```

`extras/ripgrep/crates/regex/src/error.rs` follows the same kind-wrapped
shape with private constructors (`Error::regex`, `Error::generic`) that
translate `regex_automata` errors into domain terms.

Exit-code discipline is exemplary. `extras/ripgrep/crates/core/main.rs`
declares `fn main() -> ExitCode`, maps a broken pipe found anywhere in the
`anyhow` chain to exit 0 (matching Unix convention, with a comment explaining
why Rust programs must do this manually), prints other errors as `{:#}` and
returns 2. The 0/1/2 convention (match/no match/error) is computed in `run`
from the search result combined with a global errored flag.
`extras/ripgrep/crates/core/messages.rs` implements the non-fatal error
policy: per-file errors print a message and flip a `static ERRORED:
AtomicBool`, the search continues, and the final exit status consults the
flag. Error printing itself is careful about interleaving, via a macro that
locks stdout before writing to stderr (quoted in section 9).

API-design patterns visible across the crates:

- Builders everywhere, all `&self`-consuming-config style:
  `SearcherBuilder::build` in
  `extras/ripgrep/crates/searcher/src/searcher/mod.rs`,
  `StandardBuilder::build<W: WriteColor>` in
  `extras/ripgrep/crates/printer/src/standard.rs`, `WalkBuilder` in
  `extras/ripgrep/crates/ignore/src/walk.rs`.
- Newtypes over private enums to keep representation changeable:
  `pub struct MmapChoice(MmapChoiceImpl)` in
  `extras/ripgrep/crates/searcher/src/searcher/mmap.rs`,
  `pub struct LineTerminator(LineTerminatorImp)` and
  `pub struct ByteSet(BitSet)` in `extras/ripgrep/crates/matcher/src/lib.rs`.
- Invariant-carrying newtype: `Match` in the matcher crate is a `Copy` range
  that asserts `start <= end` at construction and implements slice indexing.
- Error plumbing as a trait: `SinkError` in
  `extras/ripgrep/crates/searcher/src/sink.rs` defines constructor hooks
  (`error_message`, `error_io`, `error_config`) so `std::io::Error` works
  out of the box while custom error types remain possible; the matcher crate
  offers `pub struct NoError(())` for infallible matchers.
- A tri-state parse result instead of overloading `Result`:
  `enum ParseResult<T> { Special(SpecialMode), Ok(T), Err(anyhow::Error) }`
  in `extras/ripgrep/crates/core/flags/parse.rs`, letting `-h/-V` short
  circuit before config files are even read.
- Visibility is tight: the whole flag system is `pub(crate)`, struct fields
  are private with accessors, and shared config is wrapped as
  `pub struct HyperlinkConfig(Arc<HyperlinkConfigInner>)` in
  `extras/ripgrep/crates/printer/src/hyperlink/mod.rs`.
- Panic policy: panics mark programmer errors only (`Match::new` documents
  its panic), user-facing failures are `Result`s, and shipped binaries build
  with `panic = "abort"` via the `release-lto` profile.

## 9. Deep Rust usage: ten-plus cited idioms

1. Trait-object plugin registry for flags. Every CLI flag is a unit struct
   implementing the `Flag` trait, collected into one global
   `&[&dyn Flag]` slice. The trait bound list in
   `extras/ripgrep/crates/core/flags/mod.rs` is itself instructive:

   ```rust
   trait Flag: Debug + Send + Sync + UnwindSafe + RefUnwindSafe + 'static {
   ```

   One implementation carries the parser behavior, the short/long names, the
   `-h` text, the `--help` text, and the roff man-page text
   (`doc_short`/`doc_long` in `extras/ripgrep/crates/core/flags/defs.rs`),
   so help, man page, and completions can never disagree with the parser.

2. Internal iteration as a deliberate trait-design choice. The matcher crate
   documents why it uses the push model, in
   `extras/ripgrep/crates/matcher/src/lib.rs`:

   ```text
   A key design decision made in this crate is the use of *internal
   iteration*, or otherwise known as the "push" model of searching.
   ```

   with two stated reasons: some engines cannot expose external iterators,
   and Rust's type system makes a generic pull-model interface cost either
   ergonomics or performance.

3. Callback trait with associated error type and default methods. `Sink` in
   `extras/ripgrep/crates/searcher/src/sink.rs` requires only `matched`;
   `context`, `context_break`, `begin`, and `finish` have default bodies,
   and returning `Ok(false)` anywhere stops the search, which is how
   `--max-count` style limits compose without special cases.

4. Zero-copy path printing with `Cow` and platform `cfg` on a struct field.
   `extras/ripgrep/crates/printer/src/util.rs`:

   ```rust
   pub(crate) struct PrinterPath<'a> {
       #[cfg(not(unix))]
       path: &'a Path,
       bytes: Cow<'a, [u8]>,
       hyperlink: OnceCell<Option<HyperlinkPath>>,
   }
   ```

   On Unix the borrowed bytes are the path, so nothing allocates; only
   Windows pays for UTF-8 conversion, and the hyperlink form is computed
   lazily via `OnceCell` interior mutability.

5. `Cow` in an algorithmic hot path. The did-you-mean flag suggester in
   `extras/ripgrep/crates/core/flags/parse.rs` builds 3-gram bags as
   `BTreeSet<Cow<'a, [u8]>>`: real windows borrow
   (`slice.windows(3).map(Cow::Borrowed)`), while short names get padded
   owned grams, then a Jaccard index ranks candidates.

6. Work-stealing parallelism from `crossbeam-deque`, not a thread pool
   crate. `extras/ripgrep/crates/ignore/src/walk.rs` builds
   `WalkParallel` on `Stealer`/`Worker` deques
   (`stealers: Arc<[Stealer<Message>]>` at line 1655) and exposes
   backpressure through a control-flow enum, `pub enum WalkState`
   (`Continue`, `Skip`, `Quit`), returned by user visitors.

7. Modern std lazies instead of `lazy_static`/`once_cell` dependencies:
   `static RE: OnceLock<Regex>` in
   `extras/ripgrep/crates/ignore/src/gitignore.rs`,
   `static P: OnceLock<Parser>` in
   `extras/ripgrep/crates/core/flags/parse.rs`, and
   `static DOC: LazyLock<String>` for a computed doc string in
   `extras/ripgrep/crates/core/flags/defs.rs`. Global state that must be
   mutable is confined to three `AtomicBool`s in
   `extras/ripgrep/crates/core/messages.rs`.

8. Unsafe as a priced-in API contract. The entire tree has five `unsafe`
   sites: two `libc` calls in `extras/ripgrep/crates/cli/src/hostname.rs`,
   two in the mmap module, and one call site in
   `extras/ripgrep/crates/core/flags/hiargs.rs`. The interesting one is that
   `MmapChoice::auto()` in
   `extras/ripgrep/crates/searcher/src/searcher/mmap.rs` is an `unsafe fn`
   whose safety comment admits the contract is environmental:

   ```text
   This constructor is not safe because there is no obvious way to
   encapsulate the safety of file backed memory maps on all platforms
   without simultaneously negating some or all of their benefits.
   ```

   The binary accepts that risk exactly once, in `hiargs.rs` line 242.

9. Feature stubs via `#[path]` module swapping. Instead of scattering
   `#[cfg(feature = ...)]` through call sites,
   `extras/ripgrep/crates/core/index/mod.rs` selects a whole module body:

   ```rust
   #[cfg(not(feature = "unstable-index"))]
   #[path = "disabled.rs"]
   mod imp;
   #[cfg(feature = "unstable-index")]
   #[path = "enabled.rs"]
   mod imp;
   ```

   `disabled.rs` is nine lines of `anyhow::bail!` stubs with identical
   signatures, so `main.rs` compiles unconditionally.

10. Macros only where functions cannot go. `eprintln_locked!` in
    `extras/ripgrep/crates/core/messages.rs` locks stdout before writing to
    stderr, an intentional abstraction violation with the reasoning inline:

    ```rust
    // This is a bit of an abstraction violation because we explicitly
    // lock stdout before printing to stderr. This avoids interleaving
    // lines within ripgrep because `search_parallel` uses `termcolor`,
    // which accesses the same stdout lock when writing lines.
    ```

    The other macros (`message!`, `err_message!`, `rgtest!`, `eqnice!`) are
    equally small and local; there is no proc-macro anywhere.

11. `let ... else` for early-exit plumbing, used the day it made sense:
    `let Ok(target_os) = std::env::var("CARGO_CFG_TARGET_OS") else { return };`
    in `extras/ripgrep/build.rs`, and
    `let Some(zeropos) = buf.iter().position(|&b| b == 0) else { ... }` when
    defending against POSIX's non-NUL-terminated `gethostname` in
    `extras/ripgrep/crates/cli/src/hostname.rs`.

12. Conditional global allocator with a written cost-benefit analysis.
    `extras/ripgrep/crates/core/main.rs` installs jemalloc only for 64-bit
    musl builds, after a 20-line comment explaining that musl's allocator is
    slow for ripgrep while glibc's is fine and jemalloc bloats compile
    times, a model of documenting a non-obvious `cfg`.

13. Byte-first text handling. `bstr::ByteSlice` is imported across the tree
    (e.g. `extras/ripgrep/crates/searcher/src/testutil.rs`,
    `extras/ripgrep/tests/util.rs`); searching operates on `&[u8]` with an
    amortized rolling buffer (`fn roll` in
    `extras/ripgrep/crates/searcher/src/line_buffer.rs`), and UTF-8 is a
    printer-level concern, not a search-level one.

14. Iterator pipelines at the orchestration layer.
    `extras/ripgrep/crates/core/main.rs` composes the walk as
    `args.walk_builder()?.build().filter_map(|result|
    haystack_builder.build_from_result(result))` before optional sorting,
    keeping the single-threaded path lazy end to end.

## 10. Documentation practices

Documentation is enforced, generated, and layered:

- Enforced: `#![deny(missing_docs)]` in all eight published library crates
  (section 5), plus the CI `docs` job compiling rustdoc for private items
  with warnings denied.
- Module-level `//!` docs open every significant module;
  `extras/ripgrep/crates/core/flags/mod.rs` begins with a full paragraph
  explaining that the module owns flags, completions, `--help`, and the man
  page. Even `main.rs` has one (`/*! The main entry point into ripgrep. */`).
- Generated from one source of truth: help text, the roff man page
  (`extras/ripgrep/crates/core/flags/doc/template.rg.1`, filled by
  `TEMPLATE.replace("!!VERSION!!", ...)` in
  `extras/ripgrep/crates/core/flags/doc/man.rs`), and four shell completion
  scripts all derive from the `Flag` implementations, exposed to users as
  `rg --generate man` and `rg --generate complete-{bash,zsh,fish,powershell}`
  (see `GenerateMode` in `extras/ripgrep/crates/core/flags/lowargs.rs`).
- User-facing books live in the repo as flat Markdown:
  `extras/ripgrep/GUIDE.md` (a full user guide including a documented sample
  config file) and `extras/ripgrep/FAQ.md`. `extras/ripgrep/scripts/copy-examples`
  extracts code blocks from documentation so examples stay compilable.
- Per-crate `README.md` plus `LICENSE-MIT` and `UNLICENSE` in every crate
  directory; `extras/ripgrep/crates/core/README.md` doubles as a short
  architecture note for the binary.
- Issue intake is engineered: `extras/ripgrep/.github/ISSUE_TEMPLATE/bug_report.yml`
  is a structured form that lists the three most common non-bugs with issue
  references and requires a checkbox (`I have a different issue.`) before
  filing; `config.yml` routes questions to GitHub Discussions;
  `feature_request.md` asks requesters to draft the ideal man-page text for
  their feature. `extras/ripgrep/CONTRIBUTING.md` is a short pointer to the
  project's contribution policy document at the repository root.

## 11. Release and distribution

Versioning and cadence are managed by a committed runbook,
`extras/ripgrep/RELEASE-CHECKLIST.md`, which encodes hard-won ordering: run
`cargo update` and `cargo outdated` first; release constituent crates in
dependency order (`globset`, `ignore`, `cli`, `matcher`, `regex`, `pcre2`,
`searcher`, `printer`, `grep`, then core); bump minimal versions in
dependents; push `master` and wait for CI to go green before pushing the
tag, with the reason recorded:

```text
Once CI for `master` finishes successfully, push the version tag. (Trying to
do this in one step seems to result in GitHub Actions not seeing the tag
push and thus not running the release workflow.)
```

Changelog discipline is visible at the top of
`extras/ripgrep/CHANGELOG.md`: a standing `TBD` section
(`Unreleased changes. Release notes have not yet been written.`) followed by
dated releases whose entries are categorized (`Platform support`,
`Performance improvements`, `Feature enhancements`, bug fixes) and each
prefixed with a typed, linked reference such as
`[PERF #3293](https://github.com/BurntSushi/ripgrep/issues/3293)`.

Distribution artifacts, all produced by
`extras/ripgrep/.github/workflows/release.yml`: 14 target archives
containing the stripped binary, licenses, `CHANGELOG`/`FAQ`/`GUIDE`, a
generated man page, and generated completions for four shells; a `.sha256`
sum per archive; a signed build-provenance attestation per archive; and a
Debian package built by `cargo-deb` from `[package.metadata.deb]`. The
Homebrew formula for the prebuilt binary lives in-repo at
`extras/ripgrep/pkg/brew/ripgrep-bin.rb`, reachable through the
`HomebrewFormula` symlink that Homebrew taps expect, and updated each
release by `ci/sha256-releases`. Version metadata is embedded at build time:
`extras/ripgrep/build.rs` exports the short git hash as
`RIPGREP_BUILD_GIT_HASH`, consumed via `option_env!` in
`extras/ripgrep/crates/core/flags/doc/version.rs`, and links
`extras/ripgrep/pkg/windows/Manifest.xml` into MSVC builds to enable
long-path awareness.

## 12. Lessons for quinjet

quinjet already exceeds ripgrep on lint tooling (clippy wall, cargo-deny,
taplo, typos, miri, mutants, coverage floor), so the transferable value is
in test architecture, CLI surface integrity, and release engineering:

1. Collapse integration tests into one binary with a real-process harness.
   Set `autotests = false` and an explicit `[[test]]` in `Cargo.toml` as in
   `extras/ripgrep/Cargo.toml`, then port the `Dir`/`TestCommand` harness
   from `extras/ripgrep/tests/util.rs`: per-test scratch directories from an
   `AtomicUsize`, commands running the compiled `quinjet` binary with cwd
   set inside a throwaway git repository. Every CLI subcommand gets tested
   as a user would run it.
2. Add an `rgtest!`-style macro that runs each end-to-end test under every
   relevant configuration (`extras/ripgrep/tests/macros.rs`). For quinjet
   the axes are natural: with and without a config file, and against
   repositories in different states.
3. Start `tests/regression.rs` now, one test per fixed issue, named after
   the issue number, following `extras/ripgrep/tests/regression.rs`. It is
   the cheapest possible insurance against reintroducing bugs.
4. Adopt the exit-code and broken-pipe discipline of
   `extras/ripgrep/crates/core/main.rs`: `fn main() -> ExitCode`, walk the
   `anyhow` chain for `ErrorKind::BrokenPipe` and exit 0, reserve distinct
   codes for "nothing to do" versus "error", and track non-fatal errors in
   a `static AtomicBool` consulted at exit
   (`extras/ripgrep/crates/core/messages.rs`).
5. Copy the `eprintln_locked!` idea for any code path where the TUI or CLI
   writes to stdout and stderr concurrently: lock stdout before writing
   stderr so lines never interleave on a tty.
6. Write inventory tests over the command surface, modeled on
   `available_shorts` in `extras/ripgrep/crates/core/flags/defs.rs`: assert
   every operation is reachable as both a subcommand and a keybinding, and
   print unassigned keys with `-- --nocapture` in CI.
7. Generate man pages and completions from the binary itself and verify
   them. With clap, wire `clap_mangen` and `clap_complete` behind a
   `quinjet generate man|complete-<shell>` subcommand mirroring ripgrep's
   `--generate` modes, ship the outputs in release archives, and add a CI
   step in the spirit of `extras/ripgrep/ci/test-complete` that diffs
   `--help` flags against the completion output.
8. Harden CI structure: top-level `permissions: contents: read`, a nightly
   `schedule:` cron, `fail-fast: false`, and a matrix with a `pinned` MSRV
   entry plus `beta` and `nightly` rows, exactly as in
   `extras/ripgrep/.github/workflows/ci.yml`. Pin any release-critical
   third-party action by full commit SHA with a version comment, as
   ripgrep pins `actions/attest-build-provenance`.
9. Make rustdoc a gate even for a binary crate: a CI job running
   `cargo doc --no-deps --document-private-items` with
   `RUSTDOCFLAGS: -D warnings`, per the `docs` job in
   `extras/ripgrep/.github/workflows/ci.yml`.
10. Split profiles: keep `[profile.release] debug = 1` for field-debuggable
    builds and add `[profile.release-lto]` with `lto = "fat"`,
    `codegen-units = 1`, `panic = "abort"`, `strip = "symbols"` used only
    by the release workflow, per `extras/ripgrep/Cargo.toml`.
11. Build a tag-triggered release workflow that verifies the tag against
    `Cargo.toml` before building, creates a draft release with
    `gh release create --draft --verify-tag`, attaches `.sha256` sums, and
    signs artifacts with `actions/attest-build-provenance`, following
    `extras/ripgrep/.github/workflows/release.yml`. Add
    `extras/ripgrep/.cargo/config.toml`-style `crt-static` rustflags if
    shipping musl or Windows binaries.
12. Add a `fuzz/` package (excluded from the workspace, `publish = false`)
    with `cargo-fuzz` targets for every parser quinjet owns (git porcelain
    output, config files), asserting round-trip properties inside the
    target as `extras/ripgrep/fuzz/fuzz_targets/fuzz_glob.rs` does, and a
    CI job that at minimum `cargo check`s the fuzz package.
13. Embed the short git hash via a `build.rs` `rustc-env` and surface it in
    `--version` through `option_env!`, per `extras/ripgrep/build.rs` and
    `extras/ripgrep/crates/core/flags/doc/version.rs`.
14. Commit a `RELEASE-CHECKLIST.md` and keep a standing `TBD` changelog
    section with typed, issue-linked entries, per
    `extras/ripgrep/RELEASE-CHECKLIST.md` and `extras/ripgrep/CHANGELOG.md`.
15. Use structured issue forms with a triage checkbox and links to known
    non-bugs (`extras/ripgrep/.github/ISSUE_TEMPLATE/bug_report.yml`), and
    route questions to Discussions via `config.yml`.
16. When quinjet grows an experimental subsystem, gate it the ripgrep way:
    an `unstable-*` feature flag plus the `#[path]` module-swap stub pattern
    from `extras/ripgrep/crates/core/index/mod.rs`, so mainline code never
    branches on the feature at call sites.
