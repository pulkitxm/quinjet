# nushell/nushell (40272 stars)

## 1. What the project is and how big it is

Nushell is a cross-platform shell and programming language in which every pipeline carries
structured data (records, tables, streams) instead of raw text. Industry uses it as a daily
driver shell, as a scripting language for CI and data plumbing, and as an embeddable engine:
the workspace exposes the parser, evaluator, and value model as separately published crates
(`nu-parser`, `nu-engine`, `nu-protocol`), and a stable plugin protocol lets third parties
ship out-of-process commands. The root manifest states the intent plainly
(extras/nushell/Cargo.toml):

```toml
description = "A new type of shell"
documentation = "https://www.nushell.sh/book/"
name = "nu"
```

Scale indicators measured directly from the clone (commit `3876934`, 2026-08-15):

- 1,852 `.rs` files totaling about 418,000 lines of Rust text (`wc -l` over all sources).
- 46 directories under extras/nushell/crates; 41 crates are explicit workspace members in
  extras/nushell/Cargo.toml, and `nu-glob` and `nu-path` join the workspace implicitly as
  path dependencies.
- Two additional cargo-fuzz crates deliberately opt out of the workspace
  (extras/nushell/crates/nu-parser/fuzz, extras/nushell/crates/nu-path/fuzz).
- Three non-Rust reference plugins document the wire protocol in other languages
  (extras/nushell/crates/nu_plugin_python, nu_plugin_javascript, nu_plugin_nu_example).
- Workspace-wide version `0.115.0`, edition 2024, `rust-version = "1.95.0"`.
- 304 Rust files under `crates/*/tests/`, 256 of them in `nu-command` alone.

## 2. Repository layout

```text
extras/nushell/
|-- Cargo.toml           workspace root AND the `nu` binary package
|-- rust-toolchain.toml  pinned channel with MSRV policy explained in comments
|-- rustfmt.toml         one line: edition
|-- clippy.toml          unwrap-in-tests + disallowed-types
|-- typos.toml           spell-check config with TUI-artifact ignores
|-- sgconfig.yml         ast-grep project file (custom structural lints)
|-- toolkit.nu           entry point for the contributor toolkit
|-- toolkit/             fmt/clippy/test/coverage/package commands, git hooks
|-- ast-grep/            rules/, utils/, tests/ with __snapshots__
|-- benches/             tango-bench benchmark suite
|-- crates/              41+ member crates: nu-* libraries, nu_plugin_* plugins
|-- devdocs/             rust_style.md, FAQ.md, HOWTOS.md, PLATFORM_SUPPORT.md
|-- docker/              Dockerfiles and a docker test script
|-- scripts/             build.rs, coverage, cross-build helpers, nix
|-- src/                 the thin `nu` binary (main.rs, run.rs, signals.rs, ...)
|-- tests/               end-to-end suite for the `nu` binary
|-- wix/                 Windows MSI packaging (WiX), terminal profile
|-- .githooks/           pre-commit (fmt), pre-push (fmt + clippy)
`-- .github/             workflows, dependabot, issue/PR templates, labeler
```

The split works because the root package is only glue: `src/main.rs` wires the crates
together and everything of substance lives in a purpose-scoped crate. The taxonomy is
written down in extras/nushell/crates/README.md:

```text
Foundational libraries are split into two kinds of crates:

* Core crates - those crates that work together to build the Nushell language engine
* Support crates - a set of crates that support the engine with additional features
```

Naming carries meaning: `nu-*` (hyphen) is a library, `nu_plugin_*` (underscore) is an
executable plugin, and `testbins` holds tiny helper binaries used only by tests
(extras/nushell/crates/testbins/Cargo.toml sets `publish = false`).

## 3. Cargo manifest practices

The root extras/nushell/Cargo.toml is a model of `workspace.package` inheritance. Shared
fields are declared once and every crate pulls them:

```toml
[workspace.package]
authors = ["The Nushell Project Developers"]
edition = "2024"
rust-version = "1.95.0"
license = "MIT"
version = "0.115.0"
```

Every member (see extras/nushell/crates/nu-protocol/Cargo.toml) repeats only
`authors.workspace = true`, `edition.workspace = true`, etc. All 34 library crates also
contain a two-line `[lints]` table pointing at the workspace definition.

Notable manifest practices, all in extras/nushell/Cargo.toml:

- Every internal crate appears in `[workspace.dependencies]` with `path`, an explicit
  `version = "0.115.0"`, and `default-features = false`, so crates compose features
  explicitly and publishing to crates.io works from the same manifest.
- External dependencies are centralized and alphabetized; member manifests say only
  `thiserror = { workspace = true }`.
- Version pins carry rationale as comments. The TLS stack is held to a tilde range:

```toml
# We have to semi-fix rustls and ureq versions
# because we use unversioned api to allow users set up their own
# crypto providers (grep for "unversioned").
# Patch updates are allowed though.
rustls = { version = "~0.23.38", default-features = false, features = ["std", "tls12"] }
```

  and exact pins like `trash = "=5.2.6"` and `fff-search = { version = "=0.10.3", ... }`
  mark crates where any drift is known to break behavior.

- Feature design is layered: a `default` set, a `stable = ["default"]` alias, and a `full`
  set documented as "Enable all features while still avoiding mutually exclusive features.
  Use this if `--all-features` fails." The `plugin` feature fans out with `dep:` syntax
  across nine crates, keeping optional dependencies invisible unless enabled.
- Mutually exclusive TLS backends (`rustls-tls` vs `native-tls`) are not just documented,
  they are machine-checked by cargo-hack in CI (section 6).
- Profiles: `[profile.release]` uses `opt-level = "s"`, `strip = "debuginfo"`, and
  `lto = "thin"` (a shell must start fast and stay small); a `profiling` profile inherits
  release with `debug = true` for `perf`; a `ci` profile inherits dev with `debug = false`
  to shrink test artifacts.
- `autotests = false` plus an explicit `[[test]] harness = false` block routes all
  integration tests through one custom-harness binary (section 7). The lib and bin set
  `bench = false` so `cargo bench` only sees the tango harness.
- `[package.metadata.binstall]` teaches `cargo binstall` the release-asset URL scheme, and
  `[package.metadata.winresource]` embeds Windows file metadata.
- An empty commented `[patch.crates-io]` section is kept on purpose: "To use a development
  version of a dependency please use a global override here".

MSRV policy lives in extras/nushell/rust-toolchain.toml as prose next to the pin:

```toml
# The current plan is to be 2 releases behind the latest stable release.
channel = "1.95.0"
```

and CI enforces that this file and `workspace.package.rust-version` never drift, using a
nushell script (extras/nushell/.github/workflows/check-msrv.nu) that opens both TOML files
and exits 1 on mismatch.

## 4. Formatting

extras/nushell/rustfmt.toml is a single line:

```toml
edition = "2024"
```

The project deliberately runs stock rustfmt with zero styling opinions, which removes all
formatting debate and guarantees any contributor's editor produces identical output. It is
enforced three times: locally via `toolkit fmt`, at commit time by
extras/nushell/.githooks/pre-commit (`fmt --check --verbose`), and in CI by
`cargo fmt --all --check` (extras/nushell/.github/workflows/ci.yml).

There is no `.editorconfig` in the repository (verified by listing the root). Non-Rust
hygiene is handled by `typos` instead of a formatter: extras/nushell/typos.toml excludes
fixture-heavy paths and uses regex ignores for strings that only look like typos, such as
box-drawing fragments from table output:

```toml
extend-ignore-re = [
    "Plasticos Rival",
    "│ in_custom_valu │",
    "([0-9a-f][0-9a-f] ){4}",
]
```

This is the correct tool split for a project whose test fixtures contain deliberately
mangled text.

## 5. Linting

Lint policy lives in three cooperating places.

First, `[workspace.lints]` in extras/nushell/Cargo.toml, inherited by all 34 crates that
declare `[lints] workspace = true`:

```toml
[workspace.lints.clippy]
# Warning: workspace lints affect library code as well as tests, so don't enable lints that would be too noisy in tests like that.
format_push_string = "warn"
needless_raw_strings = "warn"
result_large_err = "allow"
unchecked_time_subtraction = "deny"
unwrap_used = "deny"
used_underscore_binding = "warn"
```

The list is short and every deviation is annotated: `collapsible_match` is allowed with a
link to a rustc issue and a planned removal version, and `filter_map_identity` is allowed
with a performance rationale. `unexpected_cfgs` is configured with
`check-cfg = ["cfg(ci)"]` so the custom `--cfg ci` flag (section 6) stays legal.

Second, extras/nushell/clippy.toml softens the wall exactly where it should and hardens it
where clippy cannot reach by default:

```toml
allow-unwrap-in-tests = true 

[[disallowed-types]]
path = "std::time::Instant"
reason = "WASM panics if used, use instead"
replacement = "nu_utils::time::Instant"
```

`disallowed-types` turns an architectural decision (WASM support) into a compiler error
with a suggested replacement. The one legitimate use site opts out with a reasoned allow
(extras/nushell/crates/nu-utils/src/time.rs):

```rust
#![allow(
    clippy::disallowed_types,
    reason = "only allow std::time::Instant here when it's not WASM"
)]
```

Third, severity is escalated at invocation time rather than in the manifest. CI exports
`CLIPPY_OPTIONS: "-D warnings"` (extras/nushell/.github/workflows/ci.yml) and the local
aliases in extras/nushell/.cargo/config.toml mirror it, with a softer profile for tests:

```toml
nuclippy = "clippy --workspace --exclude nu_plugin_* --profile ci --all-targets -- -D warnings -D clippy::unwrap_used -D clippy::unchecked_time_subtraction"
# Clippy just for tests
nuclippy-tests = "clippy --workspace --tests --exclude nu_plugin_* --profile ci --all-targets -- -D warnings -D clippy::unchecked_time_subtraction"
```

The philosophy behind the wall is written down in extras/nushell/devdocs/rust_style.md:
conservative reliance on clippy, an outright ban on `.unwrap()` outside tests, panics
disallowed for anything reachable from user input, nightly features prohibited, and custom
macros discouraged unless they beat functions on readability and compile time.

Beyond clippy, nushell maintains custom structural lints with ast-grep.
extras/nushell/sgconfig.yml points at extras/nushell/ast-grep/rules, where each rule is a
small YAML program with a severity, an explanation, and often an autofix. Example
(extras/nushell/ast-grep/rules/internal_span.yml):

```yaml
id: internal_span
severity: error
message: "Using `internal_span` directly is deprecated."
note: "You can get the span using the `Value::span()` method."
```

The `if-matches` rule even rewrites `if matches!(v, pat)` into `if let pat = v` via
`fix: "let $$$PAT = $VAL"`, and the rules themselves have snapshot tests under
extras/nushell/ast-grep/tests/__snapshots__. This is lint infrastructure as reviewable,
tested code.

## 6. CI/CD

All CI lives in extras/nushell/.github/workflows (12 files, 1,061 lines total).

`ci.yml` is the gate. Triggers: `pull_request`, pushes to `main` and `patch-release-*`,
plus `pull_request_target` on `ready_for_review`. Global `permissions: contents: read`
and a `concurrency` group with `cancel-in-progress: true` cap cost and token power. Draft
PRs and lower layers of stacked PRs are skipped with a job-level `if` on
`github.event.pull_request.draft` and `pull_request.stack.position`.

The heart is a two-dimensional matrix: targets (Ubuntu 22.04, Windows, macOS, and
`wasm32-unknown-unknown`) crossed with workspaces (the root plus both fuzz crates), where
each cell declares which steps apply:

```yaml
- name: Ubuntu
  host: ubuntu-22.04
  target: x86_64-unknown-linux-gnu
  options: MAIN_OPTIONS
  steps: [fmt, clippy, build, test, doctest]
- name: WASM
  host: ubuntu-22.04
  target: wasm32-unknown-unknown
  options: WASM_OPTIONS
  steps: [build, check]
```

Each step then guards itself with `if: contains(matrix.target.steps, 'fmt') && ...`, so one
job template serves every combination, including clippy on the fuzz crates that are outside
the workspace. Details worth stealing:

- Ubuntu is pinned to 22.04 with an inline comment explaining glibc compatibility for
  released binaries and a revisit date (22.04 EOL, June 2027).
- Caching is `Swatinem/rust-cache` with `cache-all-crates: true` and per-workspace
  `workspaces: path -> target` mapping.
- Every third-party action is pinned to a full commit SHA with the human version in a
  comment, e.g. `actions/checkout@3d3c42e5aac5... # v7.0.1`, and
  extras/nushell/.github/dependabot.yml updates the `github-actions` ecosystem weekly so
  pins do not rot.
- Tests run with `--cfg ci` injected through `cargo --config .cargo/ci.toml`
  (extras/nushell/.cargo/ci.toml), letting tests detect CI without env-var sniffing while
  `unexpected_cfgs` keeps the cfg namespace honest.
- The final step of every job is an anti-drift check: `Assert Clean Repo` runs
  `git diff --quiet && git diff --cached --quiet`, failing if any build step mutated the
  tree.

A second job in `ci.yml` installs the freshly built `nu`, runs the standard-library test
framework written in nushell itself (`nu -c 'use crates/nu-std/testing.nu; ...'`), runs the
MSRV consistency script, exercises Python virtualenv integration, and uploads the built
binaries as 14-day artifacts so reviewers can download a PR build without compiling.

The satellite workflows divide responsibilities:

- `audit.yml`: rustsec/audit-check on any `Cargo.toml`/`Cargo.lock` change, with
  `continue-on-error: true` and the comment "Prevent sudden announcement of a new advisory
  from failing ci". Accepted advisories live in extras/nushell/.cargo/audit.toml with
  justifications.
- `typos.yml`: crate-ci/typos on every PR.
- `beta-test.yml`: a daily cron that runs the whole test suite on the beta toolchain,
  `continue-on-error`, explicitly framed as testing the compiler rather than nushell, with
  a wry note that failure notifications go to whoever last edited the cron line.
- `pre-release-checkup.yml`: manual `workflow_dispatch` running cargo-hack
  `--feature-powerset` with `--mutually-exclusive-features rustls-tls,native-tls`, proving
  the feature matrix before a release.
- `labels.yml` (actions/labeler with `sync-labels`), `milestone.yml` (binds merged PRs and
  fixed issues to the active milestone), and `friendly-config-reminder.yml` (posts a
  deduplicated bot comment when files under `crates/nu-protocol/src/config/**` change,
  reminding authors to update the user-facing `doc_config.nu`).
- `nightly-build.yml`: a cron at 00:15 UTC that force-syncs a separate `nushell/nightly`
  repository, rewrites the version in every `Cargo.toml` to `X.Y.Z-nightly.N`, tags with
  semver build metadata (`0.115.0-nightly.3+abc1234`), skips the run when the tip hash
  already shipped, and files a templated issue
  (extras/nushell/.github/AUTO_ISSUE_TEMPLATE/nightly-build-fail.md) when the build fails.
- `release.yml`, `release-msi.yml`, `winget-submission.yml`: covered in section 11.

There is no merge queue (`merge_group` appears nowhere in the workflows); the stacked-PR
skip condition and required checks on `pull_request` carry that load. A distinctive trait
throughout: CI logic is written in nushell (`shell: nu {0}`, `hustcer/setup-nu`), so the
project dogfoods its own product in its own pipelines.

## 7. Testing

Nushell replaced the default libtest harness across the workspace. The root manifest routes
everything through one binary (`[[test]] name = "tests" path = "tests/main.rs"
harness = false`, with `autotests = false`), and extras/nushell/tests/main.rs is just a
module list ending in:

```rust
#[macro_use]
extern crate nu_test_support;
use nu_test_support::harness::main;
```

The harness (extras/nushell/crates/nu-test-support/src/harness/mod.rs) is built on the
`kitest` runner plus `linkme` distributed slices: a proc macro in
extras/nushell/crates/nu-test-support-macros/src/test.rs re-implements `#[test]` and
registers each function into a linker section:

```rust
#[::nu_test_support::collect_test(::nu_test_support::harness::TESTS)]
```

so tests keep the familiar `#[test]` spelling while gaining attributes the stock harness
cannot offer. The crate-level docs in
extras/nushell/crates/nu-test-support/src/lib.rs enumerate them: `#[serial]` for
sequential execution, `#[env(FOO = "bar")]` for per-test environment, `#[exp(...)]` to
enable an experimental option, and `#[deps(NU)]` to declare binary dependencies the harness
builds before the filtered test set runs. Tests with identical environment groups run in
parallel; conflicting ones are grouped.

Layers of the pyramid, all on disk:

- Unit tests live beside code (many crates set `[lib] harness = false` too, e.g.
  extras/nushell/crates/nu-protocol/Cargo.toml).
- Integration tests live in `crates/*/tests/` (304 files) and in extras/nushell/tests for
  the binary itself, organized by domain (`repl`, `hooks`, `overlays`, `plugin_persistence`).
- The in-process `NuTester` (extras/nushell/crates/nu-test-support/src/tester/mod.rs)
  evaluates scripts against a cached, cloned `EngineState` (a custom `KeyedLazyLock` keyed
  by environment group) instead of spawning the binary, which the docs call out as the main
  speed win; assertions flow through `IntoValue`/`FromValue`.
- The `Playground` (extras/nushell/crates/nu-test-support/src/playground/play.rs) gives
  filesystem sandboxing: a `TempDir` root, fixture dirs, and per-test env vars.
- Examples are tests: every command implements `examples()` returning
  `Example { example, description, result: Option<Value> }`
  (extras/nushell/crates/nu-protocol/src/example.rs), and
  extras/nushell/crates/nu-cmd-lang/src/example_support.rs evaluates each example and
  additionally checks that its observed input/output types are a subtype of the declared
  signature types. Documentation, type declarations, and behavior can never drift apart.
- Property testing: `#[quickcheck]` feeds arbitrary strings through the lexer and parser
  (extras/nushell/crates/nu-cmd-lang/src/parse_const_test.rs).
- Fuzzing: two cargo-fuzz crates with four targets; the parser one is three lines of logic
  (extras/nushell/crates/nu-parser/fuzz/fuzz_targets/parse.rs) plus a seed-gathering
  script, and CI builds and clippy-checks the fuzz crates so they never bit-rot.
- Snapshot testing appears where it earns its keep: the ast-grep rules have
  `__snapshots__` (extras/nushell/ast-grep/tests); the Rust code itself prefers exact
  `Example` results over snapshots.
- Benchmarks use tango-bench (paired, statistically robust benchmarking) in
  extras/nushell/benches/benchmarks.rs with `harness = false`.
- Coverage comes from cargo-llvm-cov via `toolkit cov` and
  extras/nushell/scripts/coverage-local.nu, which builds with the `ci` profile to keep
  binaries small.
- End-to-end, the public surface is tested twice: the stdlib suite runs under the real
  installed `nu` in CI, and `assert_cmd` is a dev-dependency of the root for process-level
  checks. `toolkit check pr --fast` chains fmt, clippy, tests (optionally via
  cargo-nextest), and stdlib tests into the exact PR gate contributors run locally
  (extras/nushell/toolkit/checks.nu).

## 8. Error handling and API design

The error architecture is thiserror + miette, with zero anyhow in the core paths.
`ShellError` (extras/nushell/crates/nu-protocol/src/errors/shell_error/mod.rs) is a large
documented enum where every variant carries a stable diagnostic code, labeled spans, and
optional help:

```rust
#[derive(Debug, Clone, Error, Diagnostic, PartialEq)]
pub enum ShellError {
    #[error("The '{op}' operator does not work on values of type '{unsupported}'.")]
    #[diagnostic(code(nu::shell::operator_unsupported_type))]
    OperatorUnsupportedType {
        op: Operator,
        unsupported: Type,
        #[label = "does not support '{unsupported}'"]
        op_span: Span,
        ...
```

Variant doc comments include `## Resolution` sections telling users how to fix the
condition, so the error type is simultaneously the user manual. Parsing has its own
`ParseError`, plugins get `LabeledError`, and `ChainedError` composes causes; the split
keeps each layer's failure vocabulary closed and exhaustive.

The panic policy is explicit (extras/nushell/devdocs/rust_style.md): "The use of
`.unwrap()` is thus outright banned", enforced by `unwrap_used = "deny"` and relaxed only
in tests by `allow-unwrap-in-tests`. Where the binary must still fear panics, it installs a
hardened hook (extras/nushell/src/main.rs): a custom `Panic` diagnostic mirrors miette's
backtrace help text, the hook first calls `crossterm::terminal::disable_raw_mode()` as a
best-effort terminal restore, and it reports via `writeln!(io::stderr(), ...)` rather than
`eprintln!` because the print macros themselves panic on a closed pipe, which would
escalate a clean shutdown into an abort.

Exit discipline is modeled, not improvised. `ExitStatus`
(extras/nushell/crates/nu-system/src/exit_status.rs) distinguishes `Exited(i32)` from
`Signaled { signal, core_dumped }` on Unix and maps signals to negative codes, and
`cleanup_exit` (extras/nushell/crates/nu-engine/src/exit.rs) refuses to kill background
jobs on the first `exit` in an interactive session, warning instead and only exiting on the
second attempt.

API construction favors builders and newtypes. Command signatures are fluent builders
(extras/nushell/crates/nu-cmd-lang/src/core_commands/if_.rs):

```rust
Signature::build("if")
    .input_output_types(vec![(Type::Any, Type::Any)])
    .required("cond", SyntaxShape::MathExpression, "Condition to check.")
    .category(Category::Core)
```

Visibility is disciplined: harness internals are `pub(crate)`, macro plumbing is
re-exported under `#[doc(hidden)]` (extras/nushell/crates/nu-test-support/src/harness/mod.rs),
and `Id::get` documents that extracting the raw value "requires an explicit call, ensuring
we only use the raw value when intended".

## 9. Deep Rust usage

Ten-plus concrete idioms, each cited:

1. Phantom-typed IDs. `Id<M, V = usize>` wraps an index with a zero-sized marker so
   `DeclId`, `VarId`, `BlockId`, and friends cannot be confused, while `Debug` prints the
   marker name via `any::type_name::<M>()` (extras/nushell/crates/nu-protocol/src/id.rs):

   ```rust
   pub struct Id<M, V = usize> {
    inner: V,
    _phantom: PhantomData<M>,
   }
   ```

2. Typestate paths. `Path<Form>` in extras/nushell/crates/nu-path/src/path.rs uses
   `#[repr(transparent)]` over `std::path::Path` plus `RefCastCustom` so
   `RelativePath`/`AbsolutePath`/`CanonicalPath` are free coercions, and the type system
   forces callers to join relative paths onto an absolute base before touching `std` APIs
   that would consult the real process cwd.

3. Zero-copy with `Cow`. `strip_trailing_slash(path: &Path) -> Cow<'_, Path>` allocates
   only when a slash actually needs removing
   (extras/nushell/crates/nu-path/src/trailing_slash.rs), and `ArgType<'a>` in the
   `Command` trait uses `Cow<'a, str>` for flag names
   (extras/nushell/crates/nu-protocol/src/engine/command.rs).

4. Cheap cancellation with cold error paths. `Signals` is an `Option<Arc<AtomicBool>>`
   whose hot-loop check marks the failure branch `#[cold]` so the interrupt error
   construction never pollutes the fast path
   (extras/nushell/crates/nu-protocol/src/pipeline/signals.rs):

   ```rust
   #[inline]
   pub fn check(&self, span: &Span) -> Result<(), ShellError> {
    #[inline]
    #[cold]
    fn interrupt_error(span: &Span) -> Result<(), ShellError> {
        Err(ShellError::Interrupted { span: *span })
    }
   ```

5. Copy-on-write global state. `EngineState` stores large objects in `Arc` and mutates via
   `Arc::make_mut`, documented right on the struct
   (extras/nushell/crates/nu-protocol/src/engine/engine_state.rs): "Many of the larger
   objects in this structure are stored within `Arc` to decrease the cost of cloning
   `EngineState`." Parse-time additions accumulate in a `StateDelta` inside
   `StateWorkingSet` and merge back atomically, so evaluation always sees a consistent
   snapshot.

6. Streaming as a first-class enum. `PipelineData` distinguishes `Empty`, `Value`,
   `ListStream`, and `ByteStream`, and its doc comment records the two rejected designs
   (always-stream, and stream-inside-Value) with the concrete aliasing and locking problems
   each caused (extras/nushell/crates/nu-protocol/src/pipeline/pipeline_data.rs). Design
   history as rustdoc is rare and valuable.

7. Object-safe plugin surface. `pub trait Command: Send + Sync + CommandClone + Any` uses
   a clone-helper supertrait to keep `Box<dyn Command>` cloneable and `Any` for downcasts,
   the classic dyn-safe-clone idiom (extras/nushell/crates/nu-protocol/src/engine/command.rs).

8. Proc macros engineered for testability. `nu-derive-value` implements
   `#[derive(IntoValue, FromValue)]`, works internally on `proc_macro2::TokenStream` so
   macro output can be unit tested, and documents its hygiene strategy: generated code is
   deliberately obtuse so "no other code may influence this generated code or vice versa"
   (extras/nushell/crates/nu-derive-value/src/lib.rs).

9. Link-time registration. The test harness collects tests through `linkme`
   distributed slices written by the custom `#[test]` proc macro
   (extras/nushell/crates/nu-test-support-macros/src/test.rs), avoiding any central
   registry file that every module would have to touch.

10. Platform cfg dispatch by module. `nu-system` keeps one file per OS and re-exports a
    uniform surface (extras/nushell/crates/nu-system/src/lib.rs):

    ```rust
    #[cfg(target_os = "freebsd")]
    mod freebsd;
    #[cfg(any(target_os = "android", target_os = "linux"))]
    mod linux;
    #[cfg(target_os = "macos")]
    mod macos;
    ```

11. Unsafe with receipts. 32 `// SAFETY:` comments across the tree; the foreground
    process code documents async-signal-safety of `setsid` against POSIX signal-safety(7)
    before calling it in a pre-exec hook
    (extras/nushell/crates/nu-system/src/foreground.rs). The written policy in
    extras/nushell/devdocs/rust_style.md demands exactly this.

12. Edition-2024 let chains used for clarity, not novelty:
    `if let Some(suggestion) = &suggestion && suggestion.len() == 1 && ...`
    (extras/nushell/crates/nu-protocol/src/did_you_mean.rs), whose generic signature
    `I: IntoIterator<Item = &'a S>, S: AsRef<str> + 'a + ?Sized` is also a textbook
    borrow-friendly bound.

13. Enforced abstraction loops. The WASM-safe `Instant` newtype
    (extras/nushell/crates/nu-utils/src/time.rs) exists specifically because
    `web_time`'s re-export defeated `clippy::disallowed-types`; nushell wrapped it so the
    lint could police the whole codebase again. Tooling and API design reinforcing each
    other.

14. Modern sync primitives: `LazyLock`/`OnceLock` statics and `parking_lot::const_rwlock`
    in the tester (extras/nushell/crates/nu-test-support/src/tester/mod.rs),
    `crossbeam-channel` and job mailboxes in the engine state
    (extras/nushell/crates/nu-protocol/src/engine/engine_state.rs).

## 10. Documentation practices

- 19 crates begin their `lib.rs` with `#![doc = include_str!("../README.md")]`, so the
  crates.io README and the rustdoc front page are one artifact
  (e.g. extras/nushell/crates/nu-system/src/lib.rs).
- Long-form module docs teach workflows, not just APIs: the test-support crate's docs are a
  complete tutorial on adopting the custom harness, including the exact `Cargo.toml`
  stanzas to copy (extras/nushell/crates/nu-test-support/src/lib.rs), and `nu-experimental`
  documents user-facing flags, env-var syntax, and embedder guidance in one place
  (extras/nushell/crates/nu-experimental/src/lib.rs).
- Doctests are CI-enforced (`cargo test --workspace --doc` step in
  extras/nushell/.github/workflows/ci.yml), so examples cannot rot.
- Contributor docs are split by audience: extras/nushell/CONTRIBUTING.md (327 lines) for
  process, extras/nushell/devdocs for engineering policy (rust_style.md, FAQ.md,
  HOWTOS.md, PLATFORM_SUPPORT.md, release_notes_generation.md).
- The PR template (extras/nushell/.github/pull_request_template.md) contains a
  "User-facing changes (Release notes)" section that is harvested nearly verbatim for the
  release blog, and CONTRIBUTING.md documents heading conventions and a `notes:ready`
  label workflow around it. Release notes become a review artifact, not an afterthought.
- Issue templates are structured YAML forms, including a dedicated
  `experimental_option.yml` for feedback on gated features
  (extras/nushell/.github/ISSUE_TEMPLATE).
- Governance files are present and current: extras/nushell/SECURITY.md,
  extras/nushell/CODE_OF_CONDUCT.md, and extras/nushell/CITATION.cff for academic citation.

## 11. Release and distribution

Versioning is lockstep: every crate ships `0.115.0` via `workspace.package.version`, and
nightly builds append semver metadata (`0.115.0-nightly.N+shorthash`). The cadence is
encoded in an unexpected place, extras/nushell/.github/dependabot.yml:

```yaml
# We release on Tuesdays and open dependabot PRs will rebase after the
# version bump and thus consume unnecessary workers during release, thus
# let's open new ones on Wednesday
day: "wednesday"
```

The pipeline (extras/nushell/.github/workflows/release.yml) triggers on semver tags,
builds 13 targets including `riscv64gc` and both gnu and musl `loongarch64`, produces
Windows MSIs with WiX 6 (extras/nushell/wix/main.wxs), publishes everything as a draft
release, and a dependent job downloads all assets and publishes a `SHA256SUMS` file. The
packaging logic itself is a nushell script
(extras/nushell/.github/workflows/release-pkg.nu) whose header doubles as a step-by-step
manual for rebuilding an MSI by hand when automation fails. Post-release,
`winget-submission.yml` submits to the Windows Package Manager repo automatically, MSI-only
via `installers-regex: 'msvc\.msi$'`. `cargo binstall` support comes free from the
binstall metadata in extras/nushell/Cargo.toml, extras/nushell/Cross.toml documents
cross-rs builds for ARM/musl, and extras/nushell/docker provides Dockerfiles. Changelog
discipline is the PR-template release-notes section plus milestone automation
(extras/nushell/.github/workflows/milestone.yml) feeding the generation process described
in extras/nushell/devdocs/release_notes_generation.md. As nushell is itself a shell, it
ships no external completions or man pages; its help system and `wix/windows-terminal-profile.json`
cover that role.

## 12. Lessons for quinjet

quinjet already has a strict clippy wall, rustfmt, cargo-deny, taplo, typos, a coverage
floor, miri, and mutants. What nushell still adds, with exact mechanisms:

1. Terminal-safe panic hook. Register `std::panic::set_hook` in `main` that first calls
   `crossterm::terminal::disable_raw_mode()` (and leaves the alternate screen), then writes
   the report with `writeln!(io::stderr(), ...)` instead of `eprintln!` so a closed pty
   cannot escalate into an abort; model it on extras/nushell/src/main.rs. For a ratatui
   binary this is the single highest-value item in this chapter.

2. Structural lints with autofixes. Add `sgconfig.yml` plus an `ast-grep/rules/` directory
   and run `ast-grep scan` in the Makefile and CI; encode quinjet-specific bans (for
   example "no direct `Command::new("git")` outside the git module") the way
   extras/nushell/ast-grep/rules/internal_span.yml bans a field access, and snapshot-test
   the rules under `ast-grep/tests`.

3. `clippy.toml` `[[disallowed-types]]` and `disallowed-methods` with `reason` and
   `replacement` keys to make architectural rules compiler-enforced, per
   extras/nushell/clippy.toml.

4. Examples as tests for the CLI surface. Give every clap subcommand an
   `examples() -> Vec<Example>` with expected output and a harness that executes each
   example and asserts the result, like
   extras/nushell/crates/nu-cmd-lang/src/example_support.rs; help text, docs, and behavior
   then cannot diverge.

5. An "Assert Clean Repo" CI step (`git diff --quiet && git diff --cached --quiet`) after
   build and test, catching generated-file drift, from
   extras/nushell/.github/workflows/ci.yml.

6. MSRV consistency gate: pin the toolchain in `rust-toolchain.toml`, set
   `package.rust-version`, and add a CI step that fails on mismatch, like
   extras/nushell/.github/workflows/check-msrv.nu.

7. Fuzz the parsers. Create `fuzz/` cargo-fuzz crates (own `[workspace]` table to stay out
   of the main workspace) for anything quinjet parses (git porcelain output, refspecs,
   config), three-line targets like
   extras/nushell/crates/nu-parser/fuzz/fuzz_targets/parse.rs, and include the fuzz crates
   in the CI clippy/check matrix so they compile forever.

8. Property tests with `quickcheck`/`quickcheck_macros` for "never panics on arbitrary
   input" invariants, mirroring extras/nushell/crates/nu-cmd-lang/src/parse_const_test.rs.

9. Scheduled beta-toolchain job: a daily cron workflow running
   `cargo +beta test` with `continue-on-error: true`
   (extras/nushell/.github/workflows/beta-test.yml) to see compiler breakage weeks early.

10. Security audit workflow: `rustsec/audit-check` triggered on `Cargo.toml`/`Cargo.lock`
    paths with `continue-on-error: true` and an `.cargo/audit.toml` ignore list where every
    entry carries a justification comment (extras/nushell/.github/workflows/audit.yml);
    this complements cargo-deny with issue-filing on scheduled findings.

11. Cargo profile hygiene: add a `profiling` profile (`inherits = "release"`,
    `debug = true`, `strip = false`) for perf work and a `ci` profile
    (`inherits = "dev"`, `debug = false`) to shrink CI artifacts, plus release
    `opt-level = "s"`, `lto = "thin"`, `strip = "debuginfo"` for a small fast binary,
    all from extras/nushell/Cargo.toml.

12. `cfg(ci)` done right: inject `rustflags = ["--cfg", "ci"]` via a checked-in
    `.cargo/ci.toml` passed as `cargo --config`, and allowlist it with
    `unexpected_cfgs = { level = "warn", check-cfg = ["cfg(ci)"] }`
    (extras/nushell/.cargo/ci.toml and the `[workspace.lints.rust]` table).

13. Distribution polish: add `[package.metadata.binstall]` so `cargo binstall quinjet`
    works from GitHub releases, and a `SHA256SUMS` job that downloads all release assets
    and publishes checksums, both modeled on extras/nushell/Cargo.toml and
    extras/nushell/.github/workflows/release.yml.

14. Cold-path interrupt checks: if quinjet grows long-running operations, copy the
    `Signals` shape (`Option<Arc<AtomicBool>>`, `#[inline]` check with a `#[cold]` inner
    error constructor) from extras/nushell/crates/nu-protocol/src/pipeline/signals.rs.

15. Repo-local git hooks without a framework: a `.githooks/` directory activated by
    `git config --local core.hooksPath .githooks`, with pre-commit running the fmt check
    and pre-push running fmt plus clippy (extras/nushell/.githooks,
    extras/nushell/toolkit/git-hooks.nu); cheap, versioned, and opt-in.

16. Paired benchmarking with `tango-bench` (`harness = false` `[[bench]]`, `bench = false`
    on lib and bin) for statistically trustworthy regression detection on hot paths like
    diff rendering, per extras/nushell/benches/benchmarks.rs and the root manifest.
