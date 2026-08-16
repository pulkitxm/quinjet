# zed-industries/zed (88670 stars)

## 1. What the project is and what it measures like

Zed is a high-performance, collaborative code editor written entirely in Rust, built by the team that previously created Atom and Tree-sitter. It ships its own GPU-accelerated UI framework (GPUI), its own CRDT text engine, a collaboration server, a language-server client, a debugger client, and a WebAssembly extension host, all in one repository. Industry studies this codebase because it is one of the largest coherent Rust workspaces in the open, and because it demonstrates how to keep sub-second incremental builds and deterministic tests at that scale.

Measured directly from the clone:

- 250 workspace members declared in `extras/zed/Cargo.toml`: 243 under `crates/`, 4 under `extensions/`, 3 under `tooling/` (`tooling/compliance`, `tooling/lints` is deliberately excluded from the workspace, see Section 5).
- Roughly 1,554,000 lines of Rust across `crates/`, `tooling/`, and `extensions/` (sum of `wc -l` over all `.rs` files).
- The root manifest `extras/zed/Cargo.toml` alone is 1,134 lines.
- `default-members = ["crates/zed"]` so a bare `cargo build` builds only the editor binary, not all 250 members.
- The toolchain is pinned exactly in `extras/zed/rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.97.1"
profile = "minimal"
components = [ "rustfmt", "clippy", "rust-analyzer", "rust-src" ]
targets = [
    "wasm32-wasip2", # extensions
    "wasm32-unknown-unknown", # gpui on the web
    "x86_64-unknown-linux-musl", # remote server
]
```

The comments on each target are typical of this repo: every pinned thing carries its reason inline.

Why industry keeps studying it: the same repository contains a production GUI framework, a CRDT engine that survives randomized fuzzing, a WebSocket collaboration server deployed from the repo (`extras/zed/.github/workflows/deploy_collab.yml`), and the build engineering needed to keep 250 crates compiling quickly. Few open codebases show all four at once, and none at this line count with a three-line rustfmt config.

## 2. Repository layout

Selected top level of `extras/zed/`:

```text
zed/
|-- Cargo.toml            250-member workspace root, all shared config
|-- Cargo.lock
|-- rust-toolchain.toml   exact toolchain pin (1.97.1)
|-- rustfmt.toml          3 lines, defaults plus edition
|-- clippy.toml           disallowed-methods with reasons
|-- typos.toml            spell-check config with per-file justifications
|-- lychee.toml           link-checker config for docs
|-- renovate.json         dependency-update bot config
|-- REVIEWERS.conl        reviewer routing by code area
|-- .cargo/               config.toml, ci-config.toml, collab-config.toml
|-- .github/              workflows (46 files), templates, actionlint/zizmor config
|-- assets/               icons, themes, default settings, keymaps
|-- crates/               243 first-party crates
|   |-- gpui/             the UI framework
|   |-- editor/           the editor itself
|   |-- rope/ sum_tree/ text/ clock/   core data structures
|   |-- cli/              the `zed` command-line entry point
|   |-- collab/           the collaboration server
|   `-- util/ collections/ paths/      shared foundations
|-- docs/                 mdBook user documentation
|-- extensions/           first-party extensions built as wasm
|-- legal/                CLA and licensing texts
|-- nix/                  Nix packaging (plus flake.nix, shell.nix at root)
|-- script/               120+ operational scripts (bash, ps1, py, js)
`-- tooling/              compliance, lints (dylint), perf, xtask
```

Why the split works: each feature is a crate (`crates/git_ui`, `crates/file_finder`, `crates/vim`, ...), so incremental compilation and CI test selection operate at crate granularity, and licensing can differ per crate (GPL for the app, Apache for reusable infrastructure). The scaffold script `extras/zed/script/new-crate` enforces this: it symlinks the right license file into the new crate, forbids AGPL for first-party crates, validates the crate name against `^[a-z0-9_]+$`, and emits a manifest that already contains `[lints] workspace = true`:

```sh
if [[ ! "$CRATE_NAME" =~ ^[a-z0-9_]+$ ]]; then
    echo "Error: Crate name must be lowercase and contain only alphanumeric characters and underscores"
    exit 1
fi
```

Two further conventions keep the layout navigable. First, a crate's root source file is named after the crate rather than `lib.rs` (`[lib] path = "src/cli.rs"` in `extras/zed/crates/cli/Cargo.toml`), so editor tabs and stack traces are unambiguous across 243 crates. Second, `script/` is treated as production code: it is shellcheck-gated in CI (`run_shellcheck` step in `extras/zed/.github/workflows/run_tests.yml` runs `./script/shellcheck-scripts error`), and Windows variants sit next to their POSIX twins (`clippy` and `clippy.ps1`, `bundle-linux` and `bundle-windows.ps1`).

## 3. Cargo manifest practices

`extras/zed/Cargo.toml` is a reference example of a large workspace manifest.

Inheritance is minimal but total: `[workspace.package]` sets only `publish = false` and `edition = "2024"`, and every member manifest uses `edition.workspace = true` and `publish.workspace = true` (see `extras/zed/crates/zed/Cargo.toml`). Versions stay per-crate; the app crate carries the product version (`version = "1.17.0"` in `extras/zed/crates/zed/Cargo.toml`).

Dependency organization: `[workspace.dependencies]` is split into two labeled blocks, `# Workspace member crates` (all 243 path dependencies listed once, so member manifests only ever say `editor.workspace = true`) and `# External crates` (alphabetized). Every fork is pinned to a rev under the `zed-industries` org, and renamed forks carry warnings:

```toml
# WARNING: If you change this, you must also publish a new version of zed-reqwest to crates.io
reqwest = { git = "https://github.com/zed-industries/reqwest.git", rev = "c15662463bda39148ba154100dd44d3fba5873a4", ... package = "zed-reqwest", version = "0.12.15-zed" }
```

Feature flags are used to cut compile time and binary size aggressively: `image` disables default features and re-enables 15 named codecs, `objc2-foundation` enables 24 individually named Foundation classes, and the `windows` crate gets its own `[workspace.dependencies.windows]` table listing about 60 API features. The `zed` crate defines composite features like `test-support` and `visual-tests` that fan out to `gpui/test-support`, `editor/test-support`, and so on (`extras/zed/crates/zed/Cargo.toml`).

Profiles are the standout section:

```toml
[profile.dev]
split-debuginfo = "unpacked"
incremental = true
codegen-units = 16
debug = "limited"

# mirror configuration for crates compiled for the build platform
# (without this cargo will compile ~400 crates twice)
[profile.dev.build-override]
codegen-units = 16
split-debuginfo = "unpacked"
debug = "limited"
```

Then `[profile.dev.package]` sets `opt-level = 3` for every proc-macro crate plus `syn`, `quote`, `proc-macro2`, `tree-sitter`, `wasmtime`, and `serde_json`, and sets `codegen-units = 1` on about 35 single-source-file crates with the comment "Build single-source-file crates with cg=1 as it helps make `cargo build` of a whole workspace a bit faster":

```toml
[profile.dev.package]
# proc-macros start
gpui_macros = { opt-level = 3 }
sqlez_macros = { opt-level = 3, codegen-units = 1 }
quote = { opt-level = 3 }
syn = { opt-level = 3 }
proc-macro2 = { opt-level = 3 }
# proc-macros end
```

Release is `lto = "thin"`, `codegen-units = 1`, but the leaf binary is exempted (`[profile.release.package] zed = { codegen-units = 16 }`) because it never benefits from cross-crate inlining as much as it costs. Two extra profiles exist: `dbg` (dev plus full debuginfo, "debug" being a reserved name, per the comment in `extras/zed/Cargo.toml`) and `release-fast` (release minus LTO for iteration on optimized builds). A `[patch.crates-io]` section redirects nine crates (`async-process`, `calloop`, `livekit`, `notify`, and others) to pinned org forks, keeping every transitive user of those names on the patched code without editing member manifests.

Two `[workspace.metadata]` tables integrate third-party tools directly from the manifest: `cargo-machete` gets an `ignored` list, and dylint discovers the custom lint library:

```toml
# Dylint discovers our custom lints through this entry, so `cargo dylint --all`
# runs them without a `--path` argument.
[workspace.metadata.dylint]
libraries = [{ path = "tooling/lints" }]
```

`extras/zed/.cargo/config.toml` adds `rustflags = ["-C", "symbol-mangling-version=v0", "--cfg", "tokio_unstable"]` (v0 mangling for better closure backtraces) and cargo aliases `xtask`, `perf-test`, and `perf-compare`. There is no MSRV field; the pinned toolchain file replaces `rust-version` since nothing is published.

## 4. Formatting

`extras/zed/rustfmt.toml` is three lines:

```toml
# https://github.com/rust-lang/rustfmt?tab=readme-ov-file#rusts-editions
edition = "2024"
style_edition = "2024"
```

The philosophy is stock rustfmt: zero custom settings, so no contributor ever fights the formatter and no nightly-only options are needed. CI runs `cargo fmt --all -- --check` in the `check_style` job of `extras/zed/.github/workflows/run_tests.yml`.

Non-Rust formatting is layered per language:

- Prettier, version-pinned inside the script itself, formats the docs and the default settings JSON: `extras/zed/script/prettier` runs `pnpm dlx "prettier@3.5.0"` against `assets/settings/default.json` (as JSONC) and the `docs/` tree, printing the exact fix command on failure. `extras/zed/.prettierrc` sets only `"printWidth": 120`.
- Tree-sitter query files are formatted by a dedicated language server binary fetched in CI: the `check_style` job downloads `ts_query_ls` v3.15.1 from a GitHub release and runs `ts_query_ls format --check .` (`extras/zed/.github/workflows/run_tests.yml`).
- Protobuf is formatted and linted with `buf format --diff --exit-code crates/proto/proto` in the `check_postgres_and_protobuf_migrations` job.
- Formatting-only commits are erased from history archaeology via `extras/zed/.git-blame-ignore-revs`, whose header explains that GitHub picks the file up automatically for blame views.

There is no repo-root `.editorconfig`; the pinned formatters make it redundant.

## 5. Linting

Lint policy lives in three places, each with a distinct job.

First, `[workspace.lints.clippy]` in `extras/zed/Cargo.toml` denies a short list of correctness and hygiene lints (`dbg_macro`, `todo`, `declare_interior_mutable_const`, `redundant_clone`, `disallowed_methods`) and then does something unusual: it allows the entire style group, with the reasoning written into the manifest:

```toml
# We currently do not restrict any style rules
# as it slows down shipping code to Zed.
#
# Running ./script/clippy can take several minutes, and so it's
# common to skip that step and let CI do it. Any unexpected failures
# (which also take minutes to discover) thus require switching back
# to an old branch, manual fixing, and re-pushing.
style = { level = "allow", priority = -1 }
```

Each additional allow carries a reason: `single_range_in_vec_init` ("We use `vec![a..b]` a lot when dealing with ranges in text"), `too_many_arguments` ("in Rust it can be very tedious to reduce argument count without running afoul of the borrow checker"), `large_enum_variant`, `nonminimal_bool`. The philosophy is explicit: deny only what is objectively wrong, never block a merge on taste, and document every exception.

Second, `extras/zed/clippy.toml` turns `disallowed_methods` into a project-specific API firewall, with a reason and a replacement per entry:

```toml
disallowed-methods = [
    { path = "std::process::Command::spawn", reason = "Spawning `std::process::Command` can block the current thread for an unknown duration", replacement = "smol::process::Command::spawn" },
    { path = "smol::Timer::after", reason = "smol::Timer introduces non-determinism in tests", replacement = "gpui::BackgroundExecutor::timer" },
    { path = "serde_json::from_reader", reason = "Parsing from a buffer is much slower than first reading the buffer into a Vec/String, ... Use `serde_json::from_slice` instead." },
]
```

It also sets `avoid-breaking-exported-api = false` (internal workspace, no API stability debt) and `allow-private-module-inception = true`. `extras/zed/script/clippy` runs `cargo clippy --workspace --release --all-targets --all-features -- --deny warnings`, and when run locally (not in CI) it opportunistically chains `cargo machete`, `typos`, and `buf lint` if those tools are installed. CI-side warning denial is done without the RUSTFLAGS trap: `extras/zed/.cargo/ci-config.toml` is copied to `./../.cargo/config.toml` on runners so cargo merges it with the repo config instead of clobbering it, and its long header comment explains exactly why `RUSTFLAGS` was rejected (it would override the entire config file, citing rust-lang/cargo#5376). The file uses `[target.'cfg(all())'] rustflags = ["-D", "warnings"]` because target tables merge cumulatively where `[build]` would not.

Third, custom compiler-plugin lints. `extras/zed/tooling/lints` is a dylint library (linked against `clippy_utils` at a pinned rev) that encodes rules no off-the-shelf linter knows: `shared_string_from_str_literal` (use `SharedString::from_static` for literals), `async_block_without_await`, `entity_update_in_render`, `notify_in_render`, `owned_string_into_shared`, and `blocking_io_on_foreground` (blocking IO on the main thread). The crate pins its own nightly via `extras/zed/tooling/lints/rust-toolchain.toml` and keeps itself out of the main workspace:

```toml
# Keep this crate out of the zed workspace. It pins its own nightly toolchain
# (see `rust-toolchain.toml`) to match `clippy_utils`.
[workspace]
```

Individual crates can be stricter than the workspace: `extras/zed/tooling/perf/Cargo.toml` layers a full pedantic wall on top of the permissive workspace default:

```toml
[lints.clippy]
all = "warn"
pedantic = "warn"
missing_docs_in_private_items = "warn"
as_underscore = "deny"
allow_attributes = "deny"
allow_attributes_without_reason = "deny" # This covers `expect` also, since we deny `allow`
let_underscore_must_use = "forbid"
undocumented_unsafe_blocks = "forbid"
missing_safety_doc = "forbid"
```

Conformity itself is linted: `cargo xtask package-conformity` (`extras/zed/tooling/xtask/src/tasks/package_conformity.rs`) walks every member with `cargo_toml::Manifest` and reports any crate missing `[lints] workspace = true` or using a non-workspace dependency (`if !is_using_workspace_lints { eprintln!("{package:?} is not using workspace lints", ...) }`).

Spell checking is configured as carefully as clippy: `extras/zed/typos.toml` (118 lines) excludes files only with a written justification per entry:

```toml
    # Vim makes heavy use of partial typing tables.
    "crates/vim/",
    # We have some base64-encoded data that is incorrectly being flagged.
    "crates/rpc/src/auth.rs",
```

and sets `check-filename = true` so file names are spell-checked too.

## 6. CI/CD

The defining practice: workflows are Rust programs. Every YAML file in `extras/zed/.github/workflows/` begins with:

```yaml
# Generated from xtask::workflows::run_tests
# Rebuild with `cargo xtask workflows`.
```

The generator lives in `extras/zed/tooling/xtask/src/tasks/workflows/` (one module per workflow: `run_tests.rs`, `release.rs`, `danger.rs`, ...), built on a fork of the `gh-workflow` crate. CI itself guards the invariant: the `check_scripts` job runs `cargo xtask workflows` and fails if `git diff --exit-code .github` shows drift. Shared steps become Rust functions, which is why step names read like paths (`steps::checkout_repo`, `run_tests::check_style::check_for_typos`). 46 workflow files cover CI, releases, and a large amount of community automation (issue triage, stale-PR reminders, duplicate detection).

`extras/zed/.github/workflows/run_tests.yml` (982 generated lines) is the main gate:

- Triggers: `pull_request`, `push` to `main` and `v[0-9]+.[0-9]+.x` stable branches, and `merge_group` (GitHub merge queue). Most heavy jobs carry `github.event_name != 'merge_group'` so the merge queue re-runs only the cheap, high-signal subset (Linux clippy, style, migrations) after the full PR run already passed.
- An `orchestrate` job diffs against the merge base, maps changed directories to package names via `cargo metadata` piped through `jq`, and emits a `cargo nextest` filterset so test jobs run only reverse dependencies of what changed:

```sh
# Build nextest filterset with rdeps for each package
FILTERSET=$(echo "$ALL_CHANGED_PKGS" | \
  sed 's/.*/rdeps(&)/' | \
  tr '\n' '|' | \
  sed 's/|$//')
echo "changed_packages=$FILTERSET" >> "$GITHUB_OUTPUT"
```

  Toolchain, `.github/`, or root `Cargo.*` changes force the full suite (`grep -qP '^(rust-toolchain\.toml|\.cargo/|\.github/|Cargo\.(toml|lock)$)'` in the same job), and downstream jobs splice the filter into the runner invocation: `cargo nextest run --workspace --no-fail-fast --no-tests=warn${{ ... format(' -E "{0}"', needs.orchestrate.outputs.changed_packages) ... }}`.

- OS coverage: clippy on four targets (Windows, Linux, macOS aarch64, macOS x86_64 via `rustup target add`), `cargo nextest run --workspace --no-fail-fast` on Windows, Linux, and macOS, with a digest-pinned `postgres:15` service container for collab tests on Linux.
- Specialized jobs: `miri_scheduler` (nightly Miri over the `scheduler` crate), `doctests` (`cargo test --workspace --doc`), `check_workspace_binaries` (`cargo build --workspace --bins --examples`), `build_visual_tests_binary`, `check_wasm` (`cargo -Zbuild-std check --target wasm32-unknown-unknown` for two crates), `check_dependencies` (cargo-machete, `cargo update --locked --workspace`, crate-graph tests, and GitHub `dependency-review-action` on PRs), `check_docs` (mdBook build plus two lychee link-check passes), `check_licenses`, and protobuf breaking-change detection with `buf-breaking-action`.
- The whole graph funnels into a single `tests_pass` aggregation job that reads every `needs.*.result` and fails unless each is `success` or `skipped`; that one job is the required check, so the branch-protection list never changes when jobs are added:

```sh
check_result() {
  echo "* $1: $2"
  if [[ "$2" != "skipped" && "$2" != "success" ]]; then EXIT_CODE=1; fi
}
check_result "clippy_linux" "$RESULT_CLIPPY_LINUX"
```

Security hardening is thorough: every action is pinned to a full commit SHA (enforced by `"helpers:pinGitHubActionDigests"` in `extras/zed/renovate.json`), Linux jobs start with `step-security/harden-runner` in egress-audit mode, top-level `permissions: contents: read` is the default, and the `check_scripts` job runs shellcheck over `script/`, `actionlint` (with runner labels declared in `extras/zed/.github/actionlint.yml`), and `zizmor` at `min-severity: high`. Caching is two-tier: a namespace-runner cache for `~/.rustup` plus `sccache` writing to a Cloudflare R2 bucket, with `CARGO_INCREMENTAL: '0'` and a `clear-target-dir-if-larger-than` script guarding runner disks. Concurrency cancels superseded runs per ref but never on main (`github.ref_name == 'main' && github.sha || 'anysha'`), and every shell step runs under `bash -euxo pipefail` via workflow `defaults`. A Danger JS job (`extras/zed/.github/workflows/danger.yml`) lints PR descriptions themselves, routed through a token-less proxy (`DANGER_GITHUB_API_BASE_URL: https://danger-proxy.zed.dev/github`). Renovate batches updates weekly ("after 3pm on Wednesday") behind a dependency dashboard with approval.

## 7. Testing

Tests are colocated: `#[cfg(test)]` modules inside source files, plus dedicated sibling files for big suites (`extras/zed/crates/editor/src/editor_tests.rs`). Cross-crate test utilities ship behind a `test-support` cargo feature; the pattern appears over 800 times across `crates/*/Cargo.toml`, e.g. `util = { workspace = true, features = ["test-support"] }` in `extras/zed/crates/cli/Cargo.toml` dev-dependencies. This gives integration-style tests real fakes (in-memory `fs::FakeFs`, test app contexts) without shipping them in release builds.

The harness centerpiece is the `#[gpui::test]` proc macro (`extras/zed/crates/gpui_macros/src/gpui_macros.rs`), used in 334 files. Its doc comment explains the model: tests get a `TestAppContext` and an `StdRng` "seeded with the `SEED` environment variable and is used internally by the ForegroundExecutor and BackgroundExecutor to run tasks deterministically in tests." Supported arguments include `iterations = N` (run with seeds `0..N`), `seeds(10, 20, 30)`, `retries`, and `on_failure = "path::to::reporter"`. Real usage: `#[gpui::test(iterations = 100)]` in `extras/zed/crates/editor/src/display_map.rs:2743`. Because async scheduling itself is derived from the seed, one attribute turns an ordinary test into a concurrency fuzzer, and the `clippy.toml` ban on `smol::Timer::after` ("introduces non-determinism in tests") keeps the whole codebase on the deterministic clock.

Beyond that:

- Property testing: `proptest` (a pinned git rev with the `attr-macro` feature) plus a custom `Arbitrary for SumTree<T>` impl and a `sum_tree` strategy in `extras/zed/crates/sum_tree/src/property_test.rs`, exposed behind `test-support` so any dependent crate can generate trees:

```rust
impl<T> Arbitrary for SumTree<T>
where
    T: Debug + Arbitrary + Item + 'static,
    T::Summary: Debug + Summary<Context<'static> = ()>,
{
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        any::<Vec<T>>()
            .prop_map(|vec| SumTree::from_iter(vec, ()))
            .boxed()
    }
}
```

- Randomized distributed tests: `extras/zed/script/randomized-test-ci` generates a random u64 seed, runs the collab randomized suite against it, and on failure calls `minimizeTestPlan` from `script/randomized-test-minimize` to shrink the failing operation plan before filing it.
- Miri: CI runs `cargo +nightly -q miri test -p scheduler` on every PR (`run_tests.yml`, `miri_scheduler` job), targeting exactly the crate with the trickiest unsafe concurrency.
- Benchmarks: Criterion (`criterion` with `html_reports` in the workspace deps) across many `benches/` dirs (`crates/rope/benches`, `crates/fuzzy_nucleo/benches`, `crates/language/benches`, plus dedicated `crates/benchmarks` and `crates/editor_benchmarks` crates). A separate perf harness (`extras/zed/tooling/perf`) wraps hyperfine: the `perf-test` cargo alias rebuilds with `--cfg perf_enabled` under the `release-fast` profile, times tests marked `#[perf]`, emits Markdown, and `cargo perf-compare` diffs saved JSON runs.
- Doctests get their own CI job, and a `visual-tests` feature builds a `zed_visual_test_runner` binary for screenshot-style verification.
- The CLI surface is tested at the unit level in `extras/zed/crates/cli` and end to end by CI building every workspace binary (`check_workspace_binaries`) and by the bundling workflows exercising real packaging on all three OS families.

## 8. Error handling and API design

The split is conventional and disciplined: `anyhow` for propagation (166 crate manifests depend on it) and `thiserror` where callers must match on causes (23 manifests). Two representative shapes:

```rust
#[derive(Debug, thiserror::Error)]
pub enum TrashRestoreError {
    #[error("The specified `path` ({}) was not found in the system's trash.", path.display())]
    NotFound { path: PathBuf },
    ...
}
```

from `extras/zed/crates/fs/src/fs.rs`, and a struct error carrying full subprocess context in `extras/zed/crates/git/src/repository.rs`:

```rust
#[derive(Error, Debug)]
#[error("Git command failed:\n{stdout}{stderr}\n")]
struct GitBinaryCommandError {
    stdout: String,
    stderr: String,
    status: ExitStatus,
}
```

which callers recover through `error.downcast_ref::<GitBinaryCommandError>()` (same file, around line 3098), a deliberate pattern: anyhow at the surface, typed recovery where it matters.

Result discipline is supported by helpers in `extras/zed/crates/util/src/util.rs`: a `ResultExt` extension trait whose `log_err()` converts `Result<T, E>` into `Option<T>` while logging, so background failures are visible without unwinding the UI. Panic policy is graded: `some_or_debug_panic` in `extras/zed/crates/gpui_util/src/lib.rs` panics under `debug_assertions` and degrades to `None` in release; production panics are captured as minidumps and uploaded to Sentry by `extras/zed/crates/zed/src/reliability.rs`; and the PR template requires "Unsafe blocks (if any) have justifying comments" (`extras/zed/.github/pull_request_template.md`). The CLI forwards child exit codes faithfully: `std::process::exit(status.code().unwrap_or(1))` in `extras/zed/crates/cli/src/main.rs`.

API design leans on fluent builders generated by macros rather than hand-written ones: the `Styled` trait in `extras/zed/crates/gpui/src/styled.rs` pulls in `gpui_macros::margin_style_methods!()` and friends to produce a Tailwind-like chainable surface (`div().flex().size_full()`). Newtypes are pervasive: `pub struct ReplicaId(u16)` with named constants `LOCAL` and `REMOTE_SERVER` (`extras/zed/crates/clock/src/clock.rs`), `pub struct SharedString(SmolStr)` (`extras/zed/crates/gpui_shared_string/gpui_shared_string.rs`). Visibility discipline is mostly structural, 243 crates make crate boundaries the visibility boundaries, and public-surface crates opt into `#![deny(missing_docs)]` (`extras/zed/crates/theme/src/theme.rs`, `extras/zed/crates/release_channel/src/lib.rs`) while `gpui` runs `#![warn(missing_docs)]`.

## 9. Deep Rust usage: cited idioms

1. Generic B-tree over summaries with a GAT context. `SumTree` (`extras/zed/crates/sum_tree/src/sum_tree.rs`) defines `trait Summary { type Context<'a>: Copy; fn zero(cx) -> Self; fn add_summary(&mut self, other, cx); }` and `trait Dimension<'a, S: Summary>` so the same tree can be sought by bytes, chars, or lines. A blanket `impl<T: ContextLessSummary> Summary for T` plus a `NoSummary` marker avoids blanket-impl collisions, with the reasoning documented in a comment on the impl.
2. Cheap immutable strings. `SharedString(SmolStr)` derefs to `str`, has a `const fn new_static(&'static str)`, and a custom dylint lint (`owned_string_into_shared` in `extras/zed/tooling/lints`) rejects allocating conversions from literals into `SharedString`, `Arc<str>`, `Rc<str>`, or `Cow<'_, str>`.
3. Workspace-wide hasher swap by alias. `extras/zed/crates/collections/src/collections.rs` re-exports the whole standard collection vocabulary with faster defaults, so every crate imports `collections::HashMap` and the hashing strategy is a one-file decision:

   ```rust
   pub type HashMap<K, V> = FxHashMap<K, V>;
   pub type HashSet<T> = FxHashSet<T>;
   pub type IndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
   pub type TypeIdHashMap<V> =
    std::collections::HashMap<std::any::TypeId, V, gpui_util::TypeIdHashBuilder>;
   ```

4. `?` in any context. The `maybe!` macro in `extras/zed/crates/gpui_util/src/lib.rs` expands to an immediately-invoked closure, enabling early-return chains inside functions that do not return `Result`:

   ```rust
   /// Expands to an immediately-invoked function expression. Good for using the ? operator
   /// in functions which do not return an Option or Result.
   macro_rules! maybe {
    ($block:block) => {
        (|| $block)()
    };
    (async move $block:block) => {
        (async move || $block)()
    };
   }
   ```

5. Distributed registration via `inventory`. `extras/zed/crates/gpui/src/action.rs` declares `macro_rules! actions!` and `inventory::collect!(MacroActionBuilder)` (line 282); any crate can register keyboard actions and the app enumerates them at startup with `inventory::iter`, no central registry file to merge-conflict on.
6. Determinism as an architectural property. The test dispatcher runs all async tasks from a seeded RNG (Section 7), and the lint wall (`clippy.toml` disallowed-methods) makes non-deterministic APIs unreachable, turning schedule fuzzing into `#[gpui::test(iterations = 100)]`.
7. Interior mutability, chosen narrowly. `parking_lot` is the workspace lock (`extras/zed/Cargo.toml`), `declare_interior_mutable_const` is denied workspace-wide, and `clippy.toml` documents the one accepted exception via `ignore-interior-mutability` for `agent_ui::context::AgentContextKey` "as the Eq and Hash impls do not use fields with interior mutability."
8. Platform handling by crate, then by target table. The windowing layer is split into `crates/gpui_macos`, `crates/gpui_linux`, `crates/gpui_windows`, `crates/gpui_web` behind `crates/gpui_platform`, whose manifest selects them with `[target.'cfg(target_os = "macos")'.dependencies]` tables (`extras/zed/crates/gpui_platform/Cargo.toml`); even the CLI has four per-OS dependency tables (`extras/zed/crates/cli/Cargo.toml`).
9. Unsafe policy at the edge. `extras/zed/crates/gpui_windows/src/window.rs` opens with `#![deny(unsafe_op_in_unsafe_fn)]`; `tooling/perf` forbids `undocumented_unsafe_blocks`; and small named helpers turn invariants into readable panics, e.g. `CapacityResultExt::unwrap_oob` in `extras/zed/crates/sum_tree/src/sum_tree.rs` with the message "item should fit into fixed size ArrayVec".
10. Zero-copy text traversal. The rope exposes borrowed iterators, `pub fn chunks(&self) -> Chunks<'_>` and `chunks_in_range` over a `pub struct Chunks<'a>` cursor (`extras/zed/crates/rope/src/rope.rs`), so search, rendering, and diffing walk `&str` chunks without materializing the buffer.
11. CRDT plumbing with cache-friendly types. Vector clocks store observed Lamport timestamps in a `SmallVec` (`deletions: SmallVec<[clock::Lamport; 2]>` in `extras/zed/crates/text/src/text.rs`), and `ReplicaId(u16)` keeps identifiers copyable and tiny.
12. Proc-macro leverage. `extras/zed/crates/gpui_macros/src/` contains twelve macro modules (derives for `Render`, `IntoElement`, action registration, style-method generation, a `#[gpui::bench]` Criterion bridge, and the test macro), concentrating boilerplate where the compiler can generate it.

## 10. Documentation practices

User documentation is an mdBook rooted at `extras/zed/docs/book.toml`, with a twist: the HTML renderer is wrapped by a first-party crate (`command = "cargo run -p docs_preprocessor -- postprocess"` under `[output.zed-html]`), because "post-processing is not possible with mdbook in the same way pre-processing is" (comment in the same file). Docs are built and link-checked in CI (`check_docs` job runs lychee twice, on sources and on rendered output, configured by `extras/zed/lychee.toml` with retry and status-code policy), and deployed by `deploy_docs.yml` and `deploy_nightly_docs.yml`.

Contributor documentation lives in `extras/zed/CONTRIBUTING.md`, which is unusually candid about culture ("The Zed culture values working code and synchronous conversations over long discussion threads") and routes big features to a written process doc at `extras/zed/docs/src/development/feature-process.md`. Per-platform development guides sit in `extras/zed/docs/src/development/` (`macos.md`, `linux.md`, `windows.md`, `freebsd.md`, `debugging-crashes.md`, `glossary.md`, `release-notes.md`).

Rustdoc conventions: public-surface crates deny or warn on `missing_docs` (Section 8); macro entry points carry executable usage documentation (the `#[gpui::test]` doc block in `extras/zed/crates/gpui_macros/src/gpui_macros.rs` enumerates every accepted argument with examples); and design rationale is written next to the code it justifies (the `NoSummary` impl comment in `sum_tree.rs`). Process templates: `extras/zed/.github/ISSUE_TEMPLATE/` uses YAML issue forms (`10_bug_report.yml`, `11_crash_report.yml`), and the PR template ends in a five-item self-review checklist covering security, unsafe justification, UI guidelines, tests, and performance. Reviewer routing is data, not tribal knowledge: `extras/zed/REVIEWERS.conl` maps code areas to volunteer reviewers.

## 11. Release and distribution

Zed releases on channels, modeled in code: `ReleaseChannel::{Dev, Nightly, Preview, Stable}` in `extras/zed/crates/release_channel/src/lib.rs`, resolved once through a `LazyLock`:

```rust
pub static RELEASE_CHANNEL: LazyLock<ReleaseChannel> =
    LazyLock::new(|| match ReleaseChannel::from_str(&RELEASE_CHANNEL_NAME) {
```

The product version is the `zed` crate version (`1.17.0` in `extras/zed/crates/zed/Cargo.toml`), bumped by generated workflows (`bump_zed_version.yml`, `bump_patch_version.yml`) and branched to `v[0-9]+.[0-9]+.x` stable branches with a `cherry_pick.yml` helper for hotfixes. The channel names even flow into platform identifiers (`ReleaseChannel::Nightly => "Zed-Editor-Nightly"` in the same file), so parallel installs of different channels never collide.

Pushing a `v*` tag triggers `extras/zed/.github/workflows/release.yml` (1,029 lines), which re-runs the full test suite on macOS and Linux, then fans out to bundling jobs (`run_bundling.yml` builds macOS, Linux, Windows, and FreeBSD artifacts), and finally drafts the GitHub release: release notes are assembled from merged PRs by `script/draft-release-notes` and published via `script/create-draft-release`, so changelog discipline is enforced upstream by the Danger job that checks each PR carries a "Release Notes:" section (Renovate PRs even get `"prFooter": "Release Notes:\n\n- N/A"` in `extras/zed/renovate.json`). Nightly is a cron: `0 */4 * * *` in `extras/zed/.github/workflows/release_nightly.yml` checks the nightly tag and rebuilds every four hours. Distribution beyond GitHub releases includes an in-app updater (`crates/auto_update`, `crates/auto_update_helper`), an install script (`extras/zed/script/install.sh`), Nix packaging (`extras/zed/flake.nix`, `extras/zed/nix/`), and Flatpak/snap scripts under `script/`.

The CLI generates completions for six shells, including two most projects forget: `extras/zed/crates/cli/src/completions.rs` defines a `Shell` value-enum covering Bash, Elvish, Fish, Nushell, PowerShell, and Zsh, implementing `clap_complete::Generator` by delegating to `clap_complete`'s shells plus `clap_complete_nushell`.

## 12. Lessons for quinjet

quinjet already has the strict clippy wall, rustfmt, cargo-deny, taplo, typos, a coverage floor, miri, and mutants. What zed adds on top of that baseline, each with its exact mechanism:

1. Turn the lint wall into an API firewall. Add `disallowed-methods` entries to `clippy.toml` with `reason` and `replacement` keys for quinjet-specific hazards, e.g. ban `std::process::Command::spawn` outside the one sanctioned Git-execution module, exactly as `extras/zed/clippy.toml` bans it in favor of `smol`. For a TUI, also ban direct `println!`/`eprintln!` paths that would corrupt the alternate screen.
2. CI-only `-D warnings` without RUSTFLAGS. Copy the trick from `extras/zed/.cargo/ci-config.toml`: keep a second cargo config containing `[target.'cfg(all())'] rustflags = ["-D", "warnings"]`, copied into a parent `.cargo/config.toml` on CI so it merges with the repo config instead of replacing it.
3. Seeded deterministic test iteration. Adopt the `#[gpui::test(iterations = N)]` idea at quinjet scale: use `proptest` for state-machine tests over Git operations, and honor `SEED`/`ITERATIONS` environment variables in a small attr macro or test helper, as documented in `extras/zed/crates/gpui_macros/src/gpui_macros.rs`.
4. `cargo nextest` as the runner. `cargo nextest run --no-fail-fast --no-tests=warn` (installed in CI via `taiki-e/install-action` with `tool: nextest`, per `extras/zed/.github/workflows/run_tests.yml`) gives per-test isolation, retries, and better CI output than `cargo test`.
5. A single aggregation gate job. Replicate the `tests_pass` pattern: one job with `needs:` on everything, `if: always()`, iterating `needs.*.result` and accepting only `success` or `skipped`, so branch protection references exactly one check forever.
6. Workflow security hardening. Pin every action to a commit SHA and let Renovate maintain the pins (`"helpers:pinGitHubActionDigests"` in `extras/zed/renovate.json`); add `permissions: contents: read` at workflow top level; run `actionlint` and `zizmor` (via `zizmorcore/zizmor-action`) in CI; start Linux jobs with `step-security/harden-runner` in egress-audit mode.
7. Merge queue awareness. Enable GitHub merge queue and add `merge_group: {}` to the CI `on:` block, gating the expensive jobs with `github.event_name != 'merge_group'` as `run_tests.yml` does, so queue throughput stays high.
8. Dev-profile build speed. Steal the profile block from `extras/zed/Cargo.toml`: `split-debuginfo = "unpacked"`, `debug = "limited"`, `[profile.dev.package]` with `opt-level = 3` for `syn`, `quote`, `proc-macro2`, and any heavy parser dependencies; for release, `lto = "thin"` with `codegen-units = 1`.
9. Unused-dependency and lockfile checks. Add `cargo machete` (with `[workspace.metadata.cargo-machete] ignored = [...]` for false positives) and a `cargo update --locked --workspace` step to CI, both from the `check_dependencies` job in `run_tests.yml`.
10. `.git-blame-ignore-revs`. Record formatting-only commits there so blame stays useful; GitHub honors the file automatically (header comment in `extras/zed/.git-blame-ignore-revs`).
11. Completions for all six shells. quinjet already uses clap; add `clap_complete_nushell` next to `clap_complete` and mirror the `Shell` value-enum plus `Generator` impl from `extras/zed/crates/cli/src/completions.rs` to cover Bash, Zsh, Fish, PowerShell, Elvish, and Nushell.
12. Typed errors where callers branch. Keep anyhow at the surface but introduce a `GitBinaryCommandError { stdout, stderr, status }`-style thiserror type for subprocess failures, recovered via `downcast_ref`, as in `extras/zed/crates/git/src/repository.rs`; forward child exit codes with `exit(status.code().unwrap_or(1))` as in `extras/zed/crates/cli/src/main.rs`.
13. Documented lint exceptions. Every `allow` in quinjet's clippy wall should carry a comment stating why, in the style of the annotated allows in `extras/zed/Cargo.toml` `[workspace.lints.clippy]`; undocumented exceptions rot.
14. Criterion benchmarks with a comparison story. Add a `benches/` directory using workspace `criterion` with `html_reports`, and consider the `perf-test`/`perf-compare` cargo-alias pattern from `extras/zed/.cargo/config.toml` for tracking regressions between runs.
15. If a project-specific rule cannot be expressed in `clippy.toml`, a dylint library is the escalation path: a `cdylib` crate depending on `clippy_utils` and `dylint_linting`, registered under `[workspace.metadata.dylint]` and pinned to its own nightly, exactly as `extras/zed/tooling/lints` is structured.
