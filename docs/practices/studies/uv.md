# astral-sh/uv (88771 stars)

## 1. What the project is and how big it is

uv is a Python package and project manager written in Rust, developed by Astral. It replaces
pip, pip-tools, pipx, poetry, pyenv, and virtualenv with a single, extremely fast binary. Industry
adopted it because it makes dependency resolution and installation one to two orders of magnitude
faster than the incumbent tools, while remaining a drop-in interface (`uv pip install`) and adding a
modern project workflow (`uv sync`, `uv lock`, `uv run`). The crate description in
`extras/uv/crates/uv/Cargo.toml` reads:

```toml
[package]
name = "uv"
version = "0.12.5"
description = "A Python package and project manager"
```

Scale, measured directly from the clone:

- 71 crate directories under `extras/uv/crates/` (each with its own `Cargo.toml`); 70 of them are
  workspace members. `crates/uv-trampoline` is excluded from the workspace because it needs
  nightly (`extras/uv/Cargo.toml`).
- 634 Rust source files totaling roughly 520,000 lines of Rust, of which about 229,000 lines are
  CLI integration tests under `extras/uv/crates/uv/tests/`.
- 39 workflow files under `extras/uv/.github/workflows/`, about 10,200 lines of CI configuration.
- Rust edition 2024 across the workspace, MSRV 1.95.0, pinned toolchain 1.97.1
  (`extras/uv/rust-toolchain.toml`).

## 2. Repository layout

```text
extras/uv/
|-- Cargo.toml            workspace root: members, shared deps, lints, profiles
|-- Cargo.lock
|-- crates/               71 crates, all prefixed uv-*
|   |-- uv/               the CLI binary crate (uv, uvx, uvw entry points)
|   |-- uv-cli/           clap definitions, separated from command logic
|   |-- uv-resolver/      dependency resolution (PubGrub)
|   |-- uv-client/        HTTP registry client with rkyv-backed caching
|   |-- uv-test/          shared integration-test harness crate
|   |-- uv-trampoline/    no-std-ish Windows launcher, own workspace, nightly
|   `-- ...               uv-pep440, uv-pep508, uv-fs, uv-git, uv-python, ...
|-- docs/                 mkdocs site: getting-started, guides, concepts, reference
|-- scripts/              release, benchmarking, snapshot, codesign helpers
|-- test/                 fixture data: ecosystem, packages, scenarios, workspaces
|-- python/               the PyPI package shim
|-- changelogs/           archived changelogs per 0.x series
|-- .github/workflows/    39 workflows, mostly reusable (workflow_call)
|-- clippy.toml           disallowed types/methods, doc-valid-idents
|-- rustfmt.toml          edition pinning only
|-- hawk.toml             config for Astral's custom workspace lint tool
|-- dist-workspace.toml   cargo-dist release configuration
|-- ruff.toml, _typos.toml, .pre-commit-config.yaml, .editorconfig
|-- CONTRIBUTING.md, STYLE.md, BENCHMARKS.md, SECURITY.md
`-- CHANGELOG.md
```

The split works because the binary crate (`crates/uv`) contains only command orchestration; every
domain concept lives in a small, focused library crate. Standards get their own crates
(`uv-pep440`, `uv-pep508`), infrastructure gets its own crates (`uv-fs`, `uv-cache`, `uv-client`),
and even single-type crates exist when the type is shared widely (`uv-small-str`, `uv-once-map`,
`uv-redacted`). This keeps compile units small, makes dependency direction explicit, and lets the
resolver, installer, and build frontend evolve independently. The circular dependency between
"resolving needs building" and "building needs resolving" is broken by a trait crate, `uv-types`
(see section 9).

## 3. Cargo manifest practices

The root `extras/uv/Cargo.toml` demonstrates full `workspace.package` inheritance:

```toml
[workspace.package]
edition = "2024"
rust-version = "1.95.0"
homepage = "https://pypi.org/project/uv/"
repository = "https://github.com/astral-sh/uv"
authors = ["uv"]
license = "MIT OR Apache-2.0"
```

Every member crate then contains only `edition = { workspace = true }` style references plus
`[lints] workspace = true` (verified in `extras/uv/crates/uv/Cargo.toml` and
`extras/uv/crates/uv-bench/Cargo.toml`).

Distinctive manifest practices:

- Dual versioning. The `uv` binary is `0.12.5`, but the 60-plus internal library crates all share
  an independent version series, `0.0.72`, listed once in `[workspace.dependencies]` with both
  `version` and `path` so the workspace can be published to crates.io as a unit
  (`extras/uv/Cargo.toml`).
- Every third-party dependency is declared exactly once in `[workspace.dependencies]`,
  alphabetically, with explicit feature selection and frequent `default-features = false`
  (for example `reqwest` with 12 features and rustls instead of native TLS).
- Forked dependencies are declared with `package =` renames so call sites keep upstream names:
  `pubgrub = { version = "0.6.1", package = "astral-pubgrub" }` and
  `async_zip = { version = "0.0.20", package = "astral_async_zip", ... }`.
- Feature flags gate test dependencies on external services, so offline or restricted CI can turn
  them off. From `extras/uv/crates/uv/Cargo.toml`:

```toml
# Features that only apply when running tests, no-ops otherwise.
test-defaults = [
  "test-crates-io",
  "test-git",
  ...
]
# Introduces a testing dependency on PyPI.
test-pypi = ["uv-client/test-pypi"]
```

- The `self-update` feature is off by default and documented as "only enabled for uv's cargo-dist
  installer", so distro packages never ship a self-updater.
- Eight build profiles, each with a comment explaining the tradeoff. `release` uses
  `strip = true`, `lto = "fat"`, `panic = "abort"`; `profiling` inherits release but disables LTO,
  with a long measured justification in comments ("compile times with `lto = true` are completely
  untenable", including timed builds at 3m47s, 53.98s, and 30.09s); `fast-build` and
  `fast-build-nightly` optimize test compile time (`opt-level = 1`, no debuginfo,
  `panic = "abort"` for nextest with `-Z panic-abort-tests`); `minimal-size` uses
  `opt-level = "z"` and `codegen-units = 1` for the `uv-build` backend shim.

## 4. Formatting

`extras/uv/rustfmt.toml` is deliberately minimal, pinning behavior rather than customizing style:

```toml
edition = "2024"
style_edition = "2024"
```

`edition` tells rustfmt how to parse the source; `style_edition` opts into the 2024 formatting
rules (for example the new import sorting). There are no other overrides: uv accepts default
rustfmt output completely, which removes all format debate.

`extras/uv/.editorconfig` handles the cross-language basics: UTF-8, LF, trimmed trailing
whitespace, 2-space indent by default, 4-space for `*.{rs,py,pyi}`, and two precise exceptions
that show real care:

```ini
[*.snap]
trim_trailing_whitespace = false

[crates/uv/tests/help.rs]
trim_trailing_whitespace = false
```

Snapshot files and the help-text test contain meaningful trailing whitespace, so editors must not
strip it.

Non-Rust formatters: Prettier for Markdown/YAML/JSON5 with `proseWrap: "always"` for Markdown
(`extras/uv/.prettierrc`), enforced in CI by `npx prettier@3.9.0 --check .`
(`extras/uv/.github/workflows/check-fmt.yml`); Ruff formats and lints the Python scripts
(`uvx ruff format --diff .` in the same workflow, config in `extras/uv/ruff.toml`); Markdown is
wrapped at 100 characters per `extras/uv/.editorconfig` and `extras/uv/STYLE.md`.

## 5. Linting

Linting is layered across four places.

Layer 1: `[workspace.lints]` in `extras/uv/Cargo.toml`. Rust lints first:

```toml
[workspace.lints.rust]
unsafe_code = "warn"
unreachable_pub = "warn"
```

`unsafe_code = "warn"` means every unsafe block needs an explicit `#[allow(unsafe_code)]` opt-in,
making unsafe grep-able (23 such attributes across the tree, 56 `SAFETY:` comments). Clippy is
`pedantic` at `priority = -2` with a curated allow list (`too_many_lines`, `missing_errors_doc`,
`module_name_repetitions`, and 12 others) plus hand-picked restriction lints promoted to warn:
`print_stdout`, `print_stderr`, `dbg_macro`, `exit`, `get_unwrap`, `rc_buffer`, `rc_mutex`,
`rest_pat_in_fully_bound_structs`, `use_self`, `empty_drop`, `empty_structs_with_brackets`. The
philosophy: pedantic-by-default, silence categories that produce noise for this codebase, and use
restriction lints to encode house rules (no direct printing outside the printer layer, no
`std::process::exit`).

Layer 2: `extras/uv/clippy.toml` turns clippy into an architectural boundary enforcer. All of
`std::fs` and `tokio::fs` is banned at the type and method level:

```toml
disallowed-types = [
  "std::fs::DirEntry",
  "std::fs::File",
  ...
]
disallowed-methods = [
  "std::fs::canonicalize",
  "std::fs::copy",
  ...
  "tokio::fs::write",
]
```

This funnels every filesystem call through `extras/uv/crates/uv-fs`, which wraps `fs-err` so
errors always carry paths, and centralizes Windows path handling. `dotenvy` entry points are
banned the same way so env-file loading happens in exactly one place. `doc-valid-idents` teaches
clippy's doc lint the project vocabulary (`PyPI`, `PubGrub`, `CPython`, every `UV_*` env var name)
with `".."` appended to keep the defaults.

Layer 3: CI flags. `extras/uv/.github/workflows/check-lint.yml` runs
`cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` on both Linux and
Windows (Windows-only code paths get linted too), so the manifest keeps lints at `warn` for
development ergonomics while CI escalates to deny.

Layer 4: custom lint infrastructure. Astral ships its own workspace linter, hawk, configured in
`extras/uv/hawk.toml` and run in CI as `cargo +1.97.1 hawk check --target-dir target/hawk -D
warnings`. It audits the public API surface of the workspace: which packages are production
binaries, which have doctests, and which `pub` items are dead or unnecessarily public, with named,
reasoned overrides:

```toml
[[override]]
lint = "hawk::unnecessary_public"
crate = "uv_client"
item = "base_client::BaseClientBuilder::<'a>::custom_client"
kind = "inherent_method"
level = "expect"
reason = "used by Pixi to provide its own HTTP client"
```

On top of that, `cargo shear --deny-warnings` catches unused dependencies, `typos` (with
`extras/uv/_typos.toml` excluding snapshot dirs and defining domain words) catches spelling, and
`shellcheck --shell bash --severity style` lints every shell script, all in `check-lint.yml`.
Source code preference is visible in the numbers: 157 `#[expect(...)]` attributes, which fail the
build when they stop being needed, versus blanket `#[allow]`.

## 6. CI/CD

The top-level `extras/uv/.github/workflows/ci.yml` is a pure orchestrator: it declares
`permissions: {}`, a concurrency group with `cancel-in-progress: true`, and then delegates every
job to reusable workflows referenced with the same-repository shorthand:

```yaml
  check-lint:
    needs: plan
    uses: $/.github/workflows/check-lint.yml
    with:
      code-changed: ${{ needs.plan.outputs.test-code }}
      save-rust-cache: ${{ needs.plan.outputs.save-rust-cache }}
```

Key structural ideas:

- A `plan` job (`extras/uv/.github/workflows/plan.yml`) inspects the diff once and emits 17
  boolean outputs (`test-code`, `check-schema`, `run-bench`, `test-macos`, `build-docker`, and so
  on). Every downstream workflow is gated on these outputs, so a docs-only PR skips the Rust
  build entirely and a schema change triggers the schema check.
- Every `uses:` of a third-party action is pinned to a full commit SHA with a version comment,
  for example `actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`, and every
  checkout sets `persist-credentials: false`. Renovate (`extras/uv/.github/renovate.json5`)
  manages the pins on a weekly schedule, with `customManagers:githubActionsVersions` also bumping
  tool versions embedded in `run:` steps via `# renovate:` comments (see `HAWK_VERSION` and
  `SHELLCHECK_VERSION` in `check-lint.yml`).
- Workflow security is itself linted: `extras/uv/.github/workflows/check-zizmor.yml` runs the
  zizmor static analyzer over all workflows and uploads SARIF (`security-events: write`), with a
  documented rule exception in `extras/uv/.github/zizmor.yml`.
- Branch protection targets a single aggregation job. `ci.yml` ends with a
  `required-checks-passed` job that `needs:` the six required workflows, runs with `if: always()`,
  and uses `jq` over `toJSON(needs)` to fail if any dependency is neither `success` nor
  `skipped`. Required checks stay stable even as jobs are added or conditionally skipped.
- OS coverage in `extras/uv/.github/workflows/test.yml`: Linux on a 16-core Depot runner with the
  mold linker; macOS on a Namespace runner; Windows on a 16-core runner with a Dev Drive (ReFS)
  created by `extras/uv/.github/workflows/setup-dev-drive.ps1` and the repo copied onto it for
  I/O speed. Windows tests are split three ways with nextest hash partitioning
  (`--partition hash:${{ matrix.partition }}/3`). Linux CI even creates btrfs, tmpfs, and minix
  filesystems in-loop to exercise copy-on-write, non-CoW, and low-hardlink-limit code paths, and
  macOS creates an HFS+ disk image with no reflink support.
- Caching: `Swatinem/rust-cache` everywhere with `save-if` wired to the plan output so only main
  branch (or explicitly opted-in) runs write the cache, plus a custom
  `scripts/prune_cargo_workspace_cache.py` step that deletes superseded workspace artifacts
  before the cache is saved.
- Downstream validation beyond unit tests: `test-smoke.yml` (run the built binary and eval its
  shell completions), `test-integration.yml` (nushell, conda, deadsnakes, armv7-on-aarch64, and
  more, about 1,160 lines), `test-system.yml` (29 jobs installing real packages on Debian,
  Fedora, even Python 3.6 on Debian Buster), and `test-ecosystem.yml` which runs uv against
  pinned commits of real projects (prefect, flask, pydantic-core).
- `test-publish` in `ci.yml` publishes real packages to test.pypi.org through tokens, keyring,
  and OIDC trusted publishing, including impersonated GitLab OIDC tokens, on every relevant PR.
- Environment hardening and hygiene env vars appear in every Rust workflow:
  `CARGO_INCREMENTAL: 0`, `CARGO_NET_RETRY: 10`, `RUSTUP_MAX_RETRIES: 10`, and in `test.yml`
  `RUSTC_BOOTSTRAP: 1` so stable toolchains can use `-Z panic-abort-tests` and
  `-Z checksum-freshness`.
- Release automation is generated by cargo-dist (`extras/uv/.github/workflows/release.yml`,
  configured via `extras/uv/dist-workspace.toml`) with custom local-artifact, publish, and
  post-announce jobs; `release-prepare.yml` is a `workflow_dispatch` job that runs
  `./scripts/release.sh` to bump versions and open the release PR under a bot account with
  short-lived credentials from an STS broker rather than a long-lived PAT.

## 7. Testing

The test pyramid is inverted on purpose: the dominant layer is end-to-end CLI testing. The binary
crate hosts multiple integration-test targets as directories under `extras/uv/crates/uv/tests/`
(`it`, `pip`, `sync`, `lock`, `project`, `tool`, `python`, `workspace`, `build`, and more), each
with its own `main.rs` that composes modules and feature gates:

```rust
#[cfg(all(feature = "test-pypi", feature = "test-universal"))]
mod branching_urls;
...
#[cfg(unix)]
mod resource_limits;
```

(`extras/uv/crates/uv/tests/it/main.rs`.) Splitting into several test binaries lets nextest
schedule them in parallel and lets contributors run one suite at a time.

The harness lives in a dedicated workspace crate, `extras/uv/crates/uv-test/`, whose `lib.rs`
provides a `TestContext` (isolated temp dirs, virtualenvs, managed Pythons, a PyPI proxy, a local
HTTP server, find-links fixtures) plus builder-style helpers like `context.pip_install()`. Two
macros anchor the ergonomics: `test_context!`, which captures `env!("CARGO_BIN_EXE_uv")` at
compile time so the harness always runs the freshly built binary, and `uv_snapshot!`
(`extras/uv/crates/uv-test/src/lib.rs`), which wraps command execution in insta snapshot
assertions with a default filter set:

```rust
macro_rules! uv_snapshot {
    ($spawnable:expr, @$snapshot:literal) => {{
        uv_snapshot!($crate::INSTA_FILTERS.to_vec(), $spawnable, @$snapshot)
    }};
    ...
}
```

There are 6,430 `uv_snapshot!` call sites. Snapshots are mostly inline (`@"..."`), with 88
external `.snap` files; filters normalize timings, paths, and platform noise, and a
`WindowsFilters` mode rewrites Windows-only differences (for example subtracting the
Windows-only `colorama` dependency from package counts) so one snapshot serves all platforms.
CI runs nextest with `INSTA_UPDATE: new` and uploads pending snapshots as an artifact on failure
(`extras/uv/.github/workflows/test.yml`); `extras/uv/scripts/apply-ci-snapshots.sh` then applies
CI-generated snapshots locally, which is how platform-specific snapshots get updated without
owning that platform. JUnit XML from nextest is uploaded per-OS for reporting.

Unit tests exist where the domain is algorithmic (`extras/uv/crates/uv-pep440`,
`uv-distribution-filename`, `uv-once-map` all have inline `#[cfg(test)]` modules), and doctests
are tracked explicitly per package in `extras/uv/hawk.toml` (`[[doctest]] package = "uv-pep440"`).
Network tests use `wiremock` (declared in the dev-dependency block of `extras/uv/Cargo.toml`) and
a vendored PyPI proxy in `uv-test`. There is no fuzzing or property-testing harness in-tree; the
project instead invests in scenario generation (`cargo dev generate-scenario-tests` against
packse scenarios, checked in `extras/uv/.github/workflows/check-generated-files.yml`) and the
ecosystem/system/smoke workflows described in section 6.

Benchmarks live in `extras/uv/crates/uv-bench` with `harness = false` targets (`uv`,
`workspace_discovery`, `uv_pep440`, `uv_pypi_types`) built on
`criterion = { package = "codspeed-criterion-compat" }`, and CI runs them through cargo-codspeed
for continuous regression tracking (`extras/uv/.github/workflows/bench.yml`). `BENCHMARKS.md`
documents the methodology and its caveats (filesystem-dependent install strategies).

## 8. Error handling and API design

The pattern is thiserror in libraries, anyhow only at the binary boundary: 150
`thiserror::Error` derives across `extras/uv/crates/`, while `uv-resolver` contains zero direct
`anyhow::` references. The binary classifies failures for exit-code purposes with a dedicated
enum in `extras/uv/crates/uv/src/commands/mod.rs`:

```rust
pub enum ExitStatus {
    /// The command succeeded.
    Success,
    /// The command reported a failure caused by user input.
    Failure,
    /// The command reported an unexpected failure.
    Error,
    /// The command's exit status is propagated from an external command.
    External(u8),
}
```

with `impl From<ExitStatus> for ExitCode` mapping Success/Failure/Error to 0/1/2 and passing
external codes through. Nothing calls `std::process::exit` directly; the restriction lint
`exit = "warn"` backs that up, and a `UvError` enum (same file) wraps `anyhow::Error` into
`User`, `Argument`, and internal variants so the entry point can select the exit status.

User-facing diagnostics get first-class support in `extras/uv/crates/uv-errors/src/lib.rs`, which
defines a `Hint` trait so error types can attach "hint:"-prefixed suggestions rendered after the
error, with text wrapping handled centrally.

API design conventions observed:

- Builders with lifetimes: `BaseClientBuilder<'a>` in
  `extras/uv/crates/uv-client/src/base_client.rs` has 20+ private fields, 25 `must_use`
  annotations, and chainable setters; hawk overrides document which builder methods exist only
  for downstream consumers like Pixi.
- Newtypes everywhere: `pub struct PackageName(SmallString)` in
  `extras/uv/crates/uv-normalize/src/package_name.rs` guarantees normalization at construction
  (`validate_and_normalize_ref(&name).map(Self)`); `SmallString(arcstr::ArcStr)` in
  `extras/uv/crates/uv-small-str/src/lib.rs` is an O(1)-clone immutable identifier type with the
  full `From`/`AsRef`/`Borrow`/`Deref` surface.
- Security by type: `DisplaySafeUrl(Url)` in `extras/uv/crates/uv-redacted/src/lib.rs` is a
  `#[repr(transparent)]` `RefCast` wrapper whose `Display` masks passwords and sensitive query
  parameters (`X-Amz-Signature` and friends), so credentials cannot leak through logging by
  accident.
- Visibility is policed twice: `unreachable_pub = "warn"` at the compiler level and hawk's
  `dead_public`/`unnecessary_public` lints at the workspace level.
- Panic policy: `panic = "abort"` in release with the comment "This will still show a panic
  message, we only skip the unwind" (`extras/uv/Cargo.toml`); `get_unwrap` is a warn lint; the
  Windows side installs an unhandled-exception handler (`uv_windows::install_unhandled_exception_handler()`
  in `extras/uv/crates/uv/src/lib.rs`).

## 9. Deep Rust usage, cited

1. A bespoke concurrency primitive for request coalescing. `extras/uv/crates/uv-once-map/src/lib.rs`
   builds `OnceMap<K, V>` on the lock-free `papaya::HashMap` plus `tokio::sync::Notify`:
   "Run tasks only once and store the results in a parallel hash map. ... When multiple tasks
   start the same query in parallel ... we want to wait until the other task is done and get a
   reference to the same result." The `register`/`done`/`wait` protocol memoizes network fetches
   across the whole resolver.

2. Zero-copy cache deserialization with rkyv. `extras/uv/crates/uv-client/src/rkyvutil.rs`
   defines `OwnedArchive<A>`: "Constructing the type requires validating the bytes are a valid
   representation of an `Archived<A>`, but subsequent accesses (via deref) are free." Registry
   metadata is written once and then memory-mapped as archived structs; `uv-pep440`,
   `uv-distribution-filename`, and `uv-pypi-types` all derive `rkyv::Archive` behind an `rkyv`
   feature.

3. Bit-packing with an explicit niche. `extras/uv/crates/uv-pep440/src/version.rs` stores common
   versions in a single `u64` (`struct VersionSmall { repr: u64, len: u8, _force_niche:
   NonZero<u8> }`) with the comment "Force a niche into the aligned type so the [`Version`] enum
   is two words instead of three." A parsed PEP 440 version is two machine words in the hot path.

4. `#[repr(transparent)]` plus `ref-cast` for free wrapper conversions.
   `extras/uv/crates/uv-redacted/src/lib.rs` derives `RefCast` on `DisplaySafeUrl(Url)` so
   `&Url` can be reinterpreted as `&DisplaySafeUrl` without allocation, keeping redaction zero
   cost.

5. A written-down unsafe policy, enforced by lints. With `unsafe_code = "warn"` workspace-wide,
   the entry point in `extras/uv/crates/uv/src/lib.rs` is itself `pub unsafe fn main`, with a
   `# Safety` section ("It is only safe to call this routine when it is known that multiple
   threads are not running") because it calls `std::env::set_var`; the caller in
   `extras/uv/crates/uv/src/bin/uv.rs` discharges the obligation with a `SAFETY:` comment. 56
   `SAFETY:` comments exist against only 42 files that mention unsafe at all.

6. Platform code isolated into dedicated crates. `extras/uv/crates/uv-unix/src/lib.rs` opens with
   `#![cfg(unix)]` and exports only resource-limit handling; `extras/uv/crates/uv-windows/src/`
   holds job objects, ctrl handlers, structured-exception handling, and Wine detection. The
   remaining 323 `#[cfg(unix)]`/`#[cfg(windows)]` attributes are for small local divergences, and
   the `windows` crate is imported with 17 explicitly enumerated `Win32_*` features
   (`extras/uv/Cargo.toml`) to keep build times bounded.

7. Lint-enforced I/O architecture. Because `clippy.toml` bans `std::fs`/`tokio::fs` (section 5),
   all filesystem access flows through `extras/uv/crates/uv-fs`, which layers `fs-err` for
   path-carrying errors and defines a `Simplified` trait
   (`extras/uv/crates/uv-fs/src/path.rs`) whose `simplified_display()` strips the Windows
   `\\?\` prefix for user-facing output. An idiom (clippy config) enforces an architecture rule.

8. Trait-based dependency inversion with `impl Trait` in traits.
   `extras/uv/crates/uv-types/src/traits.rs` defines `BuildContext` with an associated type
   `SourceDistBuilder: SourceBuildTrait` and async-by-signature methods like
   `fn interpreter(&self) -> impl Future<Output = &Interpreter> + '_;`, letting `uv-resolver`
   invoke source builds without depending on the build crates.

9. Proc macros for metadata, not magic. `extras/uv/crates/uv-macros/src/lib.rs` derives
   `OptionsMetadata` and `PreviewMetadata` and provides attributes (`attr_hidden`,
   `attr_env_var_pattern("UV_INDEX_{name}_USERNAME")`) consumed in
   `extras/uv/crates/uv-static/src/env_vars.rs`; `cargo dev generate-env-vars-reference` then
   turns that metadata into the documentation site, so docs cannot drift from code.

10. Lazily borrowed strings by default. 168 `Cow<'_, ...>` sites; a representative one in
    `extras/uv/crates/uv-static/src/lib.rs` returns `Cow<'_, str>` from
    `astral_mirror_base_url`, borrowing the trimmed user URL or the static default without
    allocating.

11. Modern control flow at scale: 605 `let ... else` bindings and 136 `Either::Left/Right` sites
    (branching iterator pipelines without boxing), plus lifetime-annotated iterator returns such
    as `pub fn packages(&self) -> impl Iterator<Item = (&'lock PackageName, &'lock Version)> + '_`
    in `extras/uv/crates/uv-resolver/src/lock/mod.rs`.

12. Runtime tuning in the entry point. `extras/uv/crates/uv/src/lib.rs` builds a
    current-thread tokio runtime with a custom stack size, boxes the main future ("Box the large
    main future to avoid stack overflows"), and constructs the `papaya`-backed workspace cache
    before spawning threads so the seize memory barrier registers on the kernel's single-threaded
    fast path; a swappable high-performance allocator ships as its own optional crate,
    `uv-performance-memory-allocator`, kept alive via `extern crate` in
    `extras/uv/crates/uv/src/bin/uv.rs`.

13. A nightly `no-std`-adjacent launcher. `extras/uv/crates/uv-trampoline/Cargo.toml` enables
    `cargo-features = ["panic-immediate-abort"]`, `opt-level = "z"`, `lto`, and `ufmt` instead of
    `std::fmt` to produce the tiny Windows script trampoline, isolated in its own workspace with
    its own toolchain file so the main workspace stays on stable.

## 10. Documentation practices

- Rustdoc uses intra-doc links pervasively; doc comments reference types as `[`OnceMap::done`]`
  (`extras/uv/crates/uv-once-map/src/lib.rs`) and include runnable doctests where the API is
  self-contained, for example the credential-masking examples in
  `extras/uv/crates/uv-redacted/src/lib.rs`. Doctest-bearing packages are tracked in
  `extras/uv/hawk.toml`, and crates that should not run doctests disable them
  (`[lib] doctest = false` in `extras/uv/crates/uv-bench/Cargo.toml`).
- The user documentation is an in-repo mkdocs-material site (`extras/uv/mkdocs.yml`,
  `extras/uv/docs/`) split into getting-started, guides, concepts, pip compatibility, and
  reference. The reference section is generated from code by the `uv-dev` crate
  (`extras/uv/crates/uv-dev/src/generate_cli_reference.rs`, `generate_options_reference.rs`,
  `generate_env_vars_reference.rs`), regenerated in CI before `mkdocs build --strict`
  (`extras/uv/.github/workflows/check-docs.yml`), and guarded against drift by
  `cargo dev generate-all --mode dry-run` in `check-generated-files.yml` and a pre-commit hook.
- `extras/uv/STYLE.md` is unusual: a style guide for CLI output and prose, covering terminology
  ("lockfile" not "lock file"), the no-capitalization rule for the name uv, color semantics
  (green success, red error, cyan hints and paths), the rule that all logging goes to stderr, and
  per-doc-type voice rules (guides are second person imperative, concepts third person).
- `extras/uv/CONTRIBUTING.md` documents the nextest/insta workflow, local Python setup, and a
  clear contribution policy (no feature PRs without prior discussion). Issue templates are typed
  YAML forms (`extras/uv/.github/ISSUE_TEMPLATE/1_bug_report.yaml`, feature request, question),
  and `extras/uv/.github/PULL_REQUEST_TEMPLATE.md` asks for exactly two sections, Summary and
  Test Plan.

## 11. Release and distribution

- Versioning: the binary follows 0.x SemVer (`0.12.5`), internal crates follow their own `0.0.72`
  series; `extras/uv/scripts/release.sh` and `scripts/bump-workspace-crate-versions.py` perform
  the bump, driven by the `release-prepare.yml` workflow_dispatch job.
- Changelog discipline: `extras/uv/CHANGELOG.md` groups entries per release under headings such
  as Python, Enhancements, Preview features, and Bug fixes, each entry linking its PR; complete
  historical series are archived under `extras/uv/changelogs/0.1.x.md` through `0.11.x.md`.
- The release pipeline is cargo-dist (`extras/uv/dist-workspace.toml`): 18 prebuilt targets from
  `aarch64-apple-darwin` to `s390x-unknown-linux-gnu`, shell and PowerShell installers,
  `dispatch-releases = true` (releases triggered by dispatch, not tag push), GitHub build
  provenance attestations (`github-attestations = true`) with filters, per-target glibc floors
  (`aarch64-unknown-linux-gnu = "2.28"`, `"*" = "2.17"`), and a simple-hosting mirror
  (`simple-download-url = "https://releases.astral.sh/..."`) falling back to GitHub releases.
  Even the actions used inside generated release workflows are SHA-pinned via
  `[dist.github-action-commits]`.
- Binaries per platform: `uv` and `uvx` everywhere, plus a `uvw` (windowless) binary on Windows
  (`[dist.binaries]` in `extras/uv/dist-workspace.toml`).
- Distribution channels beyond GitHub: PyPI wheels built with maturin
  (`build-backend = "maturin"` in `extras/uv/pyproject.toml`, published by
  `extras/uv/.github/workflows/publish-pypi.yml` with trusted publishing), Docker images
  (`build-docker.yml` with package attestations), crates.io (`publish-crates.yml` publishes the
  whole workspace with a nightly `-Zpublish-timeout` and a raised
  `publish.timeout=600`, after `cargo publish --workspace --dry-run` gated every PR in
  `check-publish.yml`), and versioned docs (`publish-docs.yml`, `publish-versions.yml`).
- Shell completions ship as a hidden subcommand (`generate-shell-completion` via
  `clap_complete_command` in `extras/uv/crates/uv-cli/src/lib.rs`) and are smoke-tested in CI by
  actually eval-ing them: `eval "$(./uv generate-shell-completion bash)"`
  (`extras/uv/.github/workflows/test-smoke.yml`).

## 12. Lessons for quinjet

quinjet already has a strict clippy wall, rustfmt, cargo-deny, taplo, typos, coverage, miri, and
mutants. The following uv practices are still worth adopting, with exact mechanisms:

1. Turn clippy into an architecture enforcer with `clippy.toml` `disallowed-methods` and
   `disallowed-types`. For a Git TUI: ban `std::process::exit` call sites outside `main`, ban raw
   `println!`/`eprintln!` outside the presentation layer (quinjet likely has `print_stdout`
   already; the disallowed lists go further by banning specific APIs such as `std::fs::*` in
   favor of an error-context wrapper like `fs-err`), and add `doc-valid-idents` for project
   vocabulary so pedantic doc lints stop fighting names.
2. Adopt insta snapshot testing for the CLI surface: `insta` with the `filters` and `redactions`
   features plus `assert_cmd`, wrapped in one project macro modeled on `uv_snapshot!`
   (`extras/uv/crates/uv-test/src/lib.rs`) that applies a default filter list (temp paths, hashes,
   durations) and asserts stdout, stderr, and exit code together. Since every quinjet operation is
   a CLI subcommand, this covers the whole command layer; pair it with a `TestContext` that
   creates a scratch Git repository per test via `assert_fs`.
3. Add `cargo shear --deny-warnings` (workflow step, `taiki-e/install-action` with
   `tool: cargo-shear`) to catch unused dependencies; cargo-deny does not do this.
4. Add zizmor for workflow auditing: run `zizmorcore/zizmor-action` in CI as in
   `extras/uv/.github/workflows/check-zizmor.yml`, pin every action to a full SHA with a
   `# vX.Y.Z` comment, set top-level `permissions: {}` per workflow, and set
   `persist-credentials: false` on every `actions/checkout`.
5. Create a `required-checks-passed` aggregation job (`if: always()` plus a `jq` scan of
   `toJSON(needs)` as in `extras/uv/.github/workflows/ci.yml`) and make it the only required
   branch-protection check, so jobs can be added or skipped without touching repo settings.
6. Switch test execution to `cargo-nextest` with a dedicated Cargo profile modeled on uv's
   `fast-build` (`opt-level = 1`, `debug = 0`, `strip = "debuginfo"`, `lto = "off"` in
   `extras/uv/Cargo.toml`), and upload JUnit XML plus pending insta snapshots as artifacts on
   failure (`INSTA_UPDATE: new` as in `extras/uv/.github/workflows/test.yml`).
7. Use `#[expect(...)]` instead of `#[allow(...)]` for every lint suppression so stale
   suppressions fail the build; uv carries 157 of them.
8. Model the exit path on `ExitStatus`: an enum with documented variants converted through
   `impl From<ExitStatus> for ExitCode` (`extras/uv/crates/uv/src/commands/mod.rs`) instead of
   scattered `process::exit`, distinguishing user error (1) from internal error (2) and
   propagating child exit codes for pass-through Git invocations.
9. Adopt cargo-dist for releases: a `dist-workspace.toml` with SHA-pinned
   `[dist.github-action-commits]`, shell + PowerShell installers, GitHub attestations, and
   `pr-run-mode = "skip"`, following `extras/uv/dist-workspace.toml`; ship completions via a
   hidden `generate-shell-completion` subcommand (`clap_complete_command`) and smoke-test them in
   CI with `eval "$(quinjet generate-shell-completion bash)"` as in
   `extras/uv/.github/workflows/test-smoke.yml`.
10. Add a generated-files drift check: if quinjet generates docs from clap (and it should, via a
    `dev` xtask like `extras/uv/crates/uv-dev` with `generate_cli_reference.rs`), add a CI step
    running the generator in `--mode check` (`extras/uv/.github/workflows/check-generated-files.yml`)
    and a matching pre-commit hook.
11. Write a `STYLE.md` for user-facing output modeled on `extras/uv/STYLE.md`: color semantics,
    stderr-only logging, `NO_COLOR` compliance, message capitalization and punctuation rules.
    For a TUI this doubles as the theming contract.
12. Keep a `Hint`-style trait for errors (`extras/uv/crates/uv-errors/src/lib.rs`) so failure
    messages can carry actionable "hint:" lines (for example suggesting `--force` or the matching
    subcommand) rendered uniformly by one printer.
13. Gate environment-dependent tests behind Cargo features the way uv gates `test-git` and
    `test-pypi` (`extras/uv/crates/uv/Cargo.toml`), so quinjet tests needing a real remote or
    network can be excluded deterministically with `--no-default-features`.
14. Use Renovate custom managers (`customManagers:githubActionsVersions` plus `# renovate:`
    comments as in `extras/uv/.github/workflows/check-lint.yml`) so tool versions embedded in
    workflow `run:` steps are updated automatically instead of rotting.
