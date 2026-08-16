# gitui-org/gitui (22396 stars)

## 1. What the project is and how big it is

gitui is a keyboard-driven terminal user interface for Git, self-described in extras/gitui/Cargo.toml as a "blazing fast terminal-ui for git". It is one of the most widely installed Rust TUIs in industry: the README installation section at extras/gitui/README.md lists packaged builds for Fedora (`dnf`), openSUSE (`zypper`), Homebrew, MacPorts, winget, scoop, chocolatey, FreeBSD `pkg`, and conda-forge, which is a good proxy for real-world adoption. Engineers reach for it because it wraps libgit2 and gitoxide behind an async job system so that even huge repositories (the Makefile keeps commented run targets against the Linux and Kubernetes trees in extras/gitui/Makefile) stay responsive.

Measured from the clone at commit `2fa693c`:

* 162 Rust source files, 50,372 lines of Rust across the repository.
* 7 crates: the root binary plus 6 library crates. extras/gitui/Cargo.toml declares five workspace members explicitly, and `invalidstring` joins the workspace as a path dependency of asyncgit:

```toml
[workspace]
members = [
  "asyncgit",
  "filetreelist",
  "git2-hooks",
  "git2-testing",
  "scopetime",
]
```

* Line counts per crate: `src` (the binary) 29,418; `asyncgit` 17,145; `filetreelist` 1,953; `git2-hooks` 1,631; `git2-testing` 97; `scopetime` 68; `invalidstring` 11.
* 318 `#[test]` functions across the workspace (177 in asyncgit, 79 in the binary crate, 62 in filetreelist and git2-hooks combined).

## 2. Repository layout

```text
extras/gitui/
|-- Cargo.toml            root package + workspace definition
|-- build.rs              embeds git hash and build date into the version string
|-- src/                  the gitui binary: app loop, UI, input
|   |-- components/       reusable widgets (diff, commitlist, textinput, ...)
|   |-- popups/           31 modal dialogs (commit, push, fetch, blame, ...)
|   |-- tabs/             the five main screens (status, revlog, files, ...)
|   |-- ui/               low-level drawing helpers (scrollbar, reflow, style)
|   |-- keys/             keybinding model, RON override loading
|   `-- snapshots/        insta .snap files for full-terminal snapshots
|-- asyncgit/             all Git logic; sync/ has blocking ops, the rest wraps
|   |                     them in background jobs over crossbeam channels
|-- filetreelist/         pure data structure crate: foldable sorted path tree
|-- git2-hooks/           git hook discovery/execution on top of git2-rs
|-- git2-testing/         test helper crate: temp-repo constructors
|-- invalidstring/        one function producing invalid UTF-8 test data
|-- scopetime/            feature-gated scope timing macro
|-- .github/workflows/    ci.yml, cd.yml, nightly.yml, brew.yml
|-- wix/                  Windows MSI packaging sources
|-- deny.toml, typos.toml, tombi.toml, rustfmt.toml, .clippy.toml
`-- CHANGELOG.md, KEY_CONFIG.md, THEMES.md, FAQ.md, NIGHTLIES.md
```

The split works because each extracted crate has a single reason to exist and a strictly smaller dependency set than the binary. `asyncgit` is the whole Git domain layer and never touches ratatui; the UI crate never links git2 directly. `filetreelist` is pure logic with only `thiserror` as a dependency (extras/gitui/filetreelist/Cargo.toml), so its folding and navigation algorithms are testable without a repository. Test-only concerns get their own crates (`git2-testing`, `invalidstring`) so production crates never carry test scaffolding, and dev-only helpers are wired in via `[dev-dependencies]` (extras/gitui/asyncgit/Cargo.toml). The crate boundary is also the documentation boundary: extras/gitui/src/main.rs opens with a module map explaining exactly this layering.

## 3. Cargo manifest practices

The root manifest at extras/gitui/Cargo.toml is both the binary package and the workspace root. Notable practices:

* MSRV and edition are pinned in the package table: `edition = "2021"` and `rust-version = "1.88"`. The same MSRV appears in `.clippy.toml` and as an explicit row in the CI matrix, so the claim is enforced three ways.
* There is no `[workspace.package]` inheritance and no `[workspace.dependencies]`: every member repeats `authors`, `edition`, `license`, `homepage`, `repository` (see extras/gitui/asyncgit/Cargo.toml and extras/gitui/filetreelist/Cargo.toml). Each member is independently published to crates.io, which is why each carries full metadata, `categories`, and `keywords`. Path dependencies always pair a `path` with a `version` so publishing works: `asyncgit = { path = "./asyncgit", version = "0.28.1", default-features = false }`. One unusual bound: `filetreelist = { path = "./filetreelist", version = ">=0.6" }`.
* Crates-io hygiene: `exclude = [".github/*", ".vscode/*", "assets/*"]` keeps the published tarball small, and filetreelist excludes its demo gif (`exclude = ["/demo.gif"]`).
* Feature flags document their constraints inline:

```toml
[features]
default = ["ghemoji", "regex-fancy", "trace-libgit", "vendor-openssl"]
ghemoji = ["gh-emoji"]
# regex-* features are mutually exclusive.
regex-fancy = ["syntect/regex-fancy", "two-face/syntect-fancy"]
regex-onig = ["syntect/regex-onig", "two-face/syntect-onig"]
timing = ["scopetime/enabled"]
```

  Features are thin: each one either renames an optional dependency (`ghemoji = ["gh-emoji"]`) or forwards to a member crate (`vendor-openssl = ["asyncgit/vendor-openssl"]`, which in extras/gitui/asyncgit/Cargo.toml enables an optional `openssl-sys` with `features = ["vendored"]`).

* Profiles are tuned for the product. Debug builds keep the UI fast by optimizing only the hot dependency, and release optimizes for binary size:

```toml
[profile.dev.package."ratatui"]
opt-level = 3

[profile.release]
opt-level = "z"  # Optimize for size.
strip = "debuginfo"
lto = true
codegen-units = 1
```

* Dependencies are alphabetized, and `default-features = false` is applied aggressively (`chrono`, `ratatui`, `syntect`, `simplelog`, `bytesize`, `two-face`) to keep the dependency tree and binary small.
* There is no `[lints]` table anywhere; lint policy lives in crate-level attributes (section 5).
* extras/gitui/rust-toolchain.toml pins only `channel = "stable"` with `profile = "default"`, so contributors build on current stable while CI separately guards the MSRV.
* extras/gitui/.cargo/config.toml maps cross linkers per target, e.g. `[target.aarch64-unknown-linux-gnu] linker = "aarch64-linux-gnu-gcc"`.

## 4. Formatting

extras/gitui/rustfmt.toml is three lines, all stable options:

```toml
max_width = 70
hard_tabs = true
newline_style = "Unix"
```

* `max_width = 70`: far below the default 100. The codebase is designed to be read in narrow terminal splits next to the running TUI; it also forces short expressions and early extraction of locals.
* `hard_tabs = true`: indentation is tab characters, so each reader chooses their own indent width. This is paired with the editor layer: extras/gitui/.editorconfig declares `root = true` and, for `[*.rs]`, `indent_style = tab`, so non-rustfmt editors agree with rustfmt.
* `newline_style = "Unix"`: normalizes line endings across the Windows contributors the project demonstrably has (there is a full Windows CI leg and MSI packaging).

Non-Rust formatting is also enforced. TOML files are formatted with tombi, configured at extras/gitui/tombi.toml with an MSRV-driven constraint, explained in place:

```toml
# Keep dependency inline tables on a single line. Multi-line inline tables are
# TOML 1.1 syntax that Cargo on our MSRV (rust 1.88) rejects with
# "invalid inline table", so tombi must not expand them.
[format.rules]
line-width = 220
```

CI runs `tombi format --check` in the linting job (extras/gitui/.github/workflows/ci.yml) and the Makefile aliases it as `make sort`. Spelling is checked by typos with extras/gitui/typos.toml, which whitelists project words (`ratatui = "ratatui"`) and excludes the changelog via `extend-exclude = ["CHANGELOG.md"]`. Editor auto-format is switched on for contributors in extras/gitui/.vscode/settings.json (`"editor.formatOnSave": true`).

## 5. Linting

Clippy configuration lives in two places: a tiny `.clippy.toml` and per-crate attribute walls.

extras/gitui/.clippy.toml:

```toml
msrv = "1.88.0"
cognitive-complexity-threshold = 18
```

Setting `msrv` stops clippy from suggesting APIs newer than the supported compiler; lowering `cognitive-complexity-threshold` below the default 25 makes the nursery complexity lint bite earlier.

The binary crate wall at extras/gitui/src/main.rs:

```rust
#![forbid(unsafe_code)]
#![deny(
    mismatched_lifetime_syntaxes,
    unused_imports,
    unused_must_use,
    dead_code,
    unstable_name_collisions,
    unused_assignments
)]
#![deny(clippy::all, clippy::perf, clippy::nursery, clippy::pedantic)]
#![deny(
    clippy::unwrap_used,
    clippy::filetype_is_file,
    clippy::cargo,
    clippy::panic,
    clippy::match_like_matches_macro
)]
```

The philosophy is visible in what is denied versus allowed. Denied: whole groups (`all`, `perf`, `nursery`, `pedantic`, `cargo`) plus the crash-preventing restriction lints `unwrap_used` and `panic`. Allowed: a short, justified list (`module_name_repetitions`, `multiple_crate_versions`, `bool_to_int_with_if`, and two false-positive-prone lints). Aspirations are recorded as commented deny lines, e.g. in extras/gitui/asyncgit/src/lib.rs:

```rust
    //TODO: get this in someday since expect still leads us to crashes sometimes
    // clippy::expect_used
```

Each crate tunes its own wall: asyncgit adds `#![forbid(missing_docs)]` and `deprecated`, and allows `missing_errors_doc` and `must_use_candidate` (a library-appropriate relaxation), while stricter sub-modules escalate locally: extras/gitui/asyncgit/src/asyncjob/mod.rs opens with `#![deny(clippy::expect_used)]`, proving the aspirational lint one module at a time. `git2-testing` allows `unsafe_code` only via a function-scoped `#[allow(unsafe_code)]` instead of dropping the crate-wide guarantee.

Beyond clippy, the check infrastructure is aggregated in extras/gitui/Makefile:

```make
check: fmt clippy test sort deny
```

with `deny` running `cargo deny check` against extras/gitui/deny.toml. That file is a model of documented exceptions: a license allowlist of ten SPDX ids, one advisory ignore with a linked reason (`{ id = "RUSTSEC-2025-0141", reason = "Only brought in via syntect" }`), and `multiple-versions = "deny"` under `[bans]` where every `skip-tree` entry carries a comment naming the offending dependency and, where available, the upstream issue link. CI adds `cargo udeps` (unused dependency detection) as a separate nightly-toolchain job.

## 6. CI/CD

Four workflows live in extras/gitui/.github/workflows.

`ci.yml` triggers on a nightly cron (`"0 2 * * *"`), on push to every branch (`branches: ["*"]`), and on pull requests to master. Its jobs:

* `build`: a 3x3 matrix, `os: [ubuntu-latest, macos-latest, windows-latest]` by `rust: [nightly, stable, "1.88"]`, with `fail-fast: false` and `continue-on-error: ${{ matrix.rust == 'nightly' }}`. Pinning the literal MSRV as a matrix row means an accidental use of a newer API fails CI, while the nightly row is an early-warning canary that cannot block merges. Steps: `Swatinem/rust-cache@v2` with a `shared-key` composed of os, cache name, and toolchain; `dtolnay/rust-toolchain@master` with the matrix toolchain; nextest installed via `taiki-e/install-action@nextest`; debug build, `make test`, `make clippy`, `make build-release`; then `cargo install --path "." --force --locked` as a packaging smoke test; binary size listing per OS; `otool -L` on macOS to audit dynamic library linkage; and `cargo wix` on Windows to prove the MSI still builds. It even installs signing tools (`gpgsm`, gnupg) because the test suite includes real end-to-end commit-signing tests.
* `build-linux-musl`: same three toolchains against `x86_64-unknown-linux-musl`, running the full suite with `make test-linux-musl` and checking `--version` output of both debug and release binaries.
* `build-linux-arm`: cross-compiles aarch64, armv7, and arm targets with vendor GCC toolchains, then actually executes the aarch64 test binaries under emulation by exporting `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER: qemu-aarch64-static -L /usr/aarch64-linux-gnu`. Very few projects run their tests on a foreign architecture in CI.
* `build-apple-x86`: cross-builds the Intel macOS binary from Apple Silicon runners.
* `linting`: `cargo fmt -- --check`, `tombi format --check`, and `cargo deny check`.
* `udeps`: nightly toolchain plus `cargo +nightly udeps --all-targets`.
* `log-test` ("Changelog Test"): runs `ffurrer2/extract-release-notes@v2` on every PR and uploads the result, guaranteeing the changelog stays machine-extractable before release day.
* `test-homebrew`: `brew install --build-from-source gitui` on macOS, verifying the downstream formula still builds from source.

Actions are pinned by major tag (`actions/checkout@v4`, `Swatinem/rust-cache@v2`, `softprops/action-gh-release@v2`), not by SHA. `cd.yml` triggers on tag push and `workflow_dispatch`, and is the only workflow that requests write permission, minimally scoped:

```yaml
permissions:
  contents: write
```

The release job re-runs tests and clippy per OS, builds all release artifacts via Makefile targets (`release-mac`, `release-mac-x86`, `release-linux-musl`, `release-win`, `release-linux-arm`), computes a SHA256 for the mac tarball, extracts the release body from CHANGELOG.md with the same extract-release-notes action CI validated, publishes with `softprops/action-gh-release@v2` using `prerelease: ${{ contains(github.ref, '-') }}`, and finally bumps the homebrew-core formula through `mislav/bump-homebrew-formula-action@v3`, skipping prereleases. `nightly.yml` rebuilds all artifacts on a 3 a.m. cron and uploads them to an S3 bucket (`AWS_BUCKET_NAME: s3://gitui/nightly/`), documented for users in extras/gitui/NIGHTLIES.md. `brew.yml` is a manual re-run of the formula bump with a `tag-name` input for when the automatic bump fails.

Repo automation beyond workflows: extras/gitui/.github/dependabot.yml runs cargo updates daily and groups them (`cargo-minor` and `cargo-patch` groups with `patterns: ["*"]`), collapsing dependency noise into two rolling PRs; extras/gitui/.github/stale.yml marks issues `dormant` after 180 days with `pinned`, `security`, and `nostale` exemptions. No merge-queue configuration is present in the repository; branch protection is configured server-side and not visible from the clone.

## 7. Testing

There are no `tests/` directories anywhere in the workspace; all 318 tests are colocated `#[cfg(test)] mod tests` blocks inside the modules they cover (49 files contain a `mod tests`). The split by crate mirrors the architecture: the Git domain logic in asyncgit carries the bulk (177 tests, e.g. staging, rebase, hooks, signing under extras/gitui/asyncgit/src/sync/), pure data structures in filetreelist and hook logic in git2-hooks carry 62, and the binary crate has 79 including full-application tests.

The harness infrastructure is layered:

* `git2-testing` (extras/gitui/git2-testing/src/lib.rs) provides `repo_init_empty`, `repo_init`, `repo_init_bare`, and `repo_init_suffix`, each returning `(TempDir, Repository)` with committer identity preconfigured. Crucially it also sandboxes global Git state so developer machines cannot influence tests:

```rust
    // Adapted from https://github.com/rust-lang/cargo/pull/9035
    INIT.call_once(|| unsafe {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        set_search_path(ConfigLevel::System, path).unwrap();
        set_search_path(ConfigLevel::Global, path).unwrap();
```

* `invalidstring` (extras/gitui/invalidstring/src/lib.rs) manufactures invalid UTF-8 strings so path and message handling is tested against hostile data.
* Snapshot testing: the whole application is driven headlessly in extras/gitui/src/gitui.rs using ratatui's `TestBackend`, and terminal frames are asserted with insta. A local macro normalizes nondeterminism before comparison:

```rust
    macro_rules! apply_common_filters {
        {} => {
            let mut settings = insta::Settings::clone_current();
```

  with filters that rewrite temp directories to `[TEMP_FILE]` and 7-char commit ids after a box-drawing bar to `[AAAAA]`. The `gitui_starts` test boots the app on a fresh repo, snapshots the loading frame, injects an `AsyncGitNotification::Status`, snapshots again, then synthesizes key events to switch tabs and snapshots the log view. Snapshots live at extras/gitui/src/snapshots/*.snap. The dev-dependency enables filters explicitly: `insta = { version = "1.41.0", features = ["filters"] }`.

* End-to-end external-tool tests: extras/gitui/asyncgit/src/sync/sign.rs contains `test_x509_sign_and_verify_e2e`, which shells out to real `openssl` and `gpgsm` to build a throwaway X.509 identity, signs an actual commit, and verifies it. It is `#[cfg(unix)]` and `#[serial]` (via `serial_test`) because it mutates the process-wide `GNUPGHOME`. CI installs `gpgsm` on Linux and gnupg on macOS specifically for these tests.
* Runner: `cargo nextest run --workspace` is the canonical test command (extras/gitui/Makefile `test:` target), with per-target variants `test-linux-musl` and `test-linux-arm`; the ARM variant demonstrates nextest's filter expressions by excluding one kernel-behavior-dependent test: `-E 'not test(test_hook_with_missing_shebang)'`.
* `pretty_assertions` is a dev-dependency of every crate with tests, and `env_logger` is initialized per test via `git2-testing::init_log` with `is_test(true)`.

There is no fuzzing, property testing, or `benches/` directory in the repository; performance work is done ad hoc through `make profile` (`cargo flamegraph --features timing`) and the scopetime instrumentation instead. The public surface (a flag-only CLI) is covered indirectly: CI executes the built binary (`./target/.../gitui --version`) on musl and validates `cargo install` on every OS.

Two smaller habits round out the harness. First, the application object is factored so tests can drive it: `Gitui` in extras/gitui/src/gitui.rs exposes `input_event`, `update_async`, and `wait_for_async_git_notification` methods that the snapshot test calls directly, meaning the production event loop and the test share the same code path rather than a parallel test-only harness. Second, snapshot temp directories are made recognizable on purpose: `repo_init_suffix(Some("-insta"))` (extras/gitui/git2-testing/src/lib.rs) creates temp dirs ending in `-insta` precisely so the insta filter regexes can find and redact them on all three operating systems.

## 8. Error handling and API design

The repository uses the classic two-tier scheme: `thiserror` in libraries, `anyhow` in the binary.

* extras/gitui/asyncgit/src/error.rs defines a single crate-wide `Error` enum with `#[from]` conversions for io, git2, UTF-8, integer conversion, threadpool, hooks, and signing errors, plus domain variants with user-readable messages such as `#[error("git: no head found")] NoHead` and `#[error("git: uncommitted changes")] UncommittedChanges`. A nested `GixError` enum isolates the highly granular gitoxide error types, and large variants are boxed to keep the enum small: `Discover(#[from] Box<gix::discover::Error>)`.
* The signing module keeps two purpose-specific error enums (`SignBuilderError`, `SignError`) next to the `Sign` trait in extras/gitui/asyncgit/src/sync/sign.rs, so construction failures and runtime failures are distinct types.
* The binary crate uses `anyhow::Result` end to end; `main` itself returns `Result<()>` (extras/gitui/src/main.rs) and context is attached at the boundary, e.g. in extras/gitui/src/args.rs: `fs::create_dir_all(&confpath).with_context(|| format!("failed to create config directory: {}", confpath.display()))?`.
* Panic policy is enforced by lints (`clippy::unwrap_used` and `clippy::panic` denied in both main crates) and mitigated at runtime: a custom hook restores the terminal before printing, so a crash never leaves the user's shell in raw mode (extras/gitui/src/main.rs):

```rust
    panic::set_hook(Box::new(|e| {
        let backtrace = Backtrace::new();
        shutdown_terminal();
```

  Normal shutdown uses `scopeguard::defer! { shutdown_terminal(); }` immediately after terminal setup. The only explicit exit code is `std::process::exit(0)` after `--bugreport` output in extras/gitui/src/args.rs; all other paths exit through `Result`.

* Builder pattern: `SignBuilder::from_gitconfig` returns a `Box<dyn Sign>` selected from git config, with `impl Sign for GPGSign` and `impl Sign for SSHSign` as concrete strategies.
* Newtypes: `CommitId(Oid)` in extras/gitui/asyncgit/src/sync/commits_info.rs wraps the raw git2 object id and centralizes every conversion (`From<Oid>`, `From<gix::ObjectId>`, `From<gix::Commit<'_>>`, `Display`), letting the UI crate stay ignorant of both Git backends. `RepoPath` in extras/gitui/asyncgit/src/sync/repository.rs is an enum newtype distinguishing a plain path from a separated gitdir/workdir pair, with `From<PathBuf>` and `From<&str>` for ergonomic construction.
* Visibility discipline: asyncgit's `lib.rs` re-exports a curated `sync` API while keeping job modules (`fetch_job`, `filter_commits`) private; `#![forbid(missing_docs)]` forces every public item to be documented or hidden.

## 9. Deep Rust usage: ten-plus cited idioms

1. Trait-object component architecture. extras/gitui/src/components/mod.rs defines `Component` (event handling, command reporting) and `DrawableComponent` (rendering), and generic pumps that fan events through `&mut [&mut dyn Component]`: `event_pump` returns `Result<EventState>` and stops at the first consumer, while `command_pump` respects `CommandBlocking::Blocking` to scope the help bar to the focused component.
2. Declarative macros to keep component lists exhaustive. The `accessors!` macro in the same file generates matched `components()` and `components_mut()` vectors from one identifier list, and `setup_popups!` composes `any_popup_visible!` and `draw_popups!`, so adding a popup in one place updates visibility checks, drawing, and event routing together.
3. Generic async job abstraction with associated types. extras/gitui/asyncgit/src/asyncjob/mod.rs:

   ```rust
   pub trait AsyncJob: Send + Sync + Clone {
    /// defines what notification type is used to communicate outside
    type Notification: Copy + Send;
    /// type of progress
    type Progress: Clone + Default + Send + Sync + PartialEq;
   ```

   `AsyncSingleJob<J: AsyncJob>` implements a one-slot queue that keeps overwriting `next` until the worker takes it, which is exactly the right semantics for a UI that only cares about the latest requested diff.
4. Channel multiplexing with `crossbeam_channel::Select`. `select_event` in extras/gitui/src/main.rs registers six receivers (input, git notifications, app notifications, ticker, watcher, spinner) and maps the ready operation index into a `QueueEvent`, giving a single-threaded event loop over many producers without async runtimes.
5. A purpose-built synchronization primitive. extras/gitui/src/notify_mutex.rs defines `NotifiableMutex<T>` combining `Arc<(Mutex<T>, Condvar)>` with `wait(condition)` and `set_and_notify(value)`, used to park the input thread cheaply while the UI is suspended.
6. Bitflags as render dirty-flags. extras/gitui/src/queue.rs declares `NeedsUpdate` with `bitflags!` (`ALL`, `DIFF`, `COMMANDS`, ...) so internal events can request the minimal redraw work.
7. Zero-copy with `Cow`. extras/gitui/src/strings.rs:

   ```rust
   pub fn ellipsis_trim_start(s: &str, width: usize) -> Cow<'_, str> {
    if s.width() <= width {
        Cow::Borrowed(s)
    } else {
   ```

   The common case borrows; only over-wide strings allocate. `Vec<Cow<'a, str>>` also backs wrapped commit messages in extras/gitui/src/components/commit_details/details.rs.
8. Hand-written lifetime-carrying iterators. extras/gitui/filetreelist/src/treeitems_iter.rs implements `Iterator for TreeItemsIterator<'a>` with `type Item = (usize, &'a FileTreeItem)`, yielding only visible items of a folded tree without allocating, and extras/gitui/asyncgit/src/sync/sign.rs shows idiomatic pipeline style (`lines().filter_map(|line| line.strip_prefix("fpr:")).find_map(...)`) for parsing gpgsm output.
9. Deliberate interior-mutability split. The single-threaded UI uses `Rc<RefCell<Options>>` and `cmdbar: RefCell<CommandBar>` (extras/gitui/src/app.rs) plus the alias `pub type RepoPathRef = RefCell<RepoPath>` (extras/gitui/asyncgit/src/sync/repository.rs), while everything crossing the threadpool boundary in asyncjob uses `Arc<Mutex<...>>` and `Arc<RwLock<Progress>>`. Cheap where possible, synchronized only where required.
10. Unsafe policy: forbid by default, allow surgically. `#![forbid(unsafe_code)]` guards the binary, scopetime, and git2-hooks; the only two unsafe sites in the whole workspace are the git2 `set_search_path` sandboxing in extras/gitui/git2-testing/src/lib.rs (behind `#[allow(unsafe_code)]` on one function, with a provenance comment) and the intentional invalid-UTF-8 constructor in extras/gitui/invalidstring/src/lib.rs.
11. Lossless numeric conversion instead of `as`. The `easy-cast` crate's `Cast` trait is used at UI boundaries, e.g. `x += Cast::<u16>::cast(symbol.width());` in extras/gitui/src/ui/stateful_paragraph.rs, and its failure mode is integrated into the error enum (`EasyCast(#[from] easy_cast::Error)` in extras/gitui/asyncgit/src/error.rs).
12. Feature-compiled instrumentation with RAII. extras/gitui/scopetime/src/lib.rs implements `Drop for ScopeTimeLog<'_>` to log elapsed time, and exports two versions of `scope_time!`: the real one under `#[cfg(feature = "enabled")]` and an empty `macro_rules! scope_time { ($target:literal) => {}; }` otherwise, so instrumentation costs zero in normal builds yet stays syntactically valid everywhere.
13. Platform `cfg` handling at function granularity. extras/gitui/src/clipboard.rs selects `pbcopy` under `#[cfg(target_os = "macos")]`, `clip.exe` under `#[cfg(windows)]`, and probes wl-copy/xclip/xsel via `which` elsewhere; unix-only tests are gated `#[cfg(unix)]` (extras/gitui/asyncgit/src/sync/sign.rs) and one hooks test is `#[cfg(target_os = "linux")]` (extras/gitui/asyncgit/src/sync/hooks.rs).
14. Derive-powered partial configuration. Keybindings are a plain struct with `#[derive(Debug, Clone, Patch)]` and `#[patch(attribute(derive(Deserialize, Debug)))]` from `struct-patch` (extras/gitui/src/keys/key_list.rs); user RON files deserialize into the generated patch type and are applied over defaults with `keys_list.apply(patch)`, so a config file only ever needs to mention the keys it overrides (a full example ships as extras/gitui/vim_style_key_config.ron).
15. Ergonomic enum conversions instead of bare booleans. `EventState` in extras/gitui/src/components/mod.rs replaces a `bool` return with a named two-variant enum and supplies `impl From<bool> for EventState` plus an `is_consumed()` accessor, so event handlers read as intent (`Ok(true.into())` at call sites, `if c.event(ev)?.is_consumed()` in the pump) and cannot be accidentally inverted. The same file models help-bar propagation as `CommandBlocking::{Blocking, PassingOn}` rather than a boolean flag.
16. Wrapper types to bridge foreign traits. `GituiKeyEvent` in extras/gitui/src/keys/key_list.rs wraps `crossterm::event::KeyEvent`'s fields so the project can derive `Serialize`/`Deserialize` and `struct-patch` support on key bindings, with `From<&GituiKeyEvent> for KeyEvent` conversions and a custom `PartialEq` that compares through the canonical crossterm representation, keeping serialization concerns out of the vendor type.

## 10. Documentation practices

* Crate docs double as architecture docs. extras/gitui/src/main.rs opens with a `//!` map of the module groups (tabs, components, popups, ui, asyncgit) and of the included crates with their dependency relationships. extras/gitui/src/components/mod.rs documents the composition philosophy explicitly, including its limits ("composition is driven by code", plus an honest note that the two traits should probably merge someday).
* asyncgit enforces docs with `#![forbid(missing_docs)]` (extras/gitui/asyncgit/src/lib.rs). The team consciously trades prose for coverage: many items carry an empty `///` doc, and `clippy::empty_docs` is allowed, meaning the forbid acts as a checklist that makes undocumented surface impossible while letting trivial items stay terse.
* User docs are versioned markdown at the repo root: extras/gitui/KEY_CONFIG.md (custom keybindings), extras/gitui/THEMES.md (theme RON patching), extras/gitui/FAQ.md, extras/gitui/NIGHTLIES.md (nightly artifact URLs), and a 301-line README with a linked table of contents and a per-package-manager install matrix.
* extras/gitui/CONTRIBUTING.md is short and welcoming: build instructions by reference, a Discord link for help, and a pointer to `good-first-issue` labels.
* extras/gitui/.github/PULL_REQUEST_TEMPLATE.md encodes the quality gate as a checklist:

```markdown
I followed the checklist:
- [ ] I added unittests
- [ ] I ran `make check` without errors
- [ ] I tested the overall application
- [ ] I added an appropriate item to the changelog
```

* Issue templates exist for bug reports and feature requests (extras/gitui/.github/ISSUE_TEMPLATE/bug_report.md, feature_request.md), and the in-app `--bugreport` flag (extras/gitui/src/bug_report.rs, built on the `bugreport` crate) prints version, OS, compile-time info, and relevant environment variables as Markdown ready to paste into an issue.

## 11. Release and distribution

* Versioning is SemVer with the binary and asyncgit released in lockstep at 0.28.1 (extras/gitui/Cargo.toml, extras/gitui/asyncgit/Cargo.toml); utility crates version independently (filetreelist 0.6.0, git2-hooks 0.7.0).
* Changelog discipline is strict Keep a Changelog: extras/gitui/CHANGELOG.md (1,025 lines) keeps an `## Unreleased` section that every PR must append to (enforced socially by the PR template and mechanically by the `log-test` CI job that extracts release notes from it on every run). Entries credit contributors by handle and link issues, and release sections embed screenshots of headline features.
* The release pipeline is tag-driven (extras/gitui/.github/workflows/cd.yml): artifacts are mac arm64 and x86 tarballs, a musl-static Linux x86_64 tarball, aarch64/armv7/arm tarballs, a Windows tarball, and a WiX MSI (sources in extras/gitui/wix/main.wxs). Makefile release targets strip binaries and print `otool -L` so accidental dynamic linkage is visible in logs. Release bodies come from the changelog via `ffurrer2/extract-release-notes@v2`; hyphenated tags publish as prereleases; a successful stable release auto-bumps homebrew-core.
* Reproducibility and provenance: extras/gitui/build.rs honors `SOURCE_DATE_EPOCH` for the build date, accepts `BUILD_GIT_COMMIT_ID` for `git archive` tarballs, and stamps `GITUI_BUILD_NAME` as either the bare version (when `GITUI_RELEASE=1`) or `<version>-nightly <date> (<hash>)`, which `clap` then surfaces via `.version(env!("GITUI_BUILD_NAME"))` in extras/gitui/src/args.rs.
* A parallel nightly channel (extras/gitui/.github/workflows/nightly.yml) rebuilds all platforms daily and pushes to S3, giving users a low-friction way to verify fixes before a release.
* gitui is a flag-only CLI (no subcommands), and the repository ships no shell completions or man pages; discoverability is delegated to the in-app help and `--help` template defined in extras/gitui/src/args.rs.
* License compliance for distributors is one command away: the Makefile's `licenses` target runs `cargo bundle-licenses --format toml --output THIRDPARTY.toml` (extras/gitui/Makefile), producing a machine-readable third-party license inventory that packagers can regenerate at any tag.
* Local packaging parity: the same Makefile targets CI uses (`release-mac`, `release-win`, `release-linux-musl`) are runnable on a developer machine, so a maintainer can reproduce any release artifact without GitHub Actions, and `install` / `install-timing` targets exercise the exact `cargo install --path "." --offline --locked` path users hit.

## 12. Lessons for quinjet

quinjet already matches gitui on rustfmt, cargo-deny, typos, taplo-style TOML checking, and a stricter clippy wall. The practices still worth importing, with mechanisms:

1. Adopt cargo-nextest as the test runner: `cargo nextest run --workspace` in the Makefile and `taiki-e/install-action@nextest` in CI, plus filter expressions (`-E 'not test(name)'`) for environment-dependent exclusions, as in extras/gitui/Makefile.
2. Add full-TUI snapshot tests: drive the ratatui app with `ratatui::backend::TestBackend`, assert frames with `insta::assert_snapshot!`, and normalize temp paths and commit hashes with `insta::Settings` filters (`features = ["filters"]`), mirroring extras/gitui/src/gitui.rs and extras/gitui/src/snapshots/.
3. Sandbox Git global config in every test: call `git2::opts::set_search_path` for System/Global/XDG/ProgramData to a temp dir inside a `std::sync::Once`, as extras/gitui/git2-testing/src/lib.rs does, so a developer's `.gitconfig` can never change test results.
4. Pin the MSRV three times: `rust-version` in Cargo.toml, `msrv` in `.clippy.toml`, and a literal MSRV row in the CI matrix with `continue-on-error` only on the nightly row (extras/gitui/.github/workflows/ci.yml).
5. Add a `cargo install --path . --force --locked` CI step: it catches lockfile drift and packaging breakage that plain `cargo build` misses.
6. Add a `cargo-udeps` job on the nightly toolchain (`cargo +nightly udeps --all-targets`) to keep the dependency list honest.
7. Enforce changelog extractability in CI: Keep a Changelog format plus a job running `ffurrer2/extract-release-notes@v2` on every PR, then reuse the same extraction for the GitHub release body in the tag-triggered workflow (extras/gitui/.github/workflows/cd.yml).
8. Group dependabot cargo updates with `groups:` keyed on `update-types` minor/patch (extras/gitui/.github/dependabot.yml) to collapse update noise.
9. Install a panic hook that restores the terminal before printing, capture a `backtrace::Backtrace`, and pair it with `scopeguard::defer!` for the normal shutdown path (extras/gitui/src/main.rs); for a TUI this is the difference between a readable crash report and a corrupted shell.
10. Ship a `--bugreport` flag using the `bugreport` crate with `SoftwareVersion`, `OperatingSystem`, `CompileTimeInformation`, and selected `EnvironmentVariables` collectors printed as Markdown (extras/gitui/src/bug_report.rs).
11. Stamp rich version strings from `build.rs`: embed short git hash and build date into an env var consumed by clap, honor `SOURCE_DATE_EPOCH` for reproducible builds, and gate release naming on an env flag as extras/gitui/build.rs does with `GITUI_RELEASE`.
12. Add feature-gated scope timing: a `scope_time!("label")` RAII macro that logs elapsed milliseconds and compiles to nothing without the `timing` feature (extras/gitui/scopetime/src/lib.rs), plus a `make profile` target wrapping `cargo flamegraph`.
13. Turn on `multiple-versions = "deny"` in deny.toml `[bans]`, documenting each `skip-tree` exception with the responsible crate and upstream issue link, following extras/gitui/deny.toml.
14. Speed up debug iteration with `[profile.dev.package."ratatui"] opt-level = 3` and shrink releases with `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = "debuginfo"` (extras/gitui/Cargo.toml).
15. Use `Swatinem/rust-cache@v2` with a `shared-key` of `${{ matrix.os }}-${{ env.cache-name }}-${{ matrix.rust }}` so cache entries are correctly partitioned per OS and toolchain.
16. Support partial user config via `struct-patch`: derive `Patch` on the keybinding and options structs and apply deserialized RON patches over defaults (extras/gitui/src/keys/key_list.rs), so user files only state deltas.
17. Encode the contribution gate in `.github/PULL_REQUEST_TEMPLATE.md` as a checklist referencing the repo's own `make check` aggregate target (extras/gitui/.github/PULL_REQUEST_TEMPLATE.md).
18. If distribution matters, copy the release lattice: musl-static Linux binary (`--target x86_64-unknown-linux-musl` with `musl-tools`), prerelease detection via `contains(github.ref, '-')`, and a scheduled nightly artifact channel (extras/gitui/.github/workflows/nightly.yml).
