# bevyengine/bevy (47648 stars)

## 1. What Bevy is and how big it is

Bevy is a data-driven game engine and application framework built around an
archetypal entity component system (ECS). The root manifest describes it
plainly in `extras/bevy/Cargo.toml`:

```toml
[package]
name = "bevy"
version = "0.20.0-dev"
edition = "2024"
description = "A refreshingly simple data-driven game engine and app framework"
rust-version = "1.96.0"
```

Industry and hobbyists use it because the ECS core (`bevy_ecs`) is usable as a
standalone library, the engine is fully modular (every subsystem is a plugin),
and the project maintains one of the most disciplined open-source Rust
workflows in existence. It is a library, not a CLI, so its "public surface" is
an enormous API plus 430 runnable examples.

Measurable scale from the clone:

- 59 published crates under `extras/bevy/crates/` (verified with `ls crates | wc -l`).
- 1846 `.rs` files totaling roughly 625,700 lines; the `crates/` tree alone is
  roughly 513,900 lines.
- 430 `[[example]]` entries declared in `extras/bevy/Cargo.toml`.
- Workspace members beyond `crates/*`: `benches`, `tools/*`, `errors`, three
  `compile_fail` crates, and several example sub-workspaces (mobile, `no_std`,
  large scenes), all listed in the `[workspace] members` array of
  `extras/bevy/Cargo.toml`.

## 2. Repository layout

Top level of `extras/bevy/`:

```text
bevy/
|-- Cargo.toml            root package "bevy" + workspace definition (5673 lines)
|-- clippy.toml           clippy configuration shared by every crate
|-- rustfmt.toml          formatting policy
|-- deny.toml             cargo-deny: advisories, licenses, bans, sources
|-- typos.toml            spell-check configuration
|-- src/lib.rs            thin facade re-exporting bevy_internal
|-- crates/               59 bevy_* crates, each independently publishable
|-- examples/             430 examples, organized by topic (2d, 3d, ecs, ui, ...)
|-- benches/              criterion benchmarks as a dedicated workspace member
|-- tests/                cross-crate integration tests against the `bevy` facade
|-- tests-integration/    consumer-style crates excluded from the workspace
|-- tools/                internal tooling: ci runner, compile_fail_utils, ...
|-- errors/               a crate whose docs are the runtime error code catalog
|-- docs/                 contributor docs: linters.md, profiling.md, debugging.md
|-- docs-rs/              rustdoc HTML extensions (trait tags on docs.rs)
|-- docs-template/        templates for generated docs pages
|-- _release-content/     draft migration guides and release notes for next release
`-- .github/              13 workflows, composite actions, templates, linter config
```

Why the split works: every subsystem is a crate with its own feature set and
MSRV, so downstream users can depend on `bevy_ecs` alone, while the root
`bevy` package is a pure facade. The facade is even split once more:
`extras/bevy/src/lib.rs` re-exports `crates/bevy_internal`, which exists, per
its own docs, "to enable simple dynamic linking for Bevy"
(`extras/bevy/crates/bevy_internal/src/lib.rs`). Tooling lives in-workspace
(`tools/ci`) so it is versioned, reviewed, and compiled by the same toolchain
as the engine.

## 3. Cargo manifest practices

The root `extras/bevy/Cargo.toml` is 5673 lines and is a study in itself.

Workspace definition uses resolver 3 and globs plus explicit exceptions:

```toml
[workspace]
resolver = "3"
members = [
  # All of Bevy's official crates are within the `crates` folder!
  "crates/*",
  "crates/bevy_derive/compile_fail",
  ...
  "benches",
  # Internal tools that are not published.
  "tools/*",
  "errors",
]
exclude = [
  # Integration tests are not part of the workspace
  "tests-integration",
]
```

Notable, and unusual, choices:

- There is no `[workspace.dependencies]` table at all (`grep -c
  "workspace.dependencies" Cargo.toml` returns 0). Every crate declares its
  own versioned dependencies, because each crate is published independently
  and must stand alone on crates.io.
- There is a `[workspace.lints]` table, and each member opts in with `[lints]
  workspace = true` (for example `extras/bevy/crates/bevy_ecs/Cargo.toml`
  line 154). The root package then duplicates the whole lint table because
  cargo cannot override workspace lints per package. The manifest documents
  this honestly:

```toml
# Unfortunately, cargo does not currently support overriding workspace lints
# inside a particular crate. See https://github.com/rust-lang/cargo/issues/13157
#
# We require an override for cases like `std_instead_of_core`, which are intended
# for the library contributors and not for how users should consume Bevy.
```

- MSRV is set per crate: the root declares `rust-version = "1.96.0"` while
  `extras/bevy/crates/bevy_ecs/Cargo.toml` declares `rust-version = "1.95.0"`,
  so the standalone ECS supports an older compiler than the full engine.
- Feature flags are a designed vocabulary, not an accident. Features in the
  root manifest carry structured comment prefixes (`# PROFILE:` and
  `# COLLECTION:`) that a tool parses into documentation:

```toml
# PROFILE: The default 2D Bevy experience. This includes the core Bevy framework, 2D functionality, scenes and picking.
2d = ["default_app", "default_platform", "2d_bevy_render", "scene", "picking"]
```

  In `extras/bevy/crates/bevy_ecs/Cargo.toml`, features use `##` doc comments
  and `dep:` syntax (`serialize = ["dep:serde", "bevy_platform/serialize",
  "indexmap/serde"]`).

- Custom profiles exist for real use cases in `extras/bevy/Cargo.toml`:

```toml
[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = "fat"
codegen-units = 1

[profile.stress-test]
inherits = "release"
lto = "fat"
panic = "abort"
```

- Each of the 430 examples has a `[package.metadata.example.<name>]` block
  with name, description, and category; CI fails if an example lacks metadata
  (section 6).
- `extras/bevy/benches/Cargo.toml` sets `autobenches = false` and registers
  each benchmark target manually, so benchmark discovery is explicit.

## 4. Formatting

`extras/bevy/rustfmt.toml` is deliberately small:

```toml
use_field_init_shorthand = true
newline_style = "Unix"
style_edition = "2021"

# The following lines may be uncommented on nightly Rust.
# Once these features have stabilized, they should be added to the always-enabled options above.
# unstable_features = true
# imports_granularity = "Crate"
# normalize_comments = true
```

Setting by setting: `use_field_init_shorthand` rewrites `Foo { x: x }` to
`Foo { x }`; `newline_style = "Unix"` pins LF endings across Windows
contributors; `style_edition = "2021"` pins formatting behavior independently
of the crate edition (the code is edition 2024) so a toolchain bump cannot
reformat the tree. The commented block is a parked wishlist of nightly-only
options, with an explicit note that two options ("wrap_comments",
"comment_width") are avoided because they "seem poorly implemented and cause
churn". There is no `.editorconfig` in the repository root.

Non-Rust formatting is enforced in CI rather than by local config: TOML by
taplo (`taplo fmt --check --diff` in the `toml` job of
`extras/bevy/.github/workflows/ci.yml`), Markdown by super-linter's
markdownlint with config at `extras/bevy/.github/linters/.markdown-lint.yml`
(line length rule MD013 disabled, `details`/`summary` HTML allowed), and
spelling by `typos` configured in `extras/bevy/typos.toml`, which shows how to
keep a spell checker useful in a technical codebase:

```toml
[default.extend-words]
LOD = "LOD"                             # Level of detail
mis = "mis"                             # mis - multiple importance sampling
```

## 5. Linting

Lint policy lives in three layers.

Layer 1: `[workspace.lints]` in `extras/bevy/Cargo.toml`. The philosophy is a
curated warn-list rather than blanket `pedantic`, combined with `-D warnings`
in CI so every warn is effectively deny:

```toml
[workspace.lints.clippy]
doc_markdown = "warn"
undocumented_unsafe_blocks = "warn"
print_stdout = "warn"
print_stderr = "warn"
ptr_as_ptr = "warn"
alloc_instead_of_core = "warn"
allow_attributes = "warn"
allow_attributes_without_reason = "warn"

[workspace.lints.rust]
missing_docs = "warn"
unsafe_code = "deny"
unsafe_op_in_unsafe_fn = "warn"
unused_qualifications = "warn"
```

Two rules stand out. `unsafe_code = "deny"` at the workspace level forces any
crate that genuinely needs unsafe to opt back in loudly at the crate root, for
example `extras/bevy/crates/bevy_ecs/src/lib.rs`:

```rust
#![expect(unsafe_code, reason = "Unsafe code is used to improve performance.")]
```

And `allow_attributes_without_reason = "warn"` means every `#[allow]` or
`#[expect]` in half a million lines must carry a written justification.

Layer 2: `extras/bevy/clippy.toml` configures lint behavior. It enables doc
linting of private items (`check-private-items = true`), teaches `doc_markdown`
project vocabulary (`doc-valid-idents = ["glTF", "VSync", ...]`), enforces
brace style for a specific macro (`standard-macro-braces = [{ name =
"children", brace = "[" }]`), and, most distinctively, uses
`disallowed-methods` to ban the entire `f32` transcendental surface so that
math is deterministic across platforms:

```toml
disallowed-methods = [
  { path = "f32::powi", reason = "use bevy_math::ops::FloatPow::squared, bevy_math::ops::FloatPow::cubed, or bevy_math::ops::powf instead for libm determinism" },
  { path = "f32::sin", reason = "use bevy_math::ops::sin instead for libm determinism" },
  ...
]
```

This is clippy configuration used as API governance: the lint config funnels
every contributor to the project's own wrapper module.

Layer 3: custom check infrastructure. `extras/bevy/tools/ci` is a workspace
crate (argh + xshell) that encodes every CI command in Rust;
`extras/bevy/tools/ci/src/commands/clippy.rs` runs:

```rust
cmd!(sh, "cargo clippy --workspace --all-targets --all-features {jobs...} -- -Dwarnings")
```

`extras/bevy/deny.toml` extends linting into the dependency graph: a license
allowlist with per-crate exceptions (all `symphonia` crates for MPL-2.0),
`wildcards = "deny"`, `unknown-registry = "deny"`, pinned single-version
requirements for high-blast-radius crates (`ahash`, `glam`,
`raw-window-handle`), and even a per-feature ban:

```toml
# thiserror is the preferred way to derive error types
[[bans.features]]
crate = "derive_more"
deny = ["error"]
```

## 6. CI/CD

There are 13 workflows in `extras/bevy/.github/workflows/`, 2247 lines of YAML
total. Highlights per file:

- `ci.yml` (600 lines, 18 jobs): triggers on `merge_group`, `pull_request`,
  and pushes to `release-*` branches, meaning the project uses the GitHub
  merge queue as the gate to main. Global hardening at the top:

```yaml
permissions:
  contents: read
env:
  CARGO_INCREMENTAL: 0
  CARGO_PROFILE_TEST_DEBUG: 0
  CARGO_PROFILE_DEV_DEBUG: 0
  RUSTFLAGS: "-D warnings"
concurrency:
  group: ${{github.workflow}}-${{github.ref}}
  cancel-in-progress: ${{github.event_name == 'pull_request'}}
```

  Jobs: `build` (tests on a 3-OS matrix: windows, ubuntu, macos), `ci`
  (fmt + clippy via `cargo run -p ci -- lints`), `miri` (`cargo miri test -p
  bevy_ecs` with `RUSTFLAGS: -Zrandomize-layout` and
  `MIRIFLAGS: -Zmiri-disable-isolation`), four `check-compiles*` jobs covering
  `no_std` and portable-atomic targets, `build-wasm` and `build-wasm-atomics`,
  `markdownlint`, `toml` (taplo), `typos`, `check-doc`, two
  `check-missing-*-in-docs` jobs that regenerate templated docs and fail on
  `git diff --quiet`, `msrv` (extracts the MSRV from `cargo metadata` with jq,
  installs exactly that toolchain, runs `cargo check`),
  `check-bevy-internal-imports` (a bash loop failing if any example imports a
  `bevy_*` internal crate directly), and `check-release-content`.

- Every job sets `timeout-minutes`, every checkout sets `persist-credentials:
  false`, and every third-party action is pinned to a full commit SHA with a
  version comment, for example:

```yaml
- uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7.0.0
```

- Caching is asymmetric and clever: PR jobs use `actions/cache/restore` only
  (read-only), while `update-caches.yml` rebuilds caches on pushes to main and
  on a nightly cron for a matrix of OS x toolchain (stable, nightly, MSRV) x
  target (including wasm). PRs therefore never pollute or thrash the cache.
- `dependencies.yml` runs the four `cargo deny check` subcommands, path
  filtered to `**/Cargo.toml` and `deny.toml` changes.
- `security-static-analysis.yml` runs CodeQL for both `rust` and `actions`
  languages, plus zizmor (a GitHub Actions security scanner) uploading SARIF.
- `example-run.yml` runs real examples on macOS Metal, Linux (via `xvfb-run`),
  and Windows DX12, driven by RON config files in
  `extras/bevy/.github/example-run/`, taking screenshots and uploading
  Chrome trace artifacts. `send-screenshots-to-pixeleagle.yml` pushes those
  screenshots to a visual regression service.
- Fork safety pattern: workflows that need write permissions
  (`ci-comment-failures.yml`, `welcome.yml`, `action-on-PR-labeled.yml`) start
  with the comment "This workflow has write permissions on the repo. It must
  not checkout a PR and run untrusted code!" and consume artifacts from the
  untrusted `CI` run via `workflow_run` instead of executing PR code.
- `action-on-PR-labeled.yml` diffs `_release-content/migration-guides` when a
  maintainer applies the `M-Migration-Guide` label and posts an instructional
  comment if the PR did not add a guide, automating changelog discipline.
- `extras/bevy/.github/dependabot.yml` groups related ecosystems (`wgpu` and
  `naga` update together, `accesskit*` together), applies a
  `cooldown: default-days: 7`, and labels PRs `C-Dependencies`.

## 7. Testing

Testing is layered by purpose:

- Unit tests live inline in each crate (`#[cfg(test)]` modules across
  `extras/bevy/crates/*/src`). The main CI test command in
  `extras/bevy/tools/ci/src/commands/test.rs` runs
  `cargo test --workspace --lib --bins --tests --features
  bevy_ecs/track_location` and a second pass with `--benches` "in order to
  verify that they behave correctly and do not panic".
- Facade-level integration tests live in `extras/bevy/tests/`, and two of them
  are executable documentation: `tests/how_to_test_apps.rs` and
  `tests/how_to_test_systems.rs` demonstrate headless testing by swapping
  `DefaultPlugins` for `MinimalPlugins` and injecting fake input resources
  (`app.insert_resource(ButtonInput::<KeyCode>::default())`).
- Consumer tests live outside the workspace: `extras/bevy/tests-integration/`
  is explicitly excluded from `[workspace]` so its crates resolve `bevy` the
  way a real user would. `tests-integration/simple-ecs-test/Cargo.toml`
  even explains why it depends on bevy twice: "We depend on bevy in both
  normal and dev dependencies to verify that the proc macros still work."
- Compile-fail (UI) tests: `crates/bevy_derive/compile_fail`,
  `crates/bevy_ecs/compile_fail`, and `crates/bevy_reflect/compile_fail` are
  dedicated crates with `harness = false` test targets, built on the shared
  helper `extras/bevy/tools/compile_fail_utils/src/lib.rs`, which wraps the
  `ui_test` crate and supports snapshot blessing via a `BLESS` environment
  variable. This pins diagnostic quality: an error message regression fails CI.
- Miri: the `miri` job in `ci.yml` runs the whole `bevy_ecs` test suite under
  the interpreter with randomized layout, which is how a codebase with 1766
  `// SAFETY:` comments in `bevy_ecs` alone keeps its unsafe honest.
- Benchmarks: `extras/bevy/benches/` is a criterion workspace member with a
  `bench!` macro in `benches/src/lib.rs` that derives benchmark names from
  `module_path!()` so names never drift from file locations.
- End-to-end: since the "product" is a rendering engine, e2e testing is the
  `example-run.yml` workflow plus the `bevy_ci_testing` cargo feature. CI sets
  `CI_TESTING_CONFIG=$example` pointing at a RON file from
  `.github/example-run/` (for example `testbed_2d.ron`), runs the example for
  a fixed number of frames, captures screenshots, and compares them via the
  Pixel Eagle visual regression service. Application-level snapshot testing,
  implemented as a first-class engine feature.

## 8. Error handling and API design

- Library errors are `thiserror` enums throughout (`thiserror = { version =
  "2", default-features = false }` in `extras/bevy/crates/bevy_ecs/Cargo.toml`),
  and `deny.toml` structurally bans the competing `derive_more` `error`
  feature so the choice cannot erode.
- The catch-all type is `BevyError` in
  `extras/bevy/crates/bevy_ecs/src/error/bevy_error.rs`: a
  `Box<InnerBevyError>` (one pointer wide on the happy path) with a blanket
  `From<E: Error>`, opt-in backtrace capture behind a `backtrace` feature,
  a `Severity` advisory level, and a `context()` extension in the anyhow
  style. Fallible systems return `Result<(), BevyError>` and a configurable
  `FallbackErrorHandler` resource decides whether to log or panic.
- Runtime panics have a documented catalog: `extras/bevy/errors/src/lib.rs`
  declares one unit struct per error code and attaches the prose with
  `#[doc = include_str!("../B0001.md")]`, and the manifest comment in the
  workspace explains why it is a crate: "This is a crate so we can
  automatically check all of the code blocks."
- Panic policy is tiered: public APIs return `Result`; internal invariants use
  `debug_assert`-style checking via the `DebugCheckedUnwrap` trait
  (`extras/bevy/crates/bevy_ecs/src/query/mod.rs`), which panics in debug
  builds and compiles to `unwrap_unchecked` in release builds.
- Builder patterns are runtime complements to type-level APIs:
  `extras/bevy/crates/bevy_ecs/src/query/builder.rs` defines
  `QueryBuilder<'w, D: QueryData, F: QueryFilter>` with chained
  `.with::<A>().without::<C>().build()`.
- Visibility discipline: the facade chain (`bevy` re-exports `bevy_internal`,
  which gates each `pub use bevy_x as x;` behind its feature flag in
  `extras/bevy/crates/bevy_internal/src/lib.rs`) plus a CI job that rejects
  any example importing `bevy_*` crates directly keeps the public surface to
  exactly one crate.

## 9. Deep Rust usage: cited idioms

1. Newtype with niche optimization:
   `extras/bevy/crates/bevy_ecs/src/entity/mod.rs` defines
   `pub struct EntityIndex(NonMaxU32);` so `Option<Entity>` costs no extra
   space, with a transmute justified inline: `// SAFETY: NonMax is repr
   transparent.`
2. Marker-type generics instead of runtime flags:
   `extras/bevy/crates/bevy_ptr/src/lib.rs` defines empty `Aligned` and
   `Unaligned` structs used as type parameters of `Ptr`, `PtrMut`, and
   `OwningPtr`, moving an alignment invariant into the type system.
3. Interior mutability as an architectural layer:
   `extras/bevy/crates/bevy_ecs/src/world/unsafe_world_cell.rs` builds
   `UnsafeWorldCell`, a documented `UnsafeCell`-style escape hatch that lets
   safe APIs hand out disjoint mutable access to the world; its module doc
   explains the rationale against `&mut World` aliasing in depth.
4. Audited unsafe with mandatory prose: `undocumented_unsafe_blocks = "warn"`
   plus `-D warnings` yields 1766 `// SAFETY:` comments in `bevy_ecs` alone;
   unsafe functions in `extras/bevy/crates/bevy_ptr/src/lib.rs` carry
   `/// # Safety` sections on every method.
5. Debug-checked unsafe unwrapping: the `DebugCheckedUnwrap` trait in
   `extras/bevy/crates/bevy_ecs/src/query/mod.rs` is `#[inline(always)]
   #[track_caller]` and splits debug and release impls "to ensure that the
   unreachable! macro does not cause inlining to fail"; `bevy_ecs` has 215
   `#[track_caller]` annotations for caller-accurate panics.
6. Variadic trait impls by macro:
   `extras/bevy/crates/bevy_ecs/src/system/system_param.rs` uses
   `variadics_please::{all_tuples, all_tuples_enumerated}` to implement
   `SystemParam` for tuples (`all_tuples_enumerated!(impl_param_set, 1, 8, P,
   p);`), the mechanism behind Bevy's magic multi-parameter system functions.
7. Platform cfg as a DSL: `extras/bevy/crates/bevy_platform/src/cfg.rs`
   defines `switch!`, `define_alias!`, `enabled!`, and `disabled!` macros so
   crates write `cfg::std! { ... }` instead of raw `#[cfg(...)]`; aliases are
   "evaluated in the context of the defining crate, not the consumer", which
   makes feature detection composable across the workspace.
8. `no_std` first: `extras/bevy/crates/bevy_ecs/src/lib.rs` opens with
   `#![no_std]`, conditionally re-adds `extern crate std;` behind a feature,
   and guards portability with
   `#[cfg(target_pointer_width = "16")] compile_error!(...)`.
9. Zero-copy string handling: `extras/bevy/crates/bevy_ecs/src/name.rs`
   stores entity names as `HashedStr(Hashed<Cow<'static, str>>)` and accepts
   `impl Into<Cow<'static, str>>`, so string literals never allocate;
   `BevyError` context uses `Vec<Cow<'static, str>>` the same way.
10. Full iterator trait ladders:
    `extras/bevy/crates/bevy_ecs/src/query/access.rs` implements `Iterator`,
    `DoubleEndedIterator`, and `FusedIterator` for `ComponentIdIter<I>`
    generically over the wrapped iterator, plus `IntoIterator` for both owned
    and borrowed `ComponentIdSet`.
11. Structured concurrency: `extras/bevy/crates/bevy_tasks/src/task_pool.rs`
    exposes `pub fn scope<'env, F, T>(&self, f: F) -> Vec<T>`, a scoped
    task-spawning API where lifetimes prove that borrowed data outlives the
    spawned tasks, and atomics come from `bevy_platform::sync` so the same
    code compiles on `no_std` targets.
12. Self-referential proc-macro support:
    `extras/bevy/crates/bevy_ecs/src/lib.rs` contains
    `extern crate self as bevy_ecs;` with the comment "Required to make proc
    macros work in bevy itself", letting derive output paths resolve both
    inside and outside the defining crate.

## 10. Documentation practices

- Every crate begins with `#![doc = include_str!("../README.md")]` (see
  `extras/bevy/crates/bevy_ecs/src/lib.rs` and
  `extras/bevy/crates/bevy_ptr/src/lib.rs`), so the crates.io page, the
  rustdoc front page, and the GitHub README are one file and its examples are
  doctested.
- `missing_docs = "warn"` at workspace level plus `-D warnings` in CI means
  documentation is compiler-enforced, including on private items via
  `check-private-items = true` in `extras/bevy/clippy.toml`.
- Docs.rs output is themed: `extras/bevy/docs-rs/trait-tags.html` injects
  HTML badges for core ECS traits via `--html-after-content`, gated by a
  custom `docsrs_dep` cfg registered in `unexpected_cfgs` check-cfg.
- Generated docs are checked, not trusted: `extras/bevy/docs/cargo_features.md`
  is produced by `cargo run -p build-templated-pages -- update features` from
  `extras/bevy/docs-template/features.md.tpl`, and CI regenerates it and
  fails on any diff.
- Contributor docs live in `extras/bevy/docs/` (`linters.md`, `profiling.md`,
  `debugging.md`, `linux_dependencies.md`); `CONTRIBUTING.md` itself is four
  lines that link to the website's maintained guide.
- `extras/bevy/.github/pull_request_template.md` structures every PR as
  Objective, Solution, Testing, plus an optional Showcase section that asks
  for screenshots or before/after comparisons.
  `extras/bevy/.github/ISSUE_TEMPLATE/` has four forms including a dedicated
  `performance_regression.md`.

## 11. Release and distribution

- Versioning: the whole workspace sits at `0.20.0-dev` between releases;
  CI runs against `release-*` branches (trigger list in
  `extras/bevy/.github/workflows/ci.yml`).
- Changelog discipline is front-loaded into PRs: breaking PRs must add a file
  to `extras/bevy/_release-content/migration-guides/` following
  `migration_guides_template.md`; `extras/bevy/_release-content/migration_guides.md`
  documents the process ("Bevy asks authors (and reviewers) to write a draft
  migration guide as part of the pull requests that make breaking changes")
  and label automation nags authors who skip it. Release notes are drafted the
  same way in `release-notes/`, and the `check-release-content` CI job
  validates the directory with `cargo run --package export-content -- --check`.
- Version bumping is automated: `extras/bevy/.github/workflows/post-release.yml`
  is a `workflow_dispatch` job that sanity-checks the current version with a
  regex, computes the next minor, runs `cargo release "${next_version}"
  --workspace --no-publish --execute --no-tag ...` excluding unpublished
  crates, and opens the bump PR via `peter-evans/create-pull-request`.
- Distribution is crates.io only (59 crates); there are no binaries, shell
  completions, or man pages to ship. Nightly rustdoc for main is deployed to
  GitHub Pages by `extras/bevy/.github/workflows/docs.yml`, which swaps the
  logo and injects `<meta name="robots" content="noindex">` so the dev docs
  never outrank docs.rs.

## 12. Lessons for quinjet

quinjet already has a strict clippy wall, rustfmt, cargo-deny, taplo, typos, a
coverage floor, miri, and mutants. The practices below are the ones Bevy has
that quinjet still lacks, each with its mechanism:

1. Steer APIs with `disallowed-methods` in `clippy.toml`. Bevy bans all of
   `f32`'s transcendental methods with a `reason` string pointing at
   `bevy_math::ops`. quinjet can ban `std::process::exit` outside `main`,
   `std::env::var` outside its config module, and raw `crossterm::execute!`
   outside the terminal layer, each with a reason that names the sanctioned
   wrapper. Also set `check-private-items = true` to lint private docs.
2. Require reasons on every suppression: add `allow_attributes = "warn"` and
   `allow_attributes_without_reason = "warn"` under `[lints.clippy]`, then use
   `#[expect(lint, reason = "...")]` instead of `allow`, as in
   `extras/bevy/Cargo.toml` and `extras/bevy/crates/bevy_ecs/src/lib.rs`.
3. Harden workflows the Bevy way: top-level `permissions: contents: read`,
   `persist-credentials: false` on every `actions/checkout`, every action
   pinned to a full commit SHA with a version comment, `timeout-minutes` on
   every job, and a `concurrency` group with
   `cancel-in-progress: ${{github.event_name == 'pull_request'}}`. Then add
   zizmor (`zizmorcore/zizmor-action`) and CodeQL with the `actions` language
   from `extras/bevy/.github/workflows/security-static-analysis.yml` to lint
   the workflows themselves.
4. Adopt the merge queue: add `merge_group:` to the CI `on:` block as in
   `extras/bevy/.github/workflows/ci.yml` and enable the queue on the repo, so
   main is only ever updated with a green combined state.
5. Split cache writing from cache reading: PR jobs use `actions/cache/restore`
   only, and a separate `update-caches.yml` on `push: main` plus a nightly
   cron rebuilds caches per toolchain, following
   `extras/bevy/.github/workflows/update-caches.yml`. Set
   `CARGO_INCREMENTAL: 0` and `CARGO_PROFILE_TEST_DEBUG: 0` in CI env to keep
   those caches small.
6. Test the real MSRV: declare `rust-version` in `Cargo.toml`, then add a CI
   job that extracts it (`cargo metadata --no-deps --format-version 1 | jq
   --raw-output '.packages[] | select(.name=="quinjet") | .rust_version'`),
   installs exactly that toolchain, and runs `cargo check`, copying the `msrv`
   job in `extras/bevy/.github/workflows/ci.yml`.
7. Script-driven end-to-end TUI runs, modeled on `bevy_ci_testing`: add a
   cargo feature that makes the TUI read a scenario file (path from an env var
   like Bevy's `CI_TESTING_CONFIG`), feed synthetic key events, render to a
   ratatui `TestBackend`, and compare the terminal buffer against committed
   snapshots. Keep one scenario file per flow in a `.github/example-run/`
   style directory, as Bevy does with its RON configs.
8. Consumer-style integration crate: create a `tests-integration/` directory
   excluded from the workspace (Bevy's `[workspace] exclude` plus
   `extras/bevy/tests-integration/simple-ecs-test/Cargo.toml`) that depends on
   quinjet by path and exercises the CLI as a black box, so packaging or
   feature-unification breakage surfaces before publishing.
9. Generated docs with a drift check: quinjet's CLI reference should be
   generated from the clap definitions by a small tool, and CI should rerun
   the generator and fail on `git diff --quiet HEAD --`, exactly as the
   `check-missing-examples-in-docs` job does in
   `extras/bevy/.github/workflows/ci.yml`.
10. An error-code catalog that cannot rot: give quinjet's user-facing failure
    modes stable codes documented in per-code Markdown files, compiled into a
    module with `#[doc = include_str!(...)]` like
    `extras/bevy/errors/src/lib.rs`, so every code block in the catalog is
    doctested.
11. Dependency-graph governance beyond `cargo deny check`: copy Bevy's
    `deny.toml` patterns of `wildcards = "deny"`, `unknown-registry = "deny"`,
    per-crate `deny-multiple-versions` for heavy deps (for quinjet: `ratatui`,
    `crossterm`, `clap`), documented `ignore` entries with links for every
    advisory exception, and `[[bans.features]]` to ban unwanted features of
    dependencies.
12. Dependabot with grouping and cooldown: `extras/bevy/.github/dependabot.yml`
    groups coupled crates so they update atomically and sets `cooldown:
    default-days: 7`; quinjet should group `ratatui`/`crossterm` the same way.
13. Front-load the changelog: keep a `_release-content/` directory with a
    template, require an entry in any behavior-changing PR, and check it in CI
    (`extras/bevy/.github/workflows/ci.yml` job `check-release-content`);
    automate version bumps with `cargo release` in a `workflow_dispatch`
    workflow modeled on `extras/bevy/.github/workflows/post-release.yml`.
14. Pin formatting semantics: add `style_edition` and `newline_style = "Unix"`
    to `rustfmt.toml` as Bevy does, so a rustfmt upgrade cannot cause a
    tree-wide diff, and keep a commented block of desired nightly options as
    the upgrade plan.
