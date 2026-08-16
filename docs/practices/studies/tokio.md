# tokio-rs/tokio (32930 stars)

## 1. What the project is and how big it is

Tokio is the de facto standard asynchronous runtime for Rust: an event-driven, non-blocking I/O
platform providing a work-stealing task scheduler, timers, and async TCP/UDP/filesystem/process/signal
APIs. Nearly every production async Rust service sits on top of it, directly or through frameworks
such as hyper, axum, tonic, and reqwest. The crate description in `extras/tokio/tokio/Cargo.toml`
states it plainly:

```toml
description = """
An event-driven, non-blocking I/O platform for writing asynchronous I/O
backed applications.
"""
```

Measurable scale from the clone:

- 10 workspace members declared in `extras/tokio/Cargo.toml`: five published crates (`tokio`,
  `tokio-macros`, `tokio-test`, `tokio-stream`, `tokio-util`) and five internal ones (`benches`,
  `examples`, `stress-test`, `tests-build`, `tests-integration`).
- 793 `.rs` files, roughly 180,000 lines of Rust across the repository.
- The main `tokio` crate alone: 378 source files, about 106,000 lines under
  `extras/tokio/tokio/src`, plus 174 integration test files in `extras/tokio/tokio/tests`.
- Current version `1.53.1`, MSRV `1.71`, edition 2021 (`extras/tokio/tokio/Cargo.toml`).
- Two out-of-workspace fuzz crates (`extras/tokio/tokio/fuzz`, `extras/tokio/tokio-stream/fuzz`).

Industry uses it because it is fast, has a decade of hardening, guarantees LTS branches with a year
of backported fixes (`extras/tokio/CONTRIBUTING.md`), and holds a strict 1.x stability promise
verified mechanically in CI.

## 2. Repository layout

```text
extras/tokio/
|-- Cargo.toml            workspace root: members, crates-io patch, workspace lints
|-- tokio/                the runtime crate (src/, tests/, fuzz/, docs/, CHANGELOG.md)
|-- tokio-macros/         proc macros (#[tokio::main], #[tokio::test])
|-- tokio-stream/         Stream utilities (own fuzz/ dir)
|-- tokio-test/           published testing utilities (mock IO, task harness, assert macros)
|-- tokio-util/           codecs, compat layers, DelayQueue, JoinMap
|-- benches/              criterion benchmarks, publish = false
|-- examples/             runnable examples (chat.rs, proxy.rs, tinyhttp.rs, ...)
|-- stress-test/          long-running leak scenarios run under valgrind
|-- tests-build/          trybuild-style macro UI tests (pass/ and fail/ with .stderr)
|-- tests-integration/    cross-feature and wasi integration binaries and tests
|-- target-specs/         custom JSON target spec (i686 without AtomicU64)
|-- docs/contributing/    the real contributor handbook (6 documents)
|-- .github/              workflows/, buildomat/ (illumos CI), templates, labeler
|-- deny.toml, spellcheck.toml, spellcheck.dic, Cross.toml, netlify.toml
```

The split works because each published crate has an independent version and changelog, while all
test-support machinery lives in unpublished members marked `publish = false` and `version = "0.0.0"`
(`extras/tokio/benches/Cargo.toml`). The root manifest patches crates.io so intra-workspace
dependencies always resolve to the local checkout:

```toml
[patch.crates-io]
tokio = { path = "tokio" }
tokio-macros = { path = "tokio-macros" }
```

CI even removes this patch mid-job with `perl -0 -i -pe 's/\[patch\.crates-io\].+\n\[/[/s' Cargo.toml`
to prove crates also build against published dependency versions
(`extras/tokio/.github/workflows/ci.yml`).

## 3. Cargo manifest practices

Tokio predates `workspace.package` inheritance and deliberately keeps full metadata in each crate,
because crates release independently and even from different LTS branches. What it does share is the
lints table: every published crate ends with `[lints] workspace = true`, and the root defines exactly
one rule, an exhaustive `check-cfg` registry of every custom `--cfg` the project uses
(`extras/tokio/Cargo.toml`):

```toml
[workspace.lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = [
  'cfg(fuzzing)',
  'cfg(loom)',
  'cfg(tokio_unstable)',
  'cfg(tokio_no_parking_lot)',
] }
```

Other notable manifest habits in `extras/tokio/tokio/Cargo.toml`:

- The release checklist is a comment pinned directly above the version field, so it cannot be missed:

```toml
# When releasing to crates.io:
# - Remove path dependencies (if any)
# - Update doc url
#   - README.md
# - Update CHANGELOG.md.
# - Create "v1.x.y" git tag.
version = "1.53.1"
```

- `default = []` with the explicit comment `# Include nothing by default`, plus a `full` umbrella
  feature. Every feature maps precisely onto optional dependencies and their sub-features, including
  granular `windows-sys` API surfaces:

```toml
net = [
  "libc",
  "mio/os-poll",
  "mio/os-ext",
  "mio/net",
  "socket2",
  "windows-sys/Win32_Foundation",
  ...
]
```

- Unstable features are double-gated: the feature flag exists, but the dependency table only
  activates under a compiler flag: `[target.'cfg(tokio_unstable)'.dependencies]` and
  `[target.'cfg(all(tokio_unstable, target_os = "linux"))'.dependencies]` for `io-uring`. Users must
  pass `--cfg tokio_unstable` in `RUSTFLAGS`, which keeps semver intact for experimental API.
- The proc macro companion is pinned with a tilde requirement, `tokio-macros = { version = "~2.7.0",
  optional = true }`, because macro output and runtime internals must move in lockstep.
- Platform-conditional dev-dependencies are extensive: `loom` under `cfg(loom)`, `wasm-bindgen-test`
  for wasm, `mio-aio` only on FreeBSD, `nix` only on Unix.
- docs.rs configuration passes the unstable cfg to both rustdoc and rustc:

```toml
[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs", "--cfg", "tokio_unstable"]
rustc-args = ["--cfg", "tokio_unstable"]
```

- Public dependency control via `[package.metadata.cargo_check_external_types]` with an explicit
  `allowed_external_types` list (`bytes::buf::buf_impl::Buf`, `tokio_macros::*`), enforced in CI.
- No `[profile]` sections in the workspace root at all; only the fuzz crates set
  `[profile.release] debug = 1` (`extras/tokio/tokio/fuzz/Cargo.toml`).
- MSRV is repeated as `rust-version = "1.71"` per crate, and `extras/tokio/.github/workflows/ci.yml`
  lists every file to update when bumping it, next to the `rust_min: '1.71'` env var.
- Dependency version policy is written down in
  `extras/tokio/docs/contributing/how-to-specify-crates-dependencies-versions.md`: declare the
  minimal version actually needed, which the `minimal-versions` CI job then proves.

## 4. Formatting

Tokio has no `rustfmt.toml` and no `.rustfmt.toml`: default rustfmt style, zero configuration. The
interesting part is how it is enforced. Because `cargo fmt` skips code hidden behind `cfg` macros
(rust-lang/cargo#7732), CI formats every tracked file directly
(`extras/tokio/.github/workflows/ci.yml`):

```yaml
- name: "rustfmt --check"
  # Workaround for rust-lang/cargo#7732
  run: |
    if ! rustfmt --check --edition 2021 $(git ls-files '*.rs'); then
      printf "Please run \`rustfmt --edition 2021 \$(git ls-files '*.rs')\` ..." >&2
      exit 1
    fi
```

There is no `.editorconfig` and no formatter for TOML or YAML. Instead, prose gets the tooling: the
`check-spelling` job runs `cargo-spellcheck` over all rustdoc with `extras/tokio/spellcheck.toml`
(Hunspell en_US plus a 328-line project dictionary `extras/tokio/spellcheck.dic`), and a shell step
validates that the dictionary's first line equals the word count and that the list is sorted and
duplicate-free with `LC_ALL=en_US.UTF8 sort -uc`. The same job bans trailing whitespace repo-wide:

```yaml
- name: Detect trailing whitespace
  run: |
    if grep --exclude-dir=.git --exclude-dir=target -rne '\s$' .
```

## 5. Linting

Clippy configuration lives in three places, none of them a giant deny list:

1. Global hard wall: `RUSTFLAGS: -Dwarnings` in the CI env promotes every rustc, clippy, and rustdoc
   warning to an error (`extras/tokio/.github/workflows/ci.yml`).
2. A pinned clippy version, `rust_clippy: '1.88'`, so a new stable release cannot suddenly fail
   unrelated PRs with new lints. The pin is bumped deliberately.
3. Crate-level attributes in `extras/tokio/tokio/src/lib.rs` express the philosophy: allow a handful
   of lints that fight the architecture, warn on API hygiene, deny only what indicates real bugs:

```rust
#![allow(
    clippy::cognitive_complexity,
    clippy::large_enum_variant,
    clippy::module_inception,
    clippy::needless_doctest_main
)]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unused_must_use, unsafe_op_in_unsafe_fn)]
```

Doctests get their own lint regime through
`#![doc(test(no_crate_inject, attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))))]`,
so every example in the documentation compiles warning-free while still allowing illustrative unused
variables. The clippy CI job runs twice, `--workspace --tests --no-deps` with stable features and
again with `--all-features` under `--cfg tokio_unstable`, and in between strips the crates-io patch
to lint against released dependencies.

Beyond clippy, tokio builds a fleet of custom checkers into CI: `cargo-check-external-types` (public
API cannot leak types not on the allowlist), `cargo-semver-checks` (no accidental major), a
`check-readme` job that literally runs `diff README.md tokio/README.md` and greps the README for the
current `Cargo.toml` version, the sorted-dictionary check, and the trailing whitespace grep. The
lesson: lint the API surface and release invariants, not just the code style.

## 6. CI/CD

`extras/tokio/.github/workflows/ci.yml` is 1420 lines and defines 45 jobs. Structure and highlights:

- Gating: a `basics` job needs `clippy`, `fmt`, `docs`, `minrust` and does nothing but `run: exit 0`;
  every expensive job declares `needs: basics`. Cheap failures cancel the whole pyramid.
- Concurrency: every workflow sets
  `group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.sha }}` with
  `cancel-in-progress: true`, so force-pushes never waste runner hours.
- Least privilege: top-level `permissions: contents: read`; the nightly audit job alone escalates to
  `checks: write` and `issues: write` (`extras/tokio/.github/workflows/audit.yml`).
- Triggers include LTS branches: `branches: ["master", "tokio-*.x"]`, and the `minrust` and `semver`
  jobs special-case backport PRs by inspecting `github.event.pull_request.base.ref`.
- OS and architecture coverage is extreme: ubuntu/windows/macos matrices, native ARM runners
  (`ubuntu-24.04-arm`, `windows-11-arm`), qemu cross-tests (`taiki-e/setup-cross-toolchain-action`
  with `qemu: '7.2'`) for i686, armv5te, armv7, aarch64, tier-3 checks with `-Zbuild-std` for Haiku,
  FreeBSD in a full VM via `vmactions/freebsd-vm@v1`, Redox and Fortanix SGX build checks, three wasm
  targets executed under pinned `wasmtime` versions, and illumos through the external Buildomat CI
  (`extras/tokio/.github/buildomat/config.toml` with `org_only = false` so fork PRs run too).
- A custom target spec, `extras/tokio/target-specs/i686-unknown-linux-gnu.json`, simulates a platform
  without `AtomicU64` to test the fallback atomics.
- Kernel-matrix testing: `get-latest-kernel-version` fetches `https://www.kernel.org/releases.json`,
  then a reusable workflow (`extras/tokio/.github/workflows/uring-kernel-version-test.yml`, invoked
  with `uses: ./.github/workflows/...`) compiles that kernel plus an ancient 4.19 kernel and boots
  them under qemu to test io_uring presence and absence.
- Correctness tooling as first-class jobs: three miri jobs (lib, integration, doctests) on a pinned
  `rust_miri_nightly` with `MIRIFLAGS: -Zmiri-disable-isolation -Zmiri-strict-provenance`, an asan
  job, valgrind leak checks on purpose-built binaries from `tests-integration`, and a
  `panic=abort` job running the suite with `RUSTFLAGS="... -C panic=abort -Zpanic-abort-tests"`.
- Feature hygiene: `cargo hack check --feature-powerset --depth 2`, `--each-feature` for the
  integration crates, and a `minimal-versions` job that removes dev-dependencies with
  `cargo hack --remove-dev-deps` before `cargo update -Z minimal-versions`.
- Reverse-dependency insurance: `test-hyper` and `test-quinn` clone those projects at their latest
  release tag, append `[patch.crates-io] tokio = { path = "../tokio" }`, and run their full test
  suites against the PR.
- Caching: `Swatinem/rust-cache@v2` everywhere, with a documented `cache-bin` workaround for macOS;
  toolchains via `dtolnay/rust-toolchain`, binaries via `taiki-e/install-action`. Actions are pinned
  to major version tags, with `.github/dependabot.yml` updating them weekly.
- Loom model checking is split out into `extras/tokio/.github/workflows/loom.yml` (7 jobs) and only
  runs on PRs when `.github/labeler.yml` auto-applies an `R-loom-*` label based on which paths
  changed; on pushes to master it always runs. Heavy verification is targeted, not blanket.
- Security: `audit.yml` runs `EmbarkStudios/cargo-deny-action@v2` on a daily cron plus pushes
  touching any `Cargo.toml`; `pr-audit.yml` runs the same check path-filtered on PRs. `deny.toml`
  allows only MIT and Apache-2.0 (one Unicode-3.0 exception), denies wildcard requirements and
  unknown registries or git sources.

Tests run under `cargo-nextest`, with doctests executed separately because nextest cannot run them:

```yaml
cargo nextest run --features full
cargo test --doc --features full
```

## 7. Testing

Tokio's stated policy, from `extras/tokio/docs/contributing/pull-requests.md`:

```text
There are two ways to write tests: integration tests and documentation tests.
(Tokio avoids unit tests as much as possible).
```

- Integration tests: 174 files in `extras/tokio/tokio/tests`, named by area
  (`fs_open_options.rs`, `io_copy_bidirectional.rs`, `rt_metrics` style, `sync_*`, `uring_*`), each
  starting with a `#![cfg(feature = "full")]`-style gate so they compose with feature matrices.
- Unit tests exist only where they must: 31 `mod tests` inside `extras/tokio/tokio/src`, mostly
  loom concurrency models that need access to private internals, run via
  `cargo test --lib --release --features full` with `RUSTFLAGS="--cfg loom"`.
- The published `tokio-test` crate (`extras/tokio/tokio-test/src`) is the harness toolkit: a mock
  `AsyncRead`/`AsyncWrite` builder in `io.rs`, a manual task driver in `task.rs`,
  `assert_ready!`/`assert_pending!` macros, and `stream_mock.rs`. Test infrastructure is a product.
- Compile-time trait tests: `extras/tokio/tokio/tests/async_send_sync.rs` asserts `Send`/`Sync`/
  `Unpin` for every public future using a method-resolution ambiguity trick:

```rust
trait AmbiguousIfSend<A> { fn some_item(&self) {} }
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
impl<T: ?Sized + Send> AmbiguousIfSend<Invalid> for T {}
```

  A dedicated CI job re-checks this file with parking_lot's `send_guard` feature force-enabled via
  `sed` on the manifest, because that feature can silently change auto-traits.

- Snapshot testing: `extras/tokio/tests-build/tests/fail` holds macro misuse cases with committed
  `.stderr` files (`macros_invalid_input.stderr`, `macros_join.stderr`), trybuild-style, exercised
  per feature with `cargo hack test --each-feature`.
- Fuzzing: `cargo-fuzz` targets in `extras/tokio/tokio/fuzz/fuzz_targets/fuzz_linked_list.rs` and
  `extras/tokio/tokio-stream/fuzz/fuzz_targets/fuzz_stream_map.rs`. The main crate exposes internal
  fuzz hooks through a public shim, `extras/tokio/tokio/src/fuzz.rs`:

```rust
pub use crate::util::linked_list::tests::fuzz_linked_list;
```

  CI keeps the harnesses compiling with `cargo fuzz check --all-features`.

- Property testing: `proptest = "1"` as a non-wasm dev-dependency of the main crate.
- Benchmarks: the `benches` member uses criterion with `harness = false` and one `[[bench]]` per
  file (`extras/tokio/benches/Cargo.toml`); CI only `cargo check --benches` so they never rot.
- Model checking: loom is wired through the `extras/tokio/tokio/src/loom` facade (see section 9) and
  budgeted in CI with `LOOM_MAX_PREEMPTIONS: 2` and `LOOM_MAX_BRANCHES: 10000`.
- Leak testing: `extras/tokio/stress-test` examples run under
  `valgrind --error-exitcode=1 --leak-check=full` (`extras/tokio/.github/workflows/stress-test.yml`).

## 8. Error handling and API design

Neither `thiserror` nor `anyhow` appears anywhere in the workspace manifests. Every error is a
hand-written type shaped for its call site:

- Payload-returning errors: `extras/tokio/tokio/src/sync/mpsc/error.rs` defines
  `pub struct SendError<T>(pub T);` and `enum TrySendError<T> { Full(T), Closed(T) }` with
  `into_inner`, so a failed send never destroys the caller's value. `Debug` is implemented manually
  with `f.debug_struct("SendError").finish_non_exhaustive()` to avoid a `T: Debug` bound.
- Encapsulated internals: `JoinError` in `extras/tokio/tokio/src/runtime/task/error.rs` wraps a
  private `enum Repr { Cancelled, Panic(SyncWrapper<Box<dyn Any + Send + 'static>>) }` and exposes
  intent-revealing predicates (`is_cancelled`, `is_panic`); the `SyncWrapper` newtype makes a
  non-`Sync` panic payload safely `Sync`.
- I/O surfaces reuse `std::io::Result` rather than inventing parallel error types.
- Panic policy is explicit and located: 167 uses of `#[track_caller]` in `tokio/src` mean runtime
  panics (like blocking inside a runtime) report the user's line, not tokio internals. `Panics`
  sections in rustdoc document each case, and `#![deny(unused_must_use)]` ensures results and
  futures cannot be silently dropped (79 `must_use` annotations).
- Builder pattern: `runtime::Builder` in `extras/tokio/tokio/src/runtime/builder.rs` is the
  canonical Rust builder, entry points `new_multi_thread()`/`new_current_thread()`, chained setters,
  fallible `build() -> io::Result<Runtime>`.
- Visibility discipline: `#![warn(unreachable_pub)]` plus pervasive `pub(crate)`; 14 uses of
  `#[non_exhaustive]`; and the external-types CI check guarantees only `bytes` traits and the macro
  crate leak into the public API.

## 9. Deep Rust usage

1. Conditional-compilation macro DSL: `extras/tokio/tokio/src/macros/cfg.rs` defines 64
   `macro_rules! cfg_*` wrappers plus a generic `feature!` macro that stamps both the `cfg` and the
   docs.rs `doc(cfg(...))` annotation on every item, so feature labels in documentation can never
   drift from reality:

   ```rust
   macro_rules! cfg_windows {
    ($($item:item)*) => {
        $(
            #[cfg(any(all(doc, docsrs), windows))]
            #[cfg_attr(docsrs, doc(cfg(windows)))]
            $item
        )*
    }
   }
   ```

2. The loom facade: `extras/tokio/tokio/src/loom/mod.rs` swaps the entire concurrency vocabulary
   between `std` and the loom model checker with two `cfg` lines, so production code imports
   `crate::loom::sync::Mutex` and gets model checking for free under `--cfg loom`:

   ```rust
   #[cfg(not(all(test, loom)))]
   mod std;
   #[cfg(all(test, loom))]
   mod mocked;
   ```

3. Closure-scoped `UnsafeCell`: `extras/tokio/tokio/src/loom/std/unsafe_cell.rs` wraps
   `std::cell::UnsafeCell` so raw pointers only exist inside `with`/`with_mut` closures, mirroring
   loom's checked API and making every access auditable:

   ```rust
   pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
    f(self.0.get())
   }
   ```

4. Intrusive data structures with a documented safety contract:
   `extras/tokio/tokio/src/util/linked_list.rs` builds a pinned intrusive doubly linked list on
   `NonNull`, `PhantomPinned`, and an `unsafe trait Link` whose docs spell out the pinning
   guarantee; `Send`/`Sync` are bounded manually:

   ```rust
   unsafe impl<L: Link> Send for LinkedList<L> where L::Target: Send {}
   unsafe impl<L: Link> Sync for LinkedList<L> where L::Target: Sync {}
   ```

5. Unsafe policy as lint plus prose: `#![deny(unsafe_op_in_unsafe_fn)]` crate-wide
   (`extras/tokio/tokio/src/lib.rs`), 132 `SAFETY:` comments in `tokio/src`, and the one module that
   opts out (`linked_list.rs`) justifies it in a file-level comment and is compensated by miri,
   fuzzing, and loom coverage.
6. Micro-architecture-aware layout: `extras/tokio/tokio/src/util/cacheline.rs` defines `CachePadded`
   with per-arch alignment (`repr(align(128))` on x86_64/aarch64/powerpc64, smaller elsewhere),
   each choice cited to Intel manuals, folly, and the Go runtime.
7. Atomic fallbacks by capability: `extras/tokio/tokio/src/loom/std/` carries `atomic_u64_native.rs`
   and `atomic_u64_as_mutex.rs` variants, selected by `target_has_atomic`, and CI proves the mutex
   path on a custom target spec without `AtomicU64`.
8. Declarative macro engineering: `extras/tokio/tokio/src/macros/select.rs` is a 1414-line
   `macro_rules!` implementation of `select!` with token-counting and pattern normalization, backed
   by a `#[doc(hidden)]` support module (`extras/tokio/tokio/src/macros/support.rs`) that re-exports
   `poll_fn`, budget hooks, and a `thread_rng_n` used for fair branch polling. Proc macros are
   quarantined in `tokio-macros` so `syn` never burdens the main crate's build.
9. Pinning without proc macros: `pin-project-lite` appears in 38 source files, and
   `extras/tokio/tokio/src/macros/pin.rs` documents stack pinning with `compile_fail` doctests that
   assert the bad pattern really fails to compile.
10. Docs-only uninhabited types: `extras/tokio/tokio/src/doc/mod.rs` declares
    `pub enum NotDefinedHere {}` as a never-like stand-in so platform-specific type aliases render
    on docs.rs for all platforms without being usable:

    ```rust
    /// This type is uninhabitable like the [`never` type] to ensure that no one
    /// will ever accidentally use it.
    #[derive(Debug)]
    pub enum NotDefinedHere {}
    ```

11. Auto-trait regression tests as API contract: the `AmbiguousIfSend` device from section 7 turns
    `Send`/`Sync`/`Unpin` for hundreds of futures into compile errors on regression.
12. `#[track_caller]` (167 uses) and `const fn` constructors (66 uses) are applied systematically,
    the former for panic ergonomics, the latter so statics like wakers and lists initialize at
    compile time.

## 10. Documentation practices

- The crate root `extras/tokio/tokio/src/lib.rs` opens with a book-length "A Tour of Tokio" that
  teaches feature selection for applications versus libraries, with `missing_docs` warned on so no
  public item ships undocumented.
- Doctests are hardened globally via `#![doc(test(no_crate_inject, attr(deny(warnings, ...))))]`,
  and CI builds docs with `RUSTDOCFLAGS: --cfg docsrs --cfg tokio_unstable -Dwarnings` including
  `--document-private-items`, so broken intra-doc links fail the build.
- Master-branch docs deploy through `extras/tokio/netlify.toml`, which installs nightly, builds
  `cargo doc --no-deps --all-features` with the docsrs cfg, and redirects `/` to `/tokio`.
- `extras/tokio/README.md` and `extras/tokio/tokio/README.md` are kept byte-identical by the
  `check-readme` CI job (`diff README.md tokio/README.md`), which also greps the README for the
  manifest version.
- `extras/tokio/CONTRIBUTING.md` is a 54-line front door holding the LTS, MSRV, and versioning
  policies; the real handbook lives in `extras/tokio/docs/contributing/`: a 409-line
  `pull-requests.md` (workflow, exact cargo commands, test philosophy, benchmark instructions),
  `reviewing-pull-requests.md` for maintainers, `keeping-track-of-issues-and-prs.md` documenting the
  label taxonomy that CI's labeler consumes, and a dependency-version policy document.
- Commit convention: `module: explain the commit in one line`, lowercase, with `Fixes: #1337` and
  `Refs:` trailers, specified with a full sample message in `pull-requests.md`.
- Issue templates (`extras/tokio/.github/ISSUE_TEMPLATE/bug_report.md`) pre-apply labels
  (`A-tokio, C-bug`) and demand `cargo tree | grep tokio` output; the PR template asks only for
  Motivation and Solution sections.
- Design documents live next to the code: `extras/tokio/tokio/docs/reactor-refactor.md` records the
  I/O driver redesign with goals and non-goals.
- All rustdoc prose is spellchecked in CI against a versioned dictionary.

## 11. Release and distribution

- Each published crate versions independently with its own `CHANGELOG.md`; tags are `v1.x.y` for
  tokio and prefixed like `tokio-util-0.7.x` for subcrates, as recorded in each manifest's release
  checklist comment (`extras/tokio/tokio-util/Cargo.toml`).
- Changelog entries are categorized (`Added`, `Changed`, `Fixed`, `Documented`, with unstable
  changes separated as `Fixed (unstable)`) and every line links its PR number
  (`extras/tokio/tokio/CHANGELOG.md`).
- Versioning policy is written and enforced: patch releases are bug fixes only, minors may raise
  MSRV, all per SemVer 2.0 (`extras/tokio/CONTRIBUTING.md`), and `cargo-semver-checks` in CI blocks
  accidental majors with `release-type: minor` and `feature-group: only-explicit-features`.
- LTS branches `tokio-*.x` get at least one year of backported fixes; every workflow triggers on
  those branches, and jobs adapt when the PR base is an LTS branch.
- There is no release automation workflow in the repository: publishing is a manual, checklist-driven
  act, which fits a library where the hard part is deciding, not uploading. Distribution is purely
  crates.io; there are no binaries, so no completions or man pages.

## 12. Lessons for quinjet

Quinjet already runs a stricter clippy wall than tokio, so the transferable value is in CI topology,
verification breadth, and release discipline:

1. Add a `basics` gate job: make fmt, clippy, doc, and MSRV checks a `needs:` prerequisite of every
   expensive job, exactly like the `basics` job with `run: exit 0` in
   `extras/tokio/.github/workflows/ci.yml`, and add a `concurrency` group keyed on
   `github.event.pull_request.number || github.sha` with `cancel-in-progress: true`.
2. Register every custom cfg: adopt tokio's only workspace lint,
   `[lints.rust] unexpected_cfgs = { level = "warn", check-cfg = [...] }`, for any quinjet test or
   instrumentation cfg, so a typoed `#[cfg]` cannot silently disable code.
3. Switch CI test execution to `cargo-nextest` (installed via `taiki-e/install-action@v2`) with a
   separate `cargo test --doc` step, and add a nightly job running the suite with
   `RUSTFLAGS="-C panic=abort -Zpanic-abort-tests"` to catch unwind-dependent tests.
4. Pin the clippy toolchain like `rust_clippy: '1.88'` instead of floating stable, and keep the MSRV
   as a single `rust_min` env var whose update sites are listed in a comment.
5. Run a real OS matrix (`ubuntu-latest`, `windows-latest`, `macos-latest`) for a crossterm TUI:
   terminal and path behavior differ exactly where tokio's matrix catches bugs; cache with
   `Swatinem/rust-cache@v2` and install toolchains with `dtolnay/rust-toolchain`.
6. Split cargo-deny into tokio's two workflows: a path-filtered PR job (`pr-audit.yml`, triggering
   on `paths: ['**/Cargo.toml']`) and a daily `schedule: cron` job (`audit.yml`) so new advisories
   surface between merges, both via `EmbarkStudios/cargo-deny-action@v2`.
7. Add `.github/dependabot.yml` with `package-ecosystem: "github-actions"` weekly, so pinned actions
   keep moving.
8. Adopt payload-returning error types for the command layer: tokio's
   `SendError<T>(pub T)` and `TrySendError::into_inner` pattern
   (`extras/tokio/tokio/src/sync/mpsc/error.rs`) maps directly onto returning the user's staged
   input when a git operation fails, instead of stringifying it.
9. Put `#[track_caller]` on every panicking or invariant-checking helper so debug output points at
   the calling command module, as tokio does 167 times.
10. Encode compile-time contracts as tests: an `async_send_sync.rs`-style file asserting auto traits
    or a trybuild directory with committed `.stderr` files (tokio's
    `extras/tokio/tests-build/tests/fail`) is the cheapest regression net for clap derive misuse and
    public type guarantees.
11. Add a `minimal-versions` CI job (`cargo hack --remove-dev-deps` then
    `cargo update -Z minimal-versions` then `cargo check`) so declared dependency lower bounds in
    quinjet's manifest stay honest.
12. Keep the release checklist as a comment above `version =` in `Cargo.toml`, keep a categorized
    CHANGELOG with PR links, and add tokio's `check-readme`-style job that greps the README for the
    current manifest version.
13. Enforce prose quality mechanically: quinjet already runs typos; borrow the sorted-dictionary
    validation shell from tokio's `check-spelling` job and the repo-wide trailing whitespace grep as
    one cheap CI step.
14. Steal the `stress-test` idea at binary scale: run one representative quinjet subcommand sequence
    under `valgrind --error-exitcode=1 --leak-check=full` in CI, compiled release, as an end-to-end
    leak and crash canary for the CLI surface.
