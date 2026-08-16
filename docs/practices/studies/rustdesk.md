# rustdesk/rustdesk (120919 stars)

## 1. What the project is and how big it is

RustDesk is an open-source remote desktop application: a self-hostable alternative to TeamViewer and AnyDesk. The performance-critical core (screen capture, video encoding, input injection, clipboard sync, networking, IPC) is Rust; the desktop and mobile UI is Flutter, with a legacy Sciter UI kept as a fallback for 32-bit Windows. Industry adopts it because it ships on every major platform from one codebase, because the rendezvous and relay servers can be self-hosted for privacy, and because the capture and codec pipeline is genuinely fast.

Measured from the clone at extras/rustdesk:

- 242 Rust source files totaling about 149,000 lines of Rust (the `libs/hbb_common` submodule, declared in extras/rustdesk/.gitmodules, is fetched separately and adds more on top of this).
- 9 `Cargo.toml` manifests: the root package plus `libs/scrap`, `libs/enigo`, `libs/clipboard`, `libs/virtual_display` (and its nested `dylib`), `libs/portable`, `libs/remote_printer`, and `libs/libxdo-sys-stub`.
- The root workspace lists 8 members and excludes one directory (extras/rustdesk/Cargo.toml):

```toml
[workspace]
members = ["libs/scrap", "libs/hbb_common", "libs/enigo", "libs/clipboard", "libs/virtual_display", "libs/virtual_display/dylib", "libs/portable", "libs/remote_printer"]
exclude = ["vdi/host"]
```

- Version 1.4.9, `rust-version = "1.75"`, edition 2021 (extras/rustdesk/Cargo.toml).
- 52 translation modules under extras/rustdesk/src/lang.
- The largest files show where the complexity lives: `src/server/connection.rs` (7,701 lines), `src/platform/windows.rs` (4,747), `src/client.rs` (4,443), `src/common.rs` (3,101), `src/flutter_ffi.rs` (3,003).
- 3,524 lines of GitHub Actions YAML across 11 workflow files under extras/rustdesk/.github/workflows.

## 2. Repository layout

Top-level tree (from `ls` of extras/rustdesk, packaging noise trimmed):

```text
extras/rustdesk/
|-- Cargo.toml            root package + workspace definition
|-- Cargo.lock            committed lockfile
|-- build.rs              native code compilation, version generation
|-- build.py              1,164-line packaging driver (deb/rpm/msi/dmg/apk)
|-- vcpkg.json            C/C++ dependency manifest (aom, libvpx, opus, ffmpeg, ...)
|-- Dockerfile            reproducible Linux build container
|-- src/                  the application: client, server, ui glue, platform code
|   |-- server/           capture/input/clipboard/terminal services
|   |-- client/           connection and io loop
|   |-- platform/         windows.rs, linux.rs, macos.rs, delegate .cc/.mm files
|   |-- lang/             52 translation tables as Rust modules
|   `-- ipc/              inter-process channel handlers
|-- libs/                 reusable capability crates
|   |-- hbb_common        (git submodule) protocol, config, tokio re-exports
|   |-- scrap/            screen capture + codecs, per-backend modules
|   |-- enigo/            keyboard/mouse simulation (forked library)
|   |-- clipboard/        file-copy-paste clipboard engine
|   |-- virtual_display/  dlopen wrapper over a driver dylib
|   |-- portable/         self-extracting Windows packer binary
|   |-- remote_printer/   Windows printer driver integration
|   `-- libxdo-sys-stub/  local [patch.crates-io] stub crate
|-- flutter/              the Flutter UI project (Dart)
|-- res/                  packaging: DEBIAN/, msi/ (WiX), rpm specs, PKGBUILD,
|                         desktop files, icons, bump.sh, CI helper scripts
|-- appimage/  flatpak/  fastlane/   per-channel packaging recipes
|-- examples/             ipc.rs manual harness
`-- docs/                 README/CONTRIBUTING/SECURITY in ~30 languages
```

The split works because each `libs/` crate encapsulates one capability behind a small API, while `src/` owns orchestration and platform policy. `scrap` is a fork of an upstream crate that kept its own identity (extras/rustdesk/libs/scrap/Cargo.toml still says `description = "Screen capture made easy."`), which keeps capture testable through its own `examples/` harnesses without booting the whole application. Shared protocol types live in the `hbb_common` submodule so the separate server repository can reuse them.

## 3. Cargo manifest practices

The root manifest (extras/rustdesk/Cargo.toml) predates `workspace.package` inheritance and instead uses a classic root-package-plus-members layout. Its notable practices:

**One package, three crate types, three binaries.** The library is compiled for FFI consumption by Flutter:

```toml
[lib]
name = "librustdesk"
crate-type = ["cdylib", "staticlib", "rlib"]
```

plus `[[bin]]` entries for `naming` (license-name generator, extras/rustdesk/src/naming.rs) and `service`, with `default-run = "rustdesk"` so `cargo run` stays unambiguous.

**Feature flags model real product variants.** Audio resampling is selectable (`use_samplerate`, `use_rubato`, `use_dasp` with `default = ["use_dasp"]`), codecs are gated (`hwcodec`, `vram`, `mediacodec`), and the UI toolkit itself is a feature (`flutter = ["flutter_rust_bridge"]`). Optional dependencies use the `dep:` syntax so features do not leak implicit feature names:

```toml
unix-file-copy-paste = [
    "dep:x11-clipboard",
    "dep:x11rb",
    "dep:percent-encoding",
    "dep:once_cell",
    "clipboard/unix-file-copy-paste",
]
```

Features that are additive risk get their own gate: `drm-wake = ["drm"]` exists purely so the input-injecting display wake can be compiled out while keeping DRM capture, and the manifest documents why in place (extras/rustdesk/Cargo.toml).

**Target-specific dependency tables carry the platform matrix.** There are separate `[target.'cfg(...)'.dependencies]` sections for Windows, macOS, Linux, Android, combinations like `cfg(any(target_os = "macos", target_os = "linux"))`, and negations like `cfg(not(target_os = "linux"))` (used to drop `cpal` on Linux, with a link to the discussion explaining why). The Windows section enumerates 14 `winapi` features and 18 `windows` crate features explicitly rather than enabling umbrella features.

**Forks are centralized under one GitHub org.** Every patched dependency points at `github.com/rustdesk-org/...` (`rdev`, `cpal`, `arboard`, `magnum-opus`, `tao`, `pam`, `evdev`, and more), which makes the fork surface auditable at a glance.

**`[patch.crates-io]` swaps in a local stub crate:**

```toml
[patch.crates-io]
libxdo-sys = { path = "libs/libxdo-sys-stub" }
```

so Wayland-only systems without libxdo can still build and run (extras/rustdesk/Cargo.toml).

**Profiles are tuned for shipping:**

```toml
[profile.release]
lto = true
codegen-units = 1
panic = 'abort'
strip = true
rpath = true

[profile.dev]
debug = 1
```

with a citation of the min-sized-rust guide in the manifest comment. `debug = 1` in dev keeps line tables without full debuginfo, a deliberate compile-time and disk saving.

**Build-time metadata sections** configure the Windows resource block (`[package.metadata.winres]`) and the macOS bundle (`[package.metadata.bundle]` with `osx_minimum_system_version = "10.14"`).

**`.cargo/config` carries linker policy** (extras/rustdesk/.cargo/config): static CRT on all three Windows MSVC targets, a macOS `-sectcreate __CGPreLoginApp` link arg so the binary can run at the login screen, and `git-fetch-with-cli = true` for the many git dependencies.

## 4. Formatting

The repository uses default rustfmt for the application; the only rustfmt configuration on disk is one file, extras/rustdesk/libs/enigo/rustfmt.toml, whose entire content is:

```toml
wrap_comments = true
```

That single setting makes rustfmt rewrap the long `//!` prose documentation in that crate to the line limit; the rest of the codebase accepts rustfmt defaults (4-space indent, 100-column max_width, edition-aware imports). There is no `.editorconfig`. Line-ending policy is delegated to git via extras/rustdesk/.gitattributes:

```text
* text=auto
```

A `cargo fmt -- --check` CI job exists in extras/rustdesk/.github/workflows/ci.yml but is commented out (lines 28-39), so formatting is enforced socially rather than mechanically. Non-Rust formatting: the Flutter tree relies on the Dart analyzer/formatter configured by extras/rustdesk/flutter/analysis_options.yaml, and CI installs the `rustfmt` component wherever `flutter_rust_bridge_codegen` runs, because the bridge generator formats its emitted Rust (extras/rustdesk/.github/workflows/bridge.yml passes `components: "rustfmt"`).

## 5. Linting

There are no `[lints]` tables and no `clippy.toml` anywhere in the repository. The linting posture is the opposite of a strict wall, and it is instructive to see how a mature FFI-heavy project manages that honestly:

- Crate-level `#![allow]` headers are placed exactly where raw bindings make the default lints wrong, for example extras/rustdesk/libs/scrap/src/common/convert.rs:

```rust
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]
#![allow(dead_code)]
```

- Item-level `#[allow(...)]` appears 65 times under `src/`, always local to the offending item (for example `#[allow(non_snake_case)]` on `test_RustDesk_interval` in extras/rustdesk/src/common.rs).
- A minimum-supported-rust-version clippy job was written and then commented out in extras/rustdesk/.github/workflows/ci.yml (lines 41-66); its retained flags (`--locked --all-targets --all-features -- --allow clippy::unknown_clippy_lints`) show the intended shape: clippy on the MSRV toolchain so contributors on newer toolchains cannot introduce warnings the MSRV cannot silence.
- Dart-side linting is real and active: extras/rustdesk/flutter/analysis_options.yaml includes `package:lints/recommended.yaml` and disables exactly two rules by name.

The philosophy: correctness gates (build + test on the pinned toolchain, `--locked` everywhere) are enforced; style gates are not, because a codebase with 799 `unsafe` occurrences spread across platform bindings would spend enormous effort keeping pedantic lints green for little payoff. The check infrastructure that does exist is domain-specific instead: checksum verification of downloaded driver binaries inside CI (extras/rustdesk/.github/workflows/flutter-build.yml compares `Get-FileHash` output against a fetched `sha256sums` file before extracting the printer driver).

## 6. CI/CD

Eleven workflows under extras/rustdesk/.github/workflows. The architecture is a reusable core with thin trigger shells:

**flutter-build.yml (2,477 lines) is the single build definition.** It is declared `on: workflow_call` with typed inputs:

```yaml
on:
  workflow_call:
    inputs:
      upload-artifact:
        type: boolean
        default: true
      upload-tag:
        type: string
        default: "nightly"
```

Three shells invoke it: extras/rustdesk/.github/workflows/flutter-ci.yml (pull requests and pushes to master, `upload-artifact: false`), flutter-nightly.yml (`cron: "0 0 * * *"`, uploads to the `nightly` tag), and flutter-tag.yml (tag patterns like `v[0-9]+.[0-9]+.[0-9]+` and `[0-9]+.[0-9]+.[0-9]+-[0-9]+`, uploads to `${{ github.ref_name }}`). One build graph, three release channels, zero duplication.

**Job coverage inside flutter-build.yml:** `generate-sbom` (Syft producing `cyclonedx-json`, published to the release), `generate-bridge` (flutter_rust_bridge codegen, uploaded as an artifact all platform jobs download, with two matrix rows because Windows arm64 needs a bridge generated by a newer Flutter), `build-for-windows-flutter` (x64 on windows-2022 and aarch64 on windows-11-arm), `build-for-windows-sciter` (32-bit fallback UI), `build-rustdesk-ios`, `build-for-macOS`, `build-rustdesk-android` plus a universal-apk job, `build-rustdesk-linux` plus dedicated drm and sciter variants, `build-appimage`, `build-flatpak`, `build-rustdesk-web`, and `publish_unsigned`. Every matrix sets `fail-fast: false` so one platform failing does not hide others.

**Version pinning is exemplary.** Every third-party action is pinned to a full commit SHA with the human-readable version as a trailing comment:

```yaml
- uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5 # v4
- uses: Swatinem/rust-cache@e18b497796c12c097a38f9edb9d0641fb99eee32 # v2
```

The same discipline extends to toolchains: the env block at the top of flutter-build.yml pins `RUST_VERSION: "1.75"`, `FLUTTER_VERSION: "3.24.5"`, `VCPKG_COMMIT_ID`, `NDK_VERSION`, `LLVM_VERSION`, and each pin carries a comment explaining the constraint (for example that Rust 1.78's i128 ABI change breaks the Sciter build, with links). Even a helper DLL built from a sibling repository is pinned: extras/rustdesk/.github/workflows/third-party-RustDeskTempTopMostWindow.yml clones and then `git checkout ecd8d6a139eee76845ea66423fb739af450fda90` before `msbuild`.

**Caching is layered per toolchain:** `Swatinem/rust-cache` with `prefix-key: ${{ matrix.job.os }}` for cargo, `VCPKG_BINARY_SOURCES: "clear;x-gha,readwrite"` plus an exported `ACTIONS_CACHE_URL`/`ACTIONS_RUNTIME_TOKEN` step so vcpkg's binary cache rides GitHub's cache backend, `actions/cache` for the generated bridge keyed on the Flutter version, and `subosito/flutter-action` with `cache: true`. When caches wedge there is a manual escape hatch: extras/rustdesk/.github/workflows/clear-cache.yml is a `workflow_dispatch`-only job with `permissions: actions: write` that deletes every repository cache through the REST API, then runs a second purge action because its own note admits the first purge is incomplete.

**Security-relevant hardening:** jobs that publish declare `permissions: contents: write` explicitly; code signing runs only when secrets exist (`if: env.UPLOAD_ARTIFACT == 'true' && env.SIGN_BASE_URL != '-2'`) and goes through an external signing service (`python3 res/job.py sign_files ./rustdesk/`), while unsigned artifacts are still uploaded separately so forks without secrets get usable builds. Supply-chain refresh is automated but review-gated: extras/rustdesk/.github/workflows/update-webpki-roots.yml runs weekly (`cron: "0 3 * * 1"`), updates the pinned Mozilla root store in every lockfile, and opens a PR rather than pushing, under a concurrency group with `cancel-in-progress: false` so a manual dispatch cannot cancel a run that already pushed. Dependabot watches the submodule pointer daily (extras/rustdesk/.github/dependabot.yml, `package-ecosystem: "gitsubmodule"`).

**Cost control:** both ci.yml and flutter-ci.yml use `paths-ignore` for `docs/**`, `README.md`, and packaging directories, and the Linux job starts with a pinned `jlumbroso/free-disk-space` step to make room for vcpkg-built ffmpeg. extras/rustdesk/.github/workflows/ci.yml itself is the fast Rust-only lane: one `x86_64-unknown-linux-gnu` build with `cargo build --locked` followed by tests.

## 7. Testing

There is no top-level `tests/` directory; all Rust tests are inline `#[cfg(test)] mod tests` blocks colocated with the code, 30+ modules containing 202 `#[test]` functions and 14 `#[tokio::test]` functions across `src/` and `libs/`. That choice fits a codebase where most logic is platform-gated: the test sits inside the same `cfg` scope as the code it exercises.

Representative tests worth studying:

- **Table-driven validation tests.** `src/common.rs` tests peer-id sanitization against a literal attack string:

```rust
let cases = [
    ("123456789", true),
    ("192.168.1.10:21118", true),
    (
        r#"1" & oWS.Run("cmd.exe /k whoami /priv",1,False) & ""#,
        false,
    ),
    ("", false),
```

(extras/rustdesk/src/common.rs, `untrusted_peer_id_validation`).

- **CLI surface contract tests.** extras/rustdesk/src/core_main.rs ends with a test that pins which flags are management commands versus service commands, so the IPC permission scope of the command surface cannot drift silently:

```rust
fn user_main_ipc_scope_cli_command_matches_management_commands_only() {
    for command in ["--password", "--set-unlock-pin", "--get-id", ...] {
        assert!(is_user_main_ipc_scope_cli_command(&args(&[command])));
    }
```

- **Byte-level protocol tests.** extras/rustdesk/src/server/terminal_service.rs tests a UTF-8 chunk reassembler on malformed and truncated sequences:

```rust
fn utf8_split_point_detects_incomplete_trailing_sequence() {
    let data = [b'a', 0xE4, 0xB8];
    assert_eq!(find_utf8_split_point(&data), 1);
}
```

- **Timing behavior tests** with a real tokio runtime comparing a custom throttled interval against `tokio::time::interval` tick-for-tick (extras/rustdesk/src/common.rs, `test_RustDesk_interval`).

CI runs `cargo test --workspace --no-fail-fast -- --skip test_get_cursor_pos --skip test_get_key_state` (extras/rustdesk/.github/workflows/ci.yml): the two skipped tests need a real display and cursor, and the skip list is computed per target so ARM builds test only `--lib --bin`. Hardware-dependent behavior that cannot run headless is covered instead by runnable example harnesses: extras/rustdesk/libs/scrap/examples contains `benchmark.rs` (the project's de facto codec benchmark), `screenshot.rs`, `record-screen.rs`, and `ffplay.rs`, with `docopt`, `repng`, and `quest` as dev-dependencies to drive them (extras/rustdesk/libs/scrap/Cargo.toml), and extras/rustdesk/examples/ipc.rs exercises the IPC channel manually. There is no fuzzing, property testing, or snapshot testing infrastructure in the repository.

## 8. Error handling and API design

**One Result alias for the whole application.** Everything fallible returns `hbb_common::ResultType<T>`, an anyhow-based alias re-exported from the shared submodule, together with `bail!` and `anyhow!` (for example extras/rustdesk/src/auth_2fa.rs: `fn new_totp(&self) -> ResultType<TOTP>`, and `.map_err(|_| anyhow!("Thread panicked"))??` to flatten a join handle's nested Results). Application code never defines ad hoc error enums for plumbing.

**Library crates own typed errors.** Where a crate has a real error taxonomy it uses `thiserror`, as in extras/rustdesk/libs/clipboard/src/lib.rs:

```rust
pub enum CliprdrError {
    #[error("failure to read file metadata or content, path: {path}, err: {err}")]
    FileError { path: String, err: std::io::Error },
    #[error("invalid request: {description}")]
    InvalidRequest { description: String },
    #[error("unknown cliprdr error")]
    Unknown(u32),
}
```

`scrap` defines its own `Error` plus `pub type Result<T> = std::result::Result<T, Error>` (extras/rustdesk/libs/scrap/src/common/mod.rs) and converts C return codes into it via generated macros that capture `module_path!()/file!()/line!()/column!()` context (see section 9).

**Panic policy.** Release builds set `panic = 'abort'` (extras/rustdesk/Cargo.toml), and release binaries register a native crash handler at startup: `register_breakdown_handler(breakdown_callback)` is imported under `#[cfg(not(debug_assertions))]` in extras/rustdesk/src/main.rs. So panics are treated as crashes to be reported, not control flow.

**Process lifecycle as API.** `core_main()` returns `Option<Vec<String>>` and documents the contract in a doc comment (extras/rustdesk/src/core_main.rs): `None` means "terminate now, the CLI handled everything", `Some(args)` means "continue into the GUI with these residual args". The GUI entry point in extras/rustdesk/src/main.rs is exactly `if let Some(args) = crate::core_main::core_main().as_mut() { ui::start(args); }`.

**Output discipline for a dual-mode binary.** Because a windows-subsystem GUI process has no console, user-facing CLI output goes through one macro that prints on Unix and raises a message box on Windows (extras/rustdesk/src/core_main.rs):

```rust
macro_rules! my_println{
    ($($arg:tt)*) => {
        #[cfg(not(windows))]
        println!("{}", format_args!($($arg)*));
        #[cfg(windows)]
        crate::platform::message_box(
            &format!("{}", format_args!($($arg)*))
        );
    };
}
```

**Visibility discipline.** extras/rustdesk/src/lib.rs is a wall of platform-gated module declarations: `pub mod` only where the FFI or the Flutter bridge needs the symbols, `mod` otherwise, with `/// cbindgen:ignore` doc-directives excluding modules from C header generation. Constants that are protocol strings are `pub const` next to the trait they serve (extras/rustdesk/src/privacy_mode.rs).

## 9. Deep Rust usage

Ten-plus concrete idioms, each cited:

1. **`cfg_if!` backend dispatch on custom cfg keys.** extras/rustdesk/libs/scrap/src/common/mod.rs selects `quartz` / `x11` / `dxgi` / `android` capture backends with nested `cfg_if!` blocks; the keys are custom cfgs emitted by the crate's build script (extras/rustdesk/libs/scrap/build.rs uses `target_build_utils`), so the platform decision is made once at build time and the rest of the crate uses clean flags instead of repeating `target_os` triples.

2. **A borrowed frame enum for zero-copy capture.** Captured frames never copy pixel data on the happy path: `pub enum Frame<'a> { PixelBuffer(PixelBuffer<'a>), Texture((*mut c_void, usize)) }` with `fn frame<'a>(&'a mut self, timeout: Duration) -> std::io::Result<Frame<'a>>` on the `TraitCapturer` trait (extras/rustdesk/libs/scrap/src/common/mod.rs). The frame borrows the capturer's buffer, and GPU-resident frames stay as raw texture pointers until encoding. `EncodeInput<'a>` continues the borrow into the encoder.

3. **`WouldBlock` as a data-level signal.** `would_block_if_equal(old: &mut Vec<u8>, b: &[u8]) -> std::io::Result<()>` returns `ErrorKind::WouldBlock` when a captured frame is identical to the previous one (extras/rustdesk/libs/scrap/src/common/mod.rs), letting the video loop reuse the same error path for "no new frame from the OS" and "frame unchanged, skip encode".

4. **A generic pub/sub service template with a marker-bound newtype.** extras/rustdesk/src/server/service.rs defines `pub trait Service: Send + Sync`, `pub trait Subscriber: Default + Send + Sync + 'static`, and `pub struct ServiceTmpl<T: Subscriber + From<ConnInner>>(Arc<RwLock<ServiceInner<T>>>)` with `pub type GenericService = ServiceTmpl<ConnInner>;`. Every capture service (video, audio, clipboard, terminal) instantiates this one template; the `From<ConnInner>` bound is how a raw connection is promoted into a typed subscriber. `EmptyExtraFieldService` then uses `Deref` to forward to the inner template, a cheap delegation idiom.

5. **An async trait as the UI abstraction boundary.** `#[async_trait] pub trait Interface: Send + Clone + 'static + Sized` (extras/rustdesk/src/client.rs) mixes sync methods, async methods (`async fn handle_hash(&self, ...)`), and default method bodies (`fn on_error(&self, err: &str) { self.msgbox("error", "Error", err, "") }`), so Sciter, Flutter desktop, and mobile sessions plug the same client engine.

6. **Macros that generate macros for FFI error walls.** extras/rustdesk/libs/scrap/src/common/mod.rs exports `generate_call_macro!`, which expands to a per-call-site macro that wraps an unsafe C call, transmutes the status to `i32`, and either logs or early-returns a `crate::Error::FailedCall` carrying `module_path!()/file!()/line!()/column!()`. Each codec module instantiates it once and gets uniform, located error reporting for hundreds of C calls.

7. **A declarative-macro dlopen wrapper with graceful degradation.** `make_lib_wrapper!` in extras/rustdesk/libs/virtual_display/src/lib.rs takes `field: Type` pairs, generates a struct holding `Option<Library>` plus one `Option<fn ...>` per symbol, and resolves each symbol at runtime with `lib.symbol::<$tp>(stringify!($field))`, logging and continuing on failure. The driver dylib being absent degrades features instead of crashing; `#[repr(C)] pub struct _MonitorMode` and `pub type PMonitorMode = *mut MonitorMode;` keep the C ABI explicit.

8. **Scoped runtimes via `#[tokio::main]` on inner functions.** Rather than one global runtime, blocking entry points wrap async bodies where needed: `#[tokio::main(flavor = "current_thread")]` appears on functions in extras/rustdesk/src/server.rs, src/lan.rs, src/tray.rs, and src/ui_interface.rs, giving a single-threaded runtime whose lifetime is the function call, which is exactly right for request-scoped async work called from sync UI code.

9. **Lock-scope discipline in iterator pipelines.** extras/rustdesk/src/server/video_service.rs collects under one mutex, then drops it before taking the next:

   ```rust
   let vec_display_idx: Vec<usize> = {
    let display_conn_ids = DISPLAY_CONN_IDS.lock().unwrap();
    display_conn_ids
        .iter()
        .filter_map(|(display_idx, conn_ids)| {
            if conn_ids.contains(&conn_id) { Some(*display_idx) } else { None }
        })
        .collect()
   };
   ```

   The block expression bounds the guard's lifetime; the subsequent loop locks a different map. Global state is `lazy_static!` (145 uses) over `Arc<Mutex<...>>` (91), `Arc<RwLock<...>>` (39), and atomics (84 `AtomicBool`/`AtomicUsize` sites), a pre-`OnceLock` vintage that is nonetheless consistently scoped like this.

10. **Serde-tagged state enums for IPC.** `#[derive(Serialize, Deserialize)] #[serde(tag = "t", content = "c")] pub enum PrivacyModeState` (extras/rustdesk/src/privacy_mode.rs) puts an explicit compact tag on the wire, and the `PrivacyMode: Sync + Send` trait behind it has multiple Windows implementations selected by string constants (`PRIVACY_MODE_IMPL_WIN_MAG`, `..._VIRTUAL_DISPLAY`) so the implementation choice is a runtime config value.

11. **`Cow` for computed-or-static strings.** `fn keysequence<'a>(key: Key) -> Cow<'a, str>` in extras/rustdesk/libs/enigo/src/linux/xdo.rs returns borrowed literals for named keys and owned strings for unicode escapes, avoiding allocation on the common path.

12. **Unsafe policy: quarantine by module.** 799 `unsafe` occurrences live almost entirely in `src/platform/` and the FFI layers of `libs/scrap`, `libs/enigo`, and `libs/clipboard`, each such module opening with the `#![allow(non_camel_case_types)]`-style header block (section 5). Cross-platform logic above those modules is safe Rust; `build.rs` compiles the companion `windows.cc` / `macos.mm` native shims (extras/rustdesk/build.rs) so C++ stays out of the Rust files entirely.

## 10. Documentation practices

- **Massively translated project docs.** extras/rustdesk/docs holds README in about 30 languages, CONTRIBUTING in 14, SECURITY and CODE_OF_CONDUCT in a dozen each, all as parallel `-XX.md` files. The root README.md links to them.
- **CONTRIBUTING encodes process, not style.** extras/rustdesk/docs/CONTRIBUTING.md requires claiming an issue before working on it, small independently-correct commits, tests relevant to the change, and a Developer Certificate of Origin sign-off (`git commit -s`) binding contributions to the license.
- **Issue intake is a structured YAML form.** extras/rustdesk/.github/ISSUE_TEMPLATE/bug_report.yaml makes description, reproduction, expected behavior, both-side OS versions, both-side RustDesk versions, and screenshots all `required: true`, which matters enormously for a two-endpoint product. extras/rustdesk/.github/ISSUE_TEMPLATE/config.yml sets `blank_issues_enabled: false` and routes feature requests and questions to GitHub Discussions.
- **Rustdoc where a crate is a library.** `libs/enigo` opens with long-form `//!` module documentation including `no_run` doctest examples (extras/rustdesk/libs/enigo/src/lib.rs); application modules instead favor targeted doc comments on contracts, like the `core_main` return-value semantics (extras/rustdesk/src/core_main.rs) and functional directives such as `/// cbindgen:ignore` in extras/rustdesk/src/lib.rs.
- **Comments explain constraints with receipts.** Workflow env pins, Cargo dependency choices, and feature definitions consistently carry links to the issue, discussion, or upstream blog post that forced the decision (for example the cpal-on-Linux exclusion in extras/rustdesk/Cargo.toml citing discussion 10197). There is no in-repo mdBook or docs site; user documentation lives outside the repository.

## 11. Release and distribution

- **Versioning is manifest-driven with a mechanical bump.** The version appears in extras/rustdesk/Cargo.toml, libs/portable/Cargo.toml, workflow env (`VERSION: "1.4.9"`), rpm specs, PKGBUILD, pubspec, flatpak and appimage recipes; extras/rustdesk/res/bump.sh rewrites all of them in one `sed` pass and then runs `cargo run` solely to regenerate `Cargo.lock`:

```bash
sed -i "s/\b$1\b/$2/g" res/*spec res/PKGBUILD flutter/pubspec.yaml Cargo.toml .github/workflows/*yml flatpak/*json appimage/*yml libs/portable/Cargo.toml
cargo run # to bump version in cargo lock
```

- **Releases are tag-triggered and nightly.** flutter-tag.yml fires on version-shaped tags, flutter-nightly.yml republishes the `nightly` prerelease every midnight, and both feed the same reusable build with `softprops/action-gh-release` (SHA-pinned) uploading with `prerelease: true`.
- **The distribution matrix is extreme:** signed `.msi` (WiX v4 solution under extras/rustdesk/res/msi with native CustomActions, built per-arch including ARM64) and a self-extracting portable `.exe` produced by the `libs/portable` packer crate; `.deb` via extras/rustdesk/res/DEBIAN maintainer scripts; four rpm spec variants and a `PKGBUILD` under res/; AppImage recipes per-arch under extras/rustdesk/appimage; flatpak under extras/rustdesk/flatpak; Android APKs plus F-Droid delivery, where extras/rustdesk/.github/workflows/fdroid.yml computes a monotonic version code (`X * 1e6 + Y * 1e4 + Z * 1e2 + A`) and publishes a `rustdesk-version.txt` under a dedicated `fdroid-version` tag for the updater to poll; fastlane metadata under extras/rustdesk/fastlane.
- **Supply-chain artifacts ship with the release:** the `generate-sbom` job attaches `rustdesk.sbom.json` (CycloneDX from Syft) to every uploaded tag (extras/rustdesk/.github/workflows/flutter-build.yml).
- **Signing is a service, not a secret file:** `res/job.py sign_files` posts artifacts to a signing endpoint configured by secrets, and unsigned artifact bundles are uploaded regardless so downstream packagers are never blocked.
- There is no CHANGELOG file in the repository; release notes live on the GitHub releases created by the tag workflow. As a GUI-first product it ships no man pages or shell completions; the CLI surface is hand-parsed in extras/rustdesk/src/core_main.rs.

## 12. Lessons for quinjet

quinjet already exceeds RustDesk on lint strictness, formatting enforcement, and test tooling, so the transferable value is in CI architecture, release mechanics, and a few testing patterns:

1. **Pin every GitHub Action to a full commit SHA with a trailing version comment.** Mechanism: in each workflow, `uses: owner/action@<40-char sha> # vN`, as done throughout extras/rustdesk/.github/workflows/ci.yml. Dependabot or Renovate can still bump the comment and SHA together.
2. **Make one reusable `workflow_call` build workflow and thin trigger shells.** Mechanism: move build/test/package steps into `.github/workflows/build.yml` with `on: workflow_call` and typed `inputs:` (boolean `upload-artifact`, string `upload-tag`), then add three callers for PR, nightly `schedule:`, and `push: tags:` exactly as extras/rustdesk/.github/workflows/flutter-ci.yml / flutter-nightly.yml / flutter-tag.yml do.
3. **Add a scheduled dependency-refresh workflow that opens a reviewable PR.** Mechanism: copy the shape of extras/rustdesk/.github/workflows/update-webpki-roots.yml: `schedule:` cron plus `workflow_dispatch:`, a `concurrency:` group with `cancel-in-progress: false`, `permissions: contents: write, pull-requests: write`, run `cargo update`, diff-gate with `git diff --quiet`, and `gh pr create` from a fixed branch. For quinjet the payload is `cargo update` plus regenerated `cargo deny` output instead of webpki roots.
4. **Attach an SBOM to every release.** Mechanism: a job using `anchore/sbom-action/download-syft`, then `syft dir:. -o cyclonedx-json=quinjet.sbom.json`, uploaded via the release action, mirroring the `generate-sbom` job in extras/rustdesk/.github/workflows/flutter-build.yml.
5. **Key Rust caches per matrix row.** Mechanism: `Swatinem/rust-cache` with `prefix-key: ${{ matrix.os }}` (extras/rustdesk/.github/workflows/flutter-build.yml) so Linux, macOS, and Windows caches never collide; add a manual `clear-cache.yml` (`workflow_dispatch`, `permissions: actions: write`, `github.rest.actions.deleteActionsCacheById`) for wedged caches, copied from extras/rustdesk/.github/workflows/clear-cache.yml.
6. **Use `paths-ignore` to keep docs changes out of the build lane.** Mechanism: on both `pull_request` and `push`, ignore `docs/**` and `README.md` as in extras/rustdesk/.github/workflows/ci.yml; pair it with a docs-only workflow if the wiki generation needs its own gate.
7. **Set matrix `fail-fast: false` everywhere.** Mechanism: `strategy: fail-fast: false` on every OS matrix, so a Windows-only failure still yields Linux and macOS signal in the same run, as every RustDesk matrix does.
8. **Write a CLI-surface contract test.** Mechanism: a unit test enumerating every subcommand and asserting its classification, modeled on `user_main_ipc_scope_cli_command_matches_management_commands_only` in extras/rustdesk/src/core_main.rs; for quinjet, iterate `Command::get_subcommands()` from the clap definition and assert each maps to a TUI operation and vice versa, so the "every operation is also a CLI subcommand" invariant is machine-checked.
9. **Table-driven validator tests with hostile inputs.** Mechanism: literal `[(input, expected)]` arrays including injection-shaped strings, as in `untrusted_peer_id_validation` (extras/rustdesk/src/common.rs); apply to quinjet's ref-name, remote, and pathspec validation.
10. **Skip environment-dependent tests by name in CI, not by deleting them.** Mechanism: `cargo test --workspace --no-fail-fast -- --skip <name>` per target, as in extras/rustdesk/.github/workflows/ci.yml; for quinjet, tests that need a real TTY can stay runnable locally while CI skips them explicitly and visibly.
11. **Make the release binary small and crash-clean.** Mechanism: `[profile.release] lto = true`, `codegen-units = 1`, `strip = true`, `panic = 'abort'` (extras/rustdesk/Cargo.toml); for a TUI, pair `panic = 'abort'` with a panic hook that restores the terminal first, the moral equivalent of `register_breakdown_handler` in extras/rustdesk/src/main.rs.
12. **Single-source the version and bump it mechanically.** Mechanism: a `bump.sh`-style script or an `xtask` that rewrites every file embedding the version and refreshes `Cargo.lock` in the same commit (extras/rustdesk/res/bump.sh); quinjet embeds its version in Cargo.toml, docs, and the wiki generator, so drift is possible today.
13. **Adopt structured YAML issue forms with required fields and disabled blank issues.** Mechanism: `.github/ISSUE_TEMPLATE/bug_report.yaml` with `validations: required: true` per field plus `config.yml` `blank_issues_enabled: false` and Discussion links, copied from extras/rustdesk/.github/ISSUE_TEMPLATE.
14. **Gate additive risk behind its own feature.** Mechanism: when a feature both reads and writes where its parent only reads, give the write path a dedicated cargo feature depending on the parent, as `drm-wake = ["drm"]` does in extras/rustdesk/Cargo.toml; for quinjet, destructive Git operations behind an opt-in feature or config gate follow the same principle.
15. **Keep runnable examples as manual harnesses for what tests cannot cover.** Mechanism: `examples/*.rs` driven by dev-dependencies (extras/rustdesk/libs/scrap/examples/benchmark.rs), which for quinjet means an example that drives the TUI against a throwaway repository for eyeball and performance checks outside the test harness.
