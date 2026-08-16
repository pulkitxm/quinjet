# tauri-apps/tauri (110245 stars)

## 1. What the project is and what the clone measures

Tauri is a framework for building small, secure desktop and mobile applications whose UI is
rendered in the operating system webview while the application backend is a compiled Rust binary.
Industry adopts it as the lighter alternative to Electron: no bundled browser runtime, binaries in
the low megabytes, and a capability-based security model between the webview and the Rust core.
The repository is the entire product surface: the runtime crates, the code generator, the macro
crates, the bundler, the `cargo tauri` CLI, and the npm packages that wrap them.

Scale, measured directly from the clone at commit `2f1cd75b0f3fb72e6870d719bbfeadedcf8ca884`:

- 14 first-party crates under `extras/tauri/crates/` (tauri, tauri-build, tauri-bundler,
  tauri-cli, tauri-codegen, tauri-driver, tauri-macos-sign, tauri-macros, tauri-plugin,
  tauri-runtime, tauri-runtime-wry, tauri-schema-generator, tauri-schema-worker, tauri-utils).
- 25 workspace members in `extras/tauri/Cargo.toml` once the napi CLI wrapper
  (`packages/cli`), two integration test crates (`crates/tests/restart`, `crates/tests/acl`),
  four bench crates, and four example app crates are counted.
- 326 `.rs` files totaling roughly 100,000 lines of Rust; about 97,000 of those lines live under
  `extras/tauri/crates/` outside the CLI project templates.
- Per-crate `src/` line counts: tauri 32,678; tauri-cli 22,606; tauri-utils 15,502;
  tauri-bundler 9,014; tauri-runtime-wry 6,748; tauri-runtime 2,851; tauri-build 2,038;
  tauri-codegen 1,618; tauri-macros 1,458; tauri-plugin 376.
- 21 workflow files under `extras/tauri/.github/workflows/`.

## 2. Repository layout

```text
extras/tauri/
|-- ARCHITECTURE.md          system overview, one section per crate
|-- Cargo.toml               virtual workspace root, no root package
|-- Cargo.lock               committed, one lock for every member
|-- rustfmt.toml             shared Rust formatting
|-- .editorconfig            cross-language whitespace baseline
|-- .prettierrc              JS/TS/MD/YAML formatting
|-- .cargo/
|   |-- config.toml          [env] workaround for Windows test loading
|   `-- audit.toml           cargo-audit RUSTSEC ignore list with reasons
|-- .changes/                covector change files, one per pending release note
|-- .scripts/ci/             Node helper scripts run by workflows
|-- .github/workflows/       21 CI/CD pipelines
|-- audits/                  published third-party security audit PDFs
|-- supply-chain/            cargo-vet config, audits, imports.lock
|-- bench/                   internal benchmark harness plus 3 app fixtures
|-- crates/                  all Rust crates
|   `-- tests/               integration-only crates (restart, acl)
|-- examples/                runnable example apps, some are workspace members
`-- packages/                npm packages (@tauri-apps/api, @tauri-apps/cli)
```

The split works because each layer has one owner crate: `tauri-runtime` defines the windowing
abstraction, `tauri-runtime-wry` implements it, `tauri-utils` holds config parsing and the ACL
model shared by build-time and run-time code, `tauri-codegen` and `tauri-macros` turn config into
compiled context, and `tauri` composes them. `extras/tauri/ARCHITECTURE.md` documents exactly this
decomposition, one heading per crate, and marks stability per component
("Tauri Core [STABLE RUST]"). Test-only crates live under `extras/tauri/crates/tests/` so heavy
integration fixtures never bloat the published packages, and everything that is npm-facing is
quarantined in `extras/tauri/packages/`.

## 3. Cargo manifest practices

The root `extras/tauri/Cargo.toml` is a virtual workspace with `resolver = "2"` and centralizes
identity fields in `[workspace.package]`:

```toml
[workspace.package]
authors = ["Tauri Programme within The Commons Conservancy"]
homepage = "https://tauri.app/"
repository = "https://github.com/tauri-apps/tauri"
categories = ["gui", "web-programming"]
license = "Apache-2.0 OR MIT"
edition = "2021"
rust-version = "1.90"
```

Every crate inherits these with the dotted form, e.g. `extras/tauri/crates/tauri/Cargo.toml`:
`edition.workspace = true`, `rust-version.workspace = true`. `[workspace.dependencies]` pins the
intra-workspace crates with both a `version` and a `path`, so members depend on
`tauri-utils = { workspace = true, features = [...] }` and publishing still resolves to crates.io
versions. Third-party dependencies deliberately stay per-crate rather than in the workspace table,
keeping each manifest an honest inventory of what that crate uses.

Notable manifest techniques, all in `extras/tauri/crates/tauri/Cargo.toml`:

- Platform-partitioned dependency tables with full `cfg` expressions, one section per platform
  family, so Linux GTK crates, `objc2-*` Apple crates, `windows`/`webview2-com`, `jni` for
  Android, and mobile-only `reqwest` never appear on the wrong target:

```toml
[target.'cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))'.dependencies]
gtk = { version = "0.18", features = ["v3_24"] }
webkit2gtk = { version = "2", features = ["v2_40"], optional = true }
```

- A large, additive feature graph where features forward to sub-crates with the `?` syntax
  (`tracing = ["dep:tracing", "tauri-macros/tracing", "tauri-runtime-wry?/tracing"]`) and
  `dep:` gates keep optional dependencies out of the implicit feature namespace.
- Inline policy comments on risky pins: `# WARNING: cookie::Cookie is re-exported so bumping this
  is a breaking change, documented to be done as a minor bump`.
- `[package.metadata.docs.rs]` requesting a curated feature set and five explicit doc targets,
  plus `[package.metadata.cargo-udeps.ignore]` to silence known false positives per dependency
  kind (`normal = ["reqwest"]`, `build = ["tauri-build"]`, `development = ["quickcheck_macros"]`).
- `exclude = ["/test", "/.scripts", "CHANGELOG.md", "/target"]` and, in
  `extras/tauri/crates/tauri-cli/Cargo.toml`, an explicit `include = [...]` allowlist so the
  published CLI package carries only sources, templates, and licenses.

The workspace tunes profiles for shipping size, with a comment stating the intent:

```toml
# default to small, optimized workspace release binaries
[profile.release]
panic = "abort"
codegen-units = 1
lto = true
incremental = false
opt-level = "s"
strip = true
```

plus `[profile.dev.package.miniz_oxide] opt-level = 3` so asset compression is bearable in dev
builds, and a `[patch.crates-io]` section pointing `schemars_derive` at a fork, annotated with the
upstream issue link that justifies the patch. The only `[lints]` table in the repo is in
`extras/tauri/crates/tauri-schema-worker/Cargo.toml`, which registers an expected custom cfg:

```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = [
  'cfg(wasm_bindgen_unstable_test_coverage)',
] }
```

## 4. Formatting

`extras/tauri/rustfmt.toml` is checked into the root and applies to all crates:

```toml
max_width = 100
hard_tabs = false
tab_spaces = 2
newline_style = "Unix"
use_small_heuristics = "Default"
reorder_imports = true
reorder_modules = true
remove_nested_parens = true
edition = "2021"
merge_derives = true
use_try_shorthand = false
use_field_init_shorthand = false
force_explicit_abi = true
```

Setting by setting: `max_width = 100` widens the default 100/comment line budget slightly for the
deeply nested builder code; `tab_spaces = 2` matches the JS side of the monorepo so mixed reviews
read uniformly; `newline_style = "Unix"` forces LF even on Windows checkouts; `reorder_imports`
and `reorder_modules` remove import-order bikeshedding; `merge_derives` collapses derive lists
into a single attribute; `use_try_shorthand = false` and `use_field_init_shorthand = false` keep
explicit forms, a readability-over-brevity call; `force_explicit_abi = true` requires
`extern "C"` rather than bare `extern`, relevant for a codebase with FFI. Two nightly-only
options (`normalize_comments`, `wrap_comments`) are kept commented out rather than deleted, which
records the intent without breaking stable rustfmt.

`extras/tauri/.editorconfig` sets the baseline for every file type: `charset = utf-8`,
`indent_style = space`, `indent_size = 2`, `end_of_line = lf`, `insert_final_newline = true`,
`trim_trailing_whitespace = true`. Non-Rust formatting is Prettier, configured in
`extras/tauri/.prettierrc` (`"singleQuote": true`, `"semi": false`) with a heavily commented
`extras/tauri/.prettierignore` that excludes generated bundles, CLI templates, WiX `.wxs`
templates, and lock files. TOML files are formatted by taplo with default settings; the `taplo`
job in `extras/tauri/.github/workflows/fmt.yml` runs `taplo fmt --check --diff`.

## 5. Linting

There is no `clippy.toml` and, apart from the wasm worker noted above, no `[lints]` tables. The
clippy wall lives entirely in CI. `extras/tauri/.github/workflows/lint-rust.yml` runs clippy as
deny-warnings across a five-target matrix (Windows MSVC, Linux GNU, macOS aarch64, iOS aarch64,
Android via `cross`):

```yaml
- run: ${{ matrix.platform.cargo }} clippy --target ${{ matrix.platform.target }} ${{ matrix.platform.include }}--all-targets --all-features -- -D warnings
```

The mobile rows lint through the example app (`include: '--package api '`) with the comment
"api example crate should pull in every dependency we want to lint on mobile", which is a cheap
way to lint the full mobile dependency closure without a separate harness. The philosophy is:
default clippy level, but enforced everywhere, on every platform, with warnings as errors, and
suppressions must be local and explicit. The tree contains only 47 `allow(clippy::...)`
attributes across ~100k lines, each scoped to one item, e.g.
`#[allow(clippy::large_enum_variant)]` in `extras/tauri/crates/tauri-macros/src/command/wrapper.rs`.

Library crates raise the bar at the crate level rather than in manifests.
`extras/tauri/crates/tauri/src/lib.rs`, `extras/tauri/crates/tauri-utils/src/lib.rs`, and
`extras/tauri/crates/tauri-bundler/src/lib.rs` all declare:

```rust
#![warn(missing_docs, rust_2018_idioms)]
```

Custom check infrastructure fills the gaps clippy cannot see: `extras/tauri/.scripts/ci/check-license-header.js`
verifies the SPDX header on every added `.rs`, `.js`, `.ts`, `.yml`, `.swift`, and `.kt` file
(wired into `extras/tauri/.github/workflows/check-license-header.yml`), and
`extras/tauri/.scripts/ci/check-change-tags.js` validates release change files. Unused
dependencies are policed by a dedicated nightly `cargo udeps --all-targets --all-features` job in
`extras/tauri/.github/workflows/udeps.yml`, which builds a dynamic per-crate matrix from
`dorny/paths-filter` outputs so only touched crates are scanned.

## 6. CI/CD

All 21 workflows live in `extras/tauri/.github/workflows/`. Shared conventions: every workflow
carries the SPDX license header; almost every one declares a concurrency group
(`group: ${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true`); PR triggers are
path-filtered so a docs change does not spin up the Windows fleet; caching uses
`Swatinem/rust-cache@v2` keyed by target triple; and jobs export
`CARGO_PROFILE_DEV_DEBUG: 0` with the comment "This would add unnecessary bloat to the target
folder, decreasing cache efficiency."

- `fmt.yml`: three jobs on every PR: `cargo fmt --all -- --check`, Prettier via
  `pnpm format:check`, and `taplo fmt --check --diff`.
- `lint-rust.yml`: the five-target clippy matrix described above, with `fail-fast: false`.
- `test-core.yml`: a matrix of five platforms times two feature sets
  (`--no-default-features` vs `--all-features`), pinned to the MSRV toolchain (`toolchain: '1.90'`),
  where iOS and Android rows downgrade `test` to `build`. Cache writes are restricted with
  `save-if: ${{ matrix.features.key == 'all' }}` so the no-default variant reuses rather than
  churns the cache. Android uses `cross` installed from a specific git revision with `--locked`.
- `test-cli-rs.yml`: CLI tests on four targets including `aarch64-pc-windows-msvc` with
  `--no-default-features --features native-tls-vendored`.
- `test-android.yml`: builds a real mobile template project on all three OS runners with Java 25
  and Gradle caching, path-filtered to the mobile template and mobile CLI sources.
- `audit.yml`: daily cron plus lockfile-path triggers, running `rustsec/audit-check@v2` and
  `pnpm audit`. Its ignore list is versioned in `extras/tauri/.cargo/audit.toml`, where every
  RUSTSEC id carries a justification comment.
- `supply-chain.yml`: daily `cargo-vet` run with the binary cached in the runner tool cache; the
  config in `extras/tauri/supply-chain/config.toml` imports audit sets from bytecode-alliance,
  Embark, Google, ISRG, Mozilla, and Zcash, and `supply-chain/audits.toml` records trusted
  publishers per crate.
- `udeps.yml`: nightly cargo-udeps, one matrix leg per changed crate, sharing one compiled
  `cargo-udeps` binary between jobs via artifact upload/download.
- `check-generated-files.yml`: rebuilds the TS API bundle and the JSON config schemas when their
  sources change, then fails if `git diff` is non-empty via `extras/tauri/.scripts/ci/has-diff.sh`.
  Generated artifacts are committed, and CI proves they are in sync.
- `check-change-tags.yml` and `check-license-header.yml`: the custom Node checks, both using
  `dorny/paths-filter@v3` with `list-files: shell` to pass exactly the changed files.
- `covector-status.yml` plus `covector-comment-on-fork.yml`: release-note status on every PR; the
  fork variant is a `workflow_run` follow-up with an explicit least-privilege `permissions:` block
  (`actions: read`, `pull-requests: write`) because fork PRs cannot comment directly.
- `covector-version-or-publish.yml`, `publish-cli-rs.yml`, `publish-cli-js.yml`,
  `deploy-schema-worker.yml`, `docker.yml`, `bench.yml`: release and infrastructure automation,
  covered in sections 7 and 11.

Action pinning is pragmatic rather than absolute: most actions ride major tags
(`actions/checkout@v7`, `dtolnay/rust-toolchain@stable`, `taiki-e/install-action@v2`), while the
actions that hold elevated tokens are pinned by full SHA, e.g. in
`covector-version-or-publish.yml`:

```yaml
uses: peter-evans/repository-dispatch@ff45666b9427631e3450c54a1bcbee4d9ff4d7c0 # 3.0.0
```

and `softprops/action-gh-release@50195ba7...` in `publish-cli-rs.yml`. The organization also
maintains a hard fork of `create-pull-request`, explained in `extras/tauri/ARCHITECTURE.md`:
"Because this is a very risky (potentially destructive) github action, we forked it in order to
have strong guarantees that the code we think is running is actually the code that is running."
Dependency update security is belt and suspenders: `extras/tauri/dependabot.yml` sets
`open-pull-requests-limit: 0` on every ecosystem (security alerts only, no version PR spam) and
`extras/tauri/renovate.json` does the version bumping with `"minimumReleaseAge": "3 days"` so a
compromised release cannot land the day it is published.

## 7. Testing

Unit tests are conventional `#[cfg(test)] mod tests` blocks colocated with the code
(`extras/tauri/crates/tauri-utils/src/acl/identifier.rs`, `.../platform.rs`, `.../html.rs`, and
many more). Integration tests get their own workspace crates under `extras/tauri/crates/tests/`
so their dependencies (tempfile, insta, fixtures) never touch published manifests.

Harness infrastructure is the standout. The `tauri` crate ships a full mock of its own runtime
behind the `test` feature: `extras/tauri/crates/tauri/src/test/mock_runtime.rs` (1,413 lines)
implements the entire `Runtime` trait without a windowing system, and
`extras/tauri/crates/tauri/src/test/mod.rs` exposes `mock_builder`, `mock_context`, and
`get_ipc_response` so downstream apps can unit test IPC commands headlessly. The module doc
carries a compilable example that invokes a `#[tauri::command]` and asserts on the response.

Snapshot testing uses insta with per-platform snapshot routing.
`extras/tauri/crates/tests/acl/src/lib.rs` iterates capability fixtures and redirects
platform-specific cases to per-OS snapshot directories:

```rust
let mut settings = insta::Settings::clone_current();
settings.set_snapshot_path(
  if fixture_entry.path().file_name().unwrap() == "platform-specific-permissions" {
    Path::new("../fixtures/snapshots").join(Target::current().to_string())
  } else {
    Path::new("../fixtures/snapshots").to_path_buf()
  },
);
```

with committed `.snap` files under `extras/tauri/crates/tests/acl/fixtures/snapshots/`. The CLI
snapshots Xcode project rewrites the same way in `extras/tauri/crates/tauri-cli/src/helpers/pbxproj.rs`
against fixtures in `extras/tauri/crates/tauri-cli/tests/fixtures/pbxproj/`.

Property testing uses both proptest and quickcheck, at serious case counts.
`extras/tauri/crates/tauri/src/event/listener.rs` drives the listener map with
`proptest! { #![proptest_config(ProptestConfig::with_cases(10000))] ... }` over generated event
names, and `extras/tauri/crates/tauri/src/ipc/format_callback.rs` defines `Arbitrary` impls for
`CallbackFn` and a `JsonStr` wrapper, then quickchecks that arbitrary strings survive the
JS-injection escaping (`#[quickcheck] fn qc_formatting(f: CallbackFn, a: String) -> bool`). This
is exactly the code where a missed escape is a security bug, and the tests are aimed there.

Process-level behavior gets real end-to-end coverage: `extras/tauri/crates/tests/restart/tests/restart.rs`
compiles a helper binary, copies it into a `tempfile::TempDir`, invokes it through single and
nested symlinks, and asserts on stdout, including the expected macOS security failure when the
`process-relaunch-dangerous-allow-symlink-macos` feature is absent. The Node CLI has its own
`__tests__` suite under `extras/tauri/packages/cli/`, run in the pre-release gate of
`covector-version-or-publish.yml` (`run-integration-tests` job runs `cargo test --test '*' -- --ignored`
and `pnpm test` on all three desktop OSes before any publish is attempted).

Benchmarks are a custom harness, not criterion: `extras/tauri/bench/src/run_benchmark.rs` builds
three fixture apps with nightly `-Z build-std` (see `extras/tauri/.github/workflows/bench.yml`),
measures startup, memory, thread and syscall counts by parsing `strace -c -f` output, and the
workflow pushes JSON results to the separate `tauri-apps/benchmark_results` repository on every
`dev` push, giving a public time series rather than a per-PR pass/fail.

## 8. Error handling and API design

Every library crate defines a `thiserror` enum, and the public one in
`extras/tauri/crates/tauri/src/error.rs` is `#[non_exhaustive]` (the repo has 54 `non_exhaustive`
attributes overall) with platform- and feature-gated variants:

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
  /// Runtime error.
  #[error("runtime error: {0}")]
  Runtime(#[from] tauri_runtime::Error),
  /// Window label must be unique.
  #[error("a window with label `{0}` already exists")]
  WindowLabelAlreadyExists(String),
```

`anyhow` appears in the app-facing surface (setup hooks return boxed errors), and the same file
shows a narrowly justified unsafe wrapper: `SetupError` holds `Box<dyn std::error::Error>` and
implements Send/Sync manually with the comment "safety: the setup error is only used on the main
thread and we exit the process immediately."

The CLI is the more interesting case study. `extras/tauri/crates/tauri-cli/src/error.rs` replaces
anyhow with a purpose-built system: a small `Error` enum whose `Fs` variant forces every
filesystem failure to carry both a static context string and the offending path, an `ErrorExt`
extension trait (`fn fs_context(self, context: &'static str, path: impl Into<PathBuf>)`), a
hand-rolled `Context`/`with_context` trait mirroring anyhow's API for both `Result` and `Option`,
and a crate-private `bail!` macro. Callers read like
`write(&output, completions).fs_context("failed to write to completions", output)?`
(`extras/tauri/crates/tauri-cli/src/completions.rs`). Exit discipline is centralized in
`extras/tauri/crates/tauri-cli/src/lib.rs`:

```rust
pub fn run<I, A>(args: I, bin_name: Option<String>)
{
  if let Err(e) = try_run(args, bin_name) {
    log::error!("{e}");
    exit(1);
  }
}
```

with a fallible `try_run` kept public so the napi wrapper in `extras/tauri/packages/cli` can embed
the CLI without process exits. Unsupported platforms fail fast in
`extras/tauri/crates/tauri-cli/src/main.rs` with a `#[cfg(not(...))] fn main()` that prints one
line and exits 1.

API design centers on builders and sealed traits. `WebviewWindowBuilder<'a, R: Runtime, M: Manager<R>>`
(`extras/tauri/crates/tauri/src/webview/webview_window.rs`) is the canonical consuming builder
ending in `pub fn build(self) -> crate::Result<WebviewWindow<R>>`. The `Manager` trait in
`extras/tauri/crates/tauri/src/lib.rs` is publicly implementable in name only:
`pub trait Manager<R: Runtime>: sealed::ManagerBase<R>` where `sealed::ManagerBase` lives in a
private module, so App, AppHandle, Window, and Webview share a rich default-method surface while
the crate keeps the right to evolve the base. Visibility is tightly held elsewhere too:
`EventName` and `StateManager::set` are `pub(crate)`, and API removals go through `#[deprecated]`
attributes first (five sites, e.g. `extras/tauri/crates/tauri/src/window/mod.rs:876`).

## 9. Deep Rust usage

1. Generic runtime abstraction with associated types.
   `extras/tauri/crates/tauri-runtime/src/lib.rs` defines
   `pub trait RuntimeHandle<T: UserEvent>: Debug + Clone + Send + Sync + Sized + 'static` with
   `type Runtime: Runtime<T, Handle = Self>`, a mutually-constrained pair that lets the whole
   framework be generic over the windowing backend while keeping handle and runtime types locked
   to each other at compile time. The mock runtime in tests is just another implementor.

2. Sealed supertraits. `extras/tauri/crates/tauri/src/lib.rs` pairs
   `pub trait Manager<R: Runtime>: sealed::ManagerBase<R>` with a private `mod sealed`, exposing
   dozens of default methods (`app_handle`, `config`, `state`) that all bottom out in three
   private accessors. Downstream code gets a uniform API on six different types without any of
   them being able to be reimplemented incorrectly.

3. An axum-style extractor trait with a blanket impl.
   `extras/tauri/crates/tauri/src/ipc/command.rs` declares
   `pub trait CommandArg<'de, R: Runtime>: Sized { fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError>; }`
   and then `impl<'de, D: Deserialize<'de>, R: Runtime> CommandArg<'de, R> for D`, so any serde
   type is automatically a command parameter while `State`, `Window`, and `AppHandle` provide
   specialized impls. The `#[tauri::command]` proc macro generates calls into this trait.

4. Lifetime-carrying newtypes for managed state.
   `extras/tauri/crates/tauri/src/state.rs` defines `pub struct State<'r, T>(&'r T)` whose
   `inner(&self) -> &'r T` deliberately returns the longer lifetime, and backs it with a
   `StateManager` built on `HashMap<TypeId, Box<dyn Any + Sync + Send>, BuildHasherDefault<IdentHash>>`,
   where `IdentHash` is a custom identity hasher because `TypeId` is already a hash. The one
   `unsafe` block extending a borrow's lifetime is fenced by written invariants:
   "Once you insert a value, you can't remove/mutated/move it anymore", and the API that would
   violate it (`unmanage`) is itself marked `unsafe` with its own SAFETY contract.

5. Storage-generic validated newtypes. `extras/tauri/crates/tauri/src/event/event_name.rs` has
   `pub(crate) struct EventName<S = String>(S)` validated in `new`, with `impl Copy for EventName<&str>`,
   a `const fn from_str(s: &'static str)` restricted to `'static` "to discharge the preconditions"
   in const contexts, and an `as_str_event` view type, giving one validation point across owned
   and borrowed use without allocation.

6. Parser newtypes as state machines. `extras/tauri/crates/tauri-utils/src/acl/identifier.rs`
   validates permission identifiers byte by byte through a `ValidByte` enum in
   `impl TryFrom<String> for Identifier`, stores the separator position as `Option<NonZeroU8>`
   for niche-packing, and enumerates every failure mode in a `ParseIdentifierError` thiserror
   enum (`TrailingHyphen`, `PrefixWithoutBase`, `Humongous(usize)`).

7. Cow-first asset APIs. `extras/tauri/crates/tauri-utils/src/assets.rs` types the asset iterator
   as `pub type AssetsIter<'a> = dyn Iterator<Item = (Cow<'a, str>, Cow<'a, [u8]>)> + 'a;` so
   embedded assets are served borrowed while compressed assets decompress into `Cow::Owned`, and
   the test helper `NoopAsset` in `extras/tauri/crates/tauri/src/test/mod.rs` satisfies the same
   trait with a borrowed iterator pipeline.

8. Main-thread affinity encoded with unsafe wrappers and ManuallyDrop.
   `extras/tauri/crates/tauri/src/menu/mod.rs` generates menu wrappers via a `gen_wrappers!`
   macro whose `Drop` impl moves the inner muda handle back to the main thread:

   ```rust
   let inner = unsafe { ManuallyDrop::take(&mut self.inner) };
   // SAFETY: inner was created on main thread and is being dropped on main thread
   let inner = $crate::UnsafeSend(inner);
   let _ = self.app_handle.run_on_main_thread(move || {
     drop(inner.take());
   });
   ```

   together with the `run_item_main_thread!` macro that round-trips any closure through an
   `std::sync::mpsc` channel to the event loop. Every `unsafe impl Send/Sync` in the tree carries
   a `# Safety` comment naming the thread invariant.

9. Message-passing concurrency instead of shared mutation.
   `extras/tauri/crates/tauri-runtime-wry/src/lib.rs` funnels all window operations through
   `pub enum Message<T: 'static>` and `send_user_message`, with a `getter!`/`window_getter!`/
   `webview_getter!` macro family that packages a oneshot channel into the message and blocks on
   the reply. Shared state that must be mutated (`AppManager` in
   `extras/tauri/crates/tauri/src/manager/mod.rs`) uses plain `Mutex` fields per concern
   (`plugins: Mutex<PluginStore<R>>`, `resources_table: Arc<Mutex<ResourceTable>>`) rather than
   one god lock.

10. Build-script cfg aliases with check-cfg registration.
    `extras/tauri/crates/tauri/build.rs` defines readable platform predicates once:

    ```rust
    fn alias(alias: &str, has_feature: bool) {
      println!("cargo:rustc-check-cfg=cfg({alias})");
      ...
    }
    let mobile = target_os == "ios" || target_os == "android";
    alias("desktop", !mobile);
    alias("mobile", mobile);
    ```

    so the code base writes `#[cfg(desktop)]`, `#[cfg(mobile)]`, and `#[cfg(dev)]` instead of
    repeating target lists, and `rustc-check-cfg` keeps `unexpected_cfgs` clean.

11. Runtime resource tables with checked Arc downcasting.
    `extras/tauri/crates/tauri/src/resources/mod.rs` defines
    `pub trait Resource: Any + 'static + Send + Sync` with a `close(self: Arc<Self>)` hook, and a
    `downcast_arc` that pointer-casts `&Arc<dyn Resource>` to `&Arc<T>` only after a
    `self.is::<T>()` TypeId check, a pattern adopted from Deno's resource table for JS-held
    handles to Rust objects.

12. Measured performance thresholds instead of folklore.
    `extras/tauri/crates/tauri/src/ipc/format_callback.rs` switches serialized payloads to
    `JSON.parse('...')` only above `const MIN_JSON_PARSE_LEN: usize = 10_240;`, documents the
    browser string limits with sourced comments (`MAX_JSON_STR_LEN = 2^30 - 2`), takes a closure
    parameter "to avoid unnecessary allocations", and guards the size assumption with a
    `#[cfg(debug_assertions)]` assert.

## 10. Documentation practices

Rustdoc is treated as a product surface. `extras/tauri/crates/tauri/src/lib.rs` opens with a
crate doc that enumerates every Cargo feature with its default status and consequences, sets a
branded `#![doc(html_logo_url = ..., html_favicon_url = ...)]`, and enables
`#![cfg_attr(docsrs, feature(doc_cfg))]` so feature-gated items render their requirements
(`#[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]` throughout
`extras/tauri/crates/tauri-runtime/src/lib.rs`). `missing_docs` is warned at crate level in the
library crates, and doc examples are runnable (the `test` module example builds an app against
the mock runtime). Docs.rs rendering is configured per crate via `[package.metadata.docs.rs]`
with five targets.

Repo-level documents divide by audience: `extras/tauri/ARCHITECTURE.md` for orientation (with an
explicit "What Tauri is NOT" section), `extras/tauri/.github/CONTRIBUTING.md` for process
(issue triage rules, signed-commit requirement, per-package development guides),
`extras/tauri/.github/RELEASING.md` as a maintainer runbook including a step-by-step recovery
procedure titled "Publishing failed, what to do?", and `extras/tauri/SECURITY.md` routing
disclosures through GitHub private vulnerability reporting with a 90-day publication target. The
`.changes/README.md` documents the change-file format contributors must follow. Issue intake is
structured YAML forms (`extras/tauri/.github/ISSUE_TEMPLATE/bug_report.yml` requires a
reproduction and the full `tauri info` output; `config.yml` diverts questions to Discord), and
`extras/tauri/.github/PULL_REQUEST_TEMPLATE.md` teaches title conventions by example, good and
bad. Two commissioned security audit reports are committed under `extras/tauri/audits/`.

## 11. Release and distribution

Versioning is per-crate semver orchestrated by covector. Every user-visible PR adds a markdown
file to `extras/tauri/.changes/` whose frontmatter names the packages and the bump with a change
tag, e.g. `extras/tauri/.changes/build-config-path.md`:

```md
---
"tauri-build": minor:feat
---

Added `Attributes::config_path` to customize config path, deprecated `CodegenContext::config_path` in favor of this
```

`extras/tauri/.changes/config.json` maps tags (`feat`, `enhance`, `bug`, `perf`, `sec`, `deps`,
`breaking`) to changelog headings, and defines the publish pipeline per package manager,
including a `prepublish` hook that runs `cargo audit` and embeds its output in the release notes.
CI enforces the discipline twice: `covector-status.yml` comments the pending bump on every PR,
and `check-change-tags.yml` rejects malformed change files.

On every push to `dev`, `extras/tauri/.github/workflows/covector-version-or-publish.yml` first
runs the cross-platform integration suite, then either opens/updates a signed-commit
"Apply Version Updates From Current Changes" PR (version mode) or publishes to crates.io and npm
(publish mode) with GitHub releases (`createRelease: true`) and npm provenance
(`NPM_CONFIG_PROVENANCE: true`, `id-token: write`). Successful publishes fan out via
`repository_dispatch` and `gh workflow run` to `publish-cli-rs.yml` (prebuilt `cargo-tauri`
binaries for six targets including `riscv64gc-unknown-linux-gnu` and Windows ARM),
`publish-cli-js.yml` (napi binaries for the npm CLI), and a docs-repo update event.

Distribution details worth copying: `extras/tauri/crates/tauri-cli/Cargo.toml` ships
`[package.metadata.binstall]` templates so `cargo binstall tauri-cli` fetches the GitHub release
artifact instead of compiling, with per-target `pkg-fmt` overrides. Shell completions are a
first-class subcommand: `extras/tauri/crates/tauri-cli/src/completions.rs` uses `clap_complete`
to emit Bash, Zsh, PowerShell, and Fish scripts, and generates them for each invocation wrapper
(`cargo tauri`, `pnpm tauri`, `npm run tauri`, `deno task tauri`) by synthesizing nested clap
`Command` trees per package manager.

## 12. Lessons for quinjet

quinjet already covers rustfmt, a maximal clippy wall, cargo-deny, taplo, typos, coverage, miri,
and mutants. The practices below are the ones tauri demonstrates that are still missing or
partially covered, each with its mechanism.

1. Add cargo-vet on top of cargo-deny. `cargo install cargo-vet`, `cargo vet init`, then import
   the same audit sets tauri does in `extras/tauri/supply-chain/config.toml`
   (`[imports.mozilla] url = "https://raw.githubusercontent.com/mozilla/supply-chain/main/audits.toml"`,
   plus google, bytecode-alliance, embark-studios, isrg, zcash). Run `cargo vet --locked` in CI;
   commit `supply-chain/{config.toml,audits.toml,imports.lock}`.

2. Run security audits on a schedule, not only on change. Tauri's
   `extras/tauri/.github/workflows/audit.yml` uses `on.schedule: cron: '0 0 * * *'` plus
   lockfile-path triggers with `rustsec/audit-check@v2`. quinjet can add a daily `schedule`
   trigger to its cargo-deny workflow so a freshly published RUSTSEC advisory fails the repo
   within a day even with no commits.

3. Version the advisory ignore list with reasons. Mirror `extras/tauri/.cargo/audit.toml`: every
   ignored id gets an inline justification and, where possible, the removal condition
   ("fixed when we remove kuchikiki from deps in v3").

4. Cancel superseded CI runs and slim the cache. Add to every workflow:
   `concurrency: { group: "${{ github.workflow }}-${{ github.ref }}", cancel-in-progress: true }`
   and `env: CARGO_PROFILE_DEV_DEBUG: 0`; cache with `Swatinem/rust-cache@v2`, and if any matrix
   exists, gate cache writes with `save-if:` the way `extras/tauri/.github/workflows/test-core.yml`
   only saves from the `--all-features` leg.

5. Pin the MSRV in CI as an exact toolchain. Tauri sets `rust-version = "1.90"` in
   `[workspace.package]` and tests with `dtolnay/rust-toolchain@1.90` / `toolchain: '1.90'`
   (`extras/tauri/.github/workflows/test-cli-rs.yml`). quinjet should add one job that builds and
   tests on `dtolnay/rust-toolchain@<rust-version>` so the `rust-version` key in Cargo.toml is
   enforced, not decorative.

6. Add a cargo-udeps job. Nightly toolchain plus `cargo udeps --all-targets --all-features`
   as in `extras/tauri/.github/workflows/udeps.yml`; for a single crate no matrix is needed, and
   known false positives go in `[package.metadata.cargo-udeps.ignore]` as in
   `extras/tauri/crates/tauri/Cargo.toml`.

7. Ship a size-tuned release profile. Copy the workspace profile from `extras/tauri/Cargo.toml`:
   `panic = "abort"`, `codegen-units = 1`, `lto = true`, `opt-level = "s"` (or `3` if TUI redraw
   speed matters more than size), `strip = true`, `incremental = false`. For a Git TUI installed
   by users, binary size and no-backtrace abort semantics are the right defaults.

8. Add `[package.metadata.binstall]` and release artifacts. Follow
   `extras/tauri/crates/tauri-cli/Cargo.toml`: a `pkg-url` template pointing at GitHub release
   tarballs plus per-target `pkg-fmt` overrides, and a release workflow that uploads
   `quinjet-<target>.tgz`/`.zip` per platform (matrix as in
   `extras/tauri/.github/workflows/publish-cli-rs.yml`), so `cargo binstall quinjet` works.

9. Make completions a subcommand, not a build artifact. Use `clap_complete::generate` behind a
   `quinjet completions --shell <shell> [--output <file>]` subcommand exactly as
   `extras/tauri/crates/tauri-cli/src/completions.rs` does, defaulting to stdout and using the
   fs-context error helper for the file path case.

10. Adopt insta snapshot tests for command output and TUI frames. Add `insta` as a
    dev-dependency, snapshot `ratatui::backend::TestBackend` buffers and CLI subcommand stdout,
    and use `insta::Settings::set_snapshot_path` for any platform-divergent output the way
    `extras/tauri/crates/tests/acl/src/lib.rs` routes snapshots per `Target::current()`.

11. Property-test the parsing layers. quinjet parses refnames, revision expressions, and
    keybinding specs; give them `proptest!` blocks with elevated case counts
    (`ProptestConfig::with_cases(10000)` as in `extras/tauri/crates/tauri/src/event/listener.rs`)
    and quickcheck `Arbitrary` impls for domain newtypes as in
    `extras/tauri/crates/tauri/src/ipc/format_callback.rs`.

12. Add a real process-level integration test. Model it on
    `extras/tauri/crates/tests/restart/tests/restart.rs`: run the built `quinjet` binary with
    `std::process::Command` inside a `tempfile::TempDir` containing a scripted Git repository,
    and assert on stdout/stderr and exit codes for each CLI subcommand. This tests the actual
    public surface, including argv parsing and exit discipline, which unit tests cannot.

13. Enforce release-note discipline in CI. Even single-crate, the covector pattern transfers: a
    `.changes/*.md` file per user-visible PR with a bump tag taxonomy
    (`extras/tauri/.changes/config.json` tags: feat, enhance, bug, perf, sec, deps, breaking), a
    PR check that a change file exists and its tags are valid
    (`extras/tauri/.github/workflows/check-change-tags.yml` plus
    `extras/tauri/.scripts/ci/check-change-tags.js`), and changelog generation at release time.
    git-cliff with commit conventions is an acceptable single-crate substitute, but the check
    must run on PRs, not at release.

14. Keep generated files honest with a diff check. If quinjet ever commits generated artifacts
    (default keymap docs, config schema, wiki pages), regenerate them in CI and fail on
    `git diff` non-empty via a two-line script like `extras/tauri/.scripts/ci/has-diff.sh`,
    triggered only when the sources change (`dorny/paths-filter@v3`).

15. Use `.cargo/config.toml` `[env]` for test-environment quirks instead of Makefile exports.
    Tauri sets `__TAURI_WORKSPACE__ = "true"` there with a comment linking the issue it works
    around (`extras/tauri/.cargo/config.toml`); the same slot suits quinjet variables like a
    deterministic `GIT_AUTHOR_DATE` for snapshot-stable test repositories.

16. Split update automation from update security. Configure Renovate with
    `"minimumReleaseAge": "3 days"` and grouped ecosystem rules as in
    `extras/tauri/renovate.json`, and if dependabot stays enabled, set
    `open-pull-requests-limit: 0` as in `extras/tauri/dependabot.yml` so it only surfaces
    security alerts.

17. Mark the public error enum `#[non_exhaustive]` and gate variants by cfg. If quinjet's
    library-side error enum is exposed for scripting or plugins, follow
    `extras/tauri/crates/tauri/src/error.rs`: `#[non_exhaustive]`, one display string per
    variant, `#[from]` conversions only where the source is unambiguous, and feature/platform
    cfg on variants that do not exist everywhere.

18. Demote noisy third-party log targets, not the global level. The CLI's logger in
    `extras/tauri/crates/tauri-cli/src/lib.rs` applies
    `.filter(Some("handlebars"), verbosity_level(n.saturating_sub(1)).to_level_filter())` per
    chatty crate with a comment explaining each; quinjet's tracing/env_logger setup can carry the
    same per-target demotions for git2 or other verbose dependencies.
