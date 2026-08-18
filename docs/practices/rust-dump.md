# Rust Dump

The whole practices reference bound into one file, for reading straight
through or searching in one place. Every part is also its own page:
start from [Rust Practices](./README.md) for the navigable version.

## Contents

- [Rust Practices](#rust-practices)
- [Studies](#studies)
- [rustdesk/rustdesk (120919 stars)](#rustdeskrustdesk-120919-stars)
- [tauri-apps/tauri (110245 stars)](#tauri-appstauri-110245-stars)
- [denoland/deno (108251 stars)](#denolanddeno-108251-stars)
- [astral-sh/uv (88771 stars)](#astral-shuv-88771-stars)
- [zed-industries/zed (88670 stars)](#zed-industrieszed-88670-stars)
- [BurntSushi/ripgrep (67319 stars)](#burntsushiripgrep-67319-stars)
- [alacritty/alacritty (65390 stars)](#alacrittyalacritty-65390-stars)
- [sharkdp/bat (60188 stars)](#sharkdpbat-60188-stars)
- [starship/starship (59420 stars)](#starshipstarship-59420-stars)
- [meilisearch/meilisearch (58979 stars)](#meilisearchmeilisearch-58979-stars)
- [astral-sh/ruff (49222 stars)](#astral-shruff-49222-stars)
- [bevyengine/bevy (47648 stars)](#bevyenginebevy-47648-stars)
- [helix-editor/helix (45833 stars)](#helix-editorhelix-45833-stars)
- [sharkdp/fd (44095 stars)](#sharkdpfd-44095-stars)
- [nushell/nushell (40272 stars)](#nushellnushell-40272-stars)
- [tokio-rs/tokio (32930 stars)](#tokio-rstokio-32930-stars)
- [gitui-org/gitui (22396 stars)](#gitui-orggitui-22396-stars)
- [clap-rs/clap (16634 stars)](#clap-rsclap-16634-stars)
- [Patterns](#patterns)
- [Formatting and Style Across the Rust Ecosystem](#formatting-and-style-across-the-rust-ecosystem)
- [Lints and Static Analysis](#lints-and-static-analysis)
- [CI/CD Patterns](#cicd-patterns)
- [Project and Workspace Structure](#project-and-workspace-structure)
- [Testing Strategies](#testing-strategies)
- [Error Handling and API Design](#error-handling-and-api-design)
- [Deep Rust Language Idioms](#deep-rust-language-idioms)
- [Dependencies, Releases, and Distribution](#dependencies-releases-and-distribution)
- [Documentation Practices](#documentation-practices)
- [Quinjet Gap Analysis](#quinjet-gap-analysis)

---

## Rust Practices

How the most widely used Rust codebases are engineered, distilled from a direct
study of eighteen repositories chosen by GitHub star count, and what that
corpus implies for Quinjet itself.

Each repository was cloned and read directly: manifests, formatting and lint
configuration, CI pipelines, test suites, source idioms, documentation, and
release machinery. Paths cited as `extras/<repo>/<file>` refer to those local
clones, which are ignored by Git; the same file exists at the same path in the
upstream repository.

### Contents

- [Studies](./studies/README.md): one chapter per repository, eighteen in all.
- [Patterns](./patterns/README.md): nine cross-cutting syntheses of what the
  corpus agrees on, where it splits, and why.

- [Gap Analysis](./gap-analysis.md): Quinjet audited against everything the
  study found, with completed recommendations and remaining gaps tracked.

- [Rust Dump](./rust-dump.md): the whole reference bound into one file for
  reading straight through or searching in one place.

### The corpus

| Repository | Stars | Study |
|---|---|---|
| [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | 120,919 | [rustdesk](./studies/rustdesk.md) |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | 110,245 | [tauri](./studies/tauri.md) |
| [denoland/deno](https://github.com/denoland/deno) | 108,251 | [deno](./studies/deno.md) |
| [astral-sh/uv](https://github.com/astral-sh/uv) | 88,771 | [uv](./studies/uv.md) |
| [zed-industries/zed](https://github.com/zed-industries/zed) | 88,670 | [zed](./studies/zed.md) |
| [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | 67,319 | [ripgrep](./studies/ripgrep.md) |
| [alacritty/alacritty](https://github.com/alacritty/alacritty) | 65,390 | [alacritty](./studies/alacritty.md) |
| [sharkdp/bat](https://github.com/sharkdp/bat) | 60,188 | [bat](./studies/bat.md) |
| [starship/starship](https://github.com/starship/starship) | 59,420 | [starship](./studies/starship.md) |
| [meilisearch/meilisearch](https://github.com/meilisearch/meilisearch) | 58,979 | [meilisearch](./studies/meilisearch.md) |
| [astral-sh/ruff](https://github.com/astral-sh/ruff) | 49,222 | [ruff](./studies/ruff.md) |
| [bevyengine/bevy](https://github.com/bevyengine/bevy) | 47,648 | [bevy](./studies/bevy.md) |
| [helix-editor/helix](https://github.com/helix-editor/helix) | 45,833 | [helix](./studies/helix.md) |
| [sharkdp/fd](https://github.com/sharkdp/fd) | 44,095 | [fd](./studies/fd.md) |
| [nushell/nushell](https://github.com/nushell/nushell) | 40,272 | [nushell](./studies/nushell.md) |
| [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | 32,930 | [tokio](./studies/tokio.md) |
| [gitui-org/gitui](https://github.com/gitui-org/gitui) | 22,396 | [gitui](./studies/gitui.md) |
| [clap-rs/clap](https://github.com/clap-rs/clap) | 16,634 | [clap](./studies/clap.md) |

Star counts were recorded in August 2026.

### How to read this

Start with the [patterns](./patterns/README.md) if you want the conclusions:
each one ends with a checklist a new Rust project can apply directly. Reach
into a [study](./studies/README.md) when you want the full context behind a
citation, and read the [gap analysis](./gap-analysis.md) to see the corpus
turned into a concrete status and prioritized plan for this repository.

---

## Studies

One chapter per repository, ordered by GitHub star count. Every chapter follows
the same shape: what the project is and how big it really is, repository
layout, manifest practices, formatting, linting, CI, testing, error handling,
deep language idioms with cited examples, documentation, release machinery, and
a closing list of lessons for Quinjet.

### Chapters

- [rustdesk](./studies/rustdesk.md): a remote desktop application, and the corpus's
  largest cross-platform release matrix.

- [tauri](./studies/tauri.md): a desktop and mobile app framework, and the reference
  for workspace-wide manifest inheritance and covenant-level CI.

- [deno](./studies/deno.md): a JavaScript and TypeScript runtime whose CI is generated
  from a typed script.

- [uv](./studies/uv.md): a Python package manager, and the strongest example of
  snapshot-first CLI testing.

- [zed](./studies/zed.md): a collaborative code editor with the largest workspace in
  the corpus.

- [ripgrep](./studies/ripgrep.md): a regex search tool, hand-rolled release
  engineering, and a masterclass in performance-first crate layering.

- [alacritty](./studies/alacritty.md): a GPU terminal emulator with a deliberately
  small dependency and CI footprint.

- [bat](./studies/bat.md): a cat clone whose CICD workflow shows MSRV discipline and
  binary packaging done thoroughly.

- [starship](./studies/starship.md): a shell prompt with fully automated
  conventional-commit releases.

- [meilisearch](./studies/meilisearch.md): a search engine, merge-queue CI, and
  declarative task orchestration.

- [ruff](./studies/ruff.md): a Python linter and formatter, and the corpus's most
  disciplined clippy configuration.

- [bevy](./studies/bevy.md): a game engine whose lint suppression discipline and
  example-driven docs stand out.

- [helix](./studies/helix.md): a modal editor with a clean three-crate seam between
  core, view, and terminal.

- [fd](./studies/fd.md): a find alternative, small enough to read whole, complete
  enough to copy from.

- [nushell](./studies/nushell.md): a structured-data shell, and the reference for
  panic hooks and terminal restoration.

- [tokio](./studies/tokio.md): the async runtime, and the reference for loom, miri,
  and public API discipline.

- [gitui](./studies/gitui.md): a terminal Git client, the closest neighbor to Quinjet
  in the corpus.

- [clap](./studies/clap.md): the argument parser Quinjet already uses, studied at the
  source.

---

## rustdesk/rustdesk (120919 stars)

### 1. What the project is and how big it is

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

The repository uses default rustfmt for the application; the only rustfmt configuration on disk is one file, extras/rustdesk/libs/enigo/rustfmt.toml, whose entire content is:

```toml
wrap_comments = true
```

That single setting makes rustfmt rewrap the long `//!` prose documentation in that crate to the line limit; the rest of the codebase accepts rustfmt defaults (4-space indent, 100-column max_width, edition-aware imports). There is no `.editorconfig`. Line-ending policy is delegated to git via extras/rustdesk/.gitattributes:

```text
* text=auto
```

A `cargo fmt -- --check` CI job exists in extras/rustdesk/.github/workflows/ci.yml but is commented out (lines 28-39), so formatting is enforced socially rather than mechanically. Non-Rust formatting: the Flutter tree relies on the Dart analyzer/formatter configured by extras/rustdesk/flutter/analysis_options.yaml, and CI installs the `rustfmt` component wherever `flutter_rust_bridge_codegen` runs, because the bridge generator formats its emitted Rust (extras/rustdesk/.github/workflows/bridge.yml passes `components: "rustfmt"`).

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage

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

### 10. Documentation practices

- **Massively translated project docs.** extras/rustdesk/docs holds README in about 30 languages, CONTRIBUTING in 14, SECURITY and CODE_OF_CONDUCT in a dozen each, all as parallel `-XX.md` files. The root README.md links to them.
- **CONTRIBUTING encodes process, not style.** extras/rustdesk/docs/CONTRIBUTING.md requires claiming an issue before working on it, small independently-correct commits, tests relevant to the change, and a Developer Certificate of Origin sign-off (`git commit -s`) binding contributions to the license.
- **Issue intake is a structured YAML form.** extras/rustdesk/.github/ISSUE_TEMPLATE/bug_report.yaml makes description, reproduction, expected behavior, both-side OS versions, both-side RustDesk versions, and screenshots all `required: true`, which matters enormously for a two-endpoint product. extras/rustdesk/.github/ISSUE_TEMPLATE/config.yml sets `blank_issues_enabled: false` and routes feature requests and questions to GitHub Discussions.
- **Rustdoc where a crate is a library.** `libs/enigo` opens with long-form `//!` module documentation including `no_run` doctest examples (extras/rustdesk/libs/enigo/src/lib.rs); application modules instead favor targeted doc comments on contracts, like the `core_main` return-value semantics (extras/rustdesk/src/core_main.rs) and functional directives such as `/// cbindgen:ignore` in extras/rustdesk/src/lib.rs.
- **Comments explain constraints with receipts.** Workflow env pins, Cargo dependency choices, and feature definitions consistently carry links to the issue, discussion, or upstream blog post that forced the decision (for example the cpal-on-Linux exclusion in extras/rustdesk/Cargo.toml citing discussion 10197). There is no in-repo mdBook or docs site; user documentation lives outside the repository.

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## tauri-apps/tauri (110245 stars)

### 1. What the project is and what the clone measures

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage

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

### 10. Documentation practices

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

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## denoland/deno (108251 stars)

### 1. What the project is and what the clone measures

Deno is a JavaScript, TypeScript, and WebAssembly runtime built on V8, Rust,
and Tokio, shipped as a single `deno` executable that bundles a package
manager, formatter, linter, test runner, LSP, bundler, and compiler. The
README states the pitch directly (extras/deno/README.md):

```text
[Deno](https://deno.com) ... is a JavaScript, TypeScript, and WebAssembly
runtime with secure defaults and a great developer experience. It's built on
[V8](https://v8.dev/), [Rust](https://www.rust-lang.org/), and
[Tokio](https://tokio.rs/).
```

Industry uses it because it is a security-first Node alternative with
permissions on by default, first-class TypeScript, and a batteries-included
toolchain, all delivered as one static binary.

Scale, measured on this clone:

- 1045 `.rs` files totaling about 670,000 lines of Rust, about 600,000 of
  which are outside `tests/` (counted with `wc -l` over the tree).
- 87 `Cargo.toml` manifests; the root workspace declares 77 explicit members
  in extras/deno/Cargo.toml, spanning `cli/`, 27 `ext/*` extension crates,
  28 `libs/*` support crates, `runtime/`, and the test crates under `tests/`.
- 2087 `__test__.jsonc` spec-test manifests under extras/deno/tests/specs.
- The CLI crate is at version 2.9.5 (extras/deno/cli/Cargo.toml) and the
  changelog extras/deno/Releases.md is 659 KB of release history.
- The generated main CI workflow alone is 7676 lines
  (extras/deno/.github/workflows/ci.generated.yml).

### 2. Repository layout

Top level of the clone (directories only, ASCII tree):

```text
extras/deno/
|-- cli/          the `deno` crate: flags, subcommands, tools, LSP, snapshot
|   |-- lib/      deno_lib shared library
|   |-- rt/       denort, the slim runtime used by `deno compile` binaries
|   |-- rt_desktop/  desktop variant of denort
|   `-- snapshot/ build-time V8 snapshot crate
|-- runtime/      deno_runtime: assembles extensions into a JS runtime
|   |-- permissions/  deno_permissions crate
|   |-- features/     feature-flag crate
|   `-- subprocess_windows/  Windows-only process handling crate
|-- ext/          one crate per capability exposed to JS (fs, net, web, ...)
|-- libs/         deno_core, serde_v8, ops macros, resolver, npm, config, ...
|-- tests/        all integration-level testing (specs, unit, wpt, bench, util)
|-- tools/        repo automation: format.js, lint.js, release/, x.ts
|-- doc/          contributor-facing architecture and process docs
`-- .github/      workflows (generated), templates, mtime_cache action
```

The split is layered and the layering is documented as a hard rule in
extras/deno/doc/architecture.md:

```text
+-----------------------------------------------------------+
|  cli/            the `deno` binary: subcommands, tooling   |
+-----------------------------------------------------------+
|  runtime/        deno_runtime: assembles the JS runtime    |
+-----------------------------------------------------------+
|  ext/*           extensions: native capabilities for JS    |
+-----------------------------------------------------------+
|  libs/*          deno_core + supporting crates (V8 bridge) |
+-----------------------------------------------------------+
|  V8 + Tokio      JavaScript engine and async runtime       |
+-----------------------------------------------------------+
```

Each layer depends only on layers below it, which keeps lower layers
publishable and reusable (`deno_runtime`, `deno_core`, and every `ext/*`
crate are published to crates.io with independent versions, see the
`[workspace.dependencies]` path entries in extras/deno/Cargo.toml). The
layout is actively defended: `ensureNoNewTopLevelEntries()` in
extras/deno/tools/lint.js fails CI if a new top-level file or directory
appears outside an allowlist, with the comment
`// WARNING: When adding anything to this list it must be discussed!`.

There is also a one-letter developer entry point, extras/deno/x:

```typescript
#!/usr/bin/env -S deno run --allow-all --ext=ts

import "./tools/x.ts";
```

so `./x fmt`, `./x lint`, `./x spec` are the canonical commands
(documented in extras/deno/.github/CONTRIBUTING.md). A Nix flake
(extras/deno/flake.nix) provides a reproducible dev shell and build.

### 3. Cargo manifest practices

The root extras/deno/Cargo.toml is the single source of truth for shared
metadata and versions:

- `[workspace.package]` carries `authors`, `edition = "2024"`,
  `license = "MIT"`, and `repository`; member crates inherit with
  `edition.workspace = true` and friends (see extras/deno/cli/Cargo.toml).
- `[workspace.dependencies]` holds every third-party version, grouped by
  concern with section comments: `# exts`, `# workspace libraries`,
  `# widely used libraries`, `# cli`, `# crypto`, `# ffi`, `# napi`,
  `# macros`, `# unix deps`, `# windows deps`.
- Risky dependencies are pinned exactly (`base32 = "=0.5.1"`,
  `rand = "=0.8.5"`, `rustls = { version = "=0.23.40", ... }`) and every
  pin carries a reason as a comment, for example:

```toml
pin-project = "1.0.11" # don't pin because they yank crates from cargo
reqwest = { version = "=0.12.5", ... } # pinned because of https://github.com/seanmonstar/reqwest/pull/1955
```

- Footgun avoidance is written into the dependency table itself:

```toml
# Note: Do not use the "clock" feature of chrono, as it links us to CoreFoundation on macOS.
#       Instead use util::time::utc_now()
chrono = { version = "0.4", default-features = false, features = ["std", "serde"] }
```

- Feature flags in extras/deno/cli/Cargo.toml model real product variants:
  `default = ["v8"]`, an experimental `quickjs` engine backend, `hmr` for
  snapshot-free development, `dhat-heap` for heap profiling, `upgrade`
  (disabled by Linux distro packagers), and private helper features spelled
  with a double underscore (`__runtime_defaults`, `__vendored_zlib_ng`) to
  signal they are not public API.
- `[lints]` tables are used sparingly and precisely: extras/deno/runtime/Cargo.toml
  and extras/deno/ext/ffi/Cargo.toml declare
  `unexpected_cfgs = { level = "warn", check-cfg = ['cfg(tokio_unstable)'] }`
  so custom `--cfg` flags stay checked.
- Profiles are tuned aggressively. `release` uses `codegen-units = 1`,
  `lto = true`, `opt-level = 'z'`, `panic = "abort"`,
  `split-debuginfo = "packed"`, `debug = "line-tables-only"`, and the file
  warns twice, "NB: the bench and release profiles must remain EXACTLY
  the same." About 50 `[profile.release.package.*]` overrides re-enable
  `opt-level = 3` for hot crates (tokio, hyper, v8, serde, zstd, ...), so
  the binary is small overall but fast where it matters. A custom
  `release-lite` profile (incremental, `codegen-units = 128`,
  `lto = "thin"`) exists purely for local iteration, and dev-profile
  overrides fix pathological cases (`[profile.dev.package.v8]`
  `opt-level = 1` with the comment `# rusty-v8 needs at least -O1 to not
  miscompile`).
- There is no `rust-version` key; the toolchain is pinned instead in
  extras/deno/rust-toolchain.toml:

```toml
[toolchain]
channel = "1.95.0"
components = ["rustfmt", "clippy", "rust-src", "rust-analyzer"]
```

- extras/deno/.cargo/config.toml carries per-target `rustflags` (static CRT
  on Windows, a 4 MB main-thread stack reserve via `link-arg=/STACK:4194304`,
  linker ICF and chained fixups on macOS) with long comments explaining the
  measured effect of each flag.
- extras/deno/.cargo/local-build.toml is a checked-in `[patch.crates-io]`
  overlay for developing against sibling checkouts of `deno_core` and
  `rusty_v8`, activated with `cargo --config .cargo/local-build.toml build`.
- `Cargo.lock` is committed and CI test runs use `--locked`.

### 4. Formatting

extras/deno/.rustfmt.toml is three lines:

```toml
max_width = 80
tab_spaces = 2
edition = "2024"
```

80 columns and 2-space indent match the JS/TS style so the polyglot codebase
reads uniformly. The interesting part is that rustfmt is not invoked
directly: dprint orchestrates every formatter through
extras/deno/.dprint.json, including Rust via the exec plugin:

```json
"exec": {
  "cwd": "${configDir}",
  "commands": [{
    "command": "rustfmt --config imports_granularity=item --config group_imports=StdExternalCrate",
    "exts": ["rs"],
    "cacheKeyFiles": ["rust-toolchain.toml", ".rustfmt.toml"]
  }]
}
```

Two unstable rustfmt options are injected on the command line: one import
per `use` item and std/external/crate import grouping, which makes import
diffs trivially mergeable. The same file configures dprint plugins for
TypeScript, JSON, Markdown, TOML, and YAML, plus a large `excludes` list of
vendored and fixture files. `tools/format.js` runs it all;
`deno run ... ./tools/format.js --check` is a CI lint step.

extras/deno/.editorconfig sets `lf`, final newline, 2-space indent, UTF-8,
and trimmed whitespace globally, then carves out test fixtures where
byte-exactness matters:

```toml
[*.out] # make editor neutral to .out files
insert_final_newline = unset
trim_trailing_whitespace = unset
```

Line endings are also normalized at the git layer
(extras/deno/.gitattributes: `* text=auto eol=lf`, with one file forced to
`eol=crlf` for a Windows-specific test).

### 5. Linting

Clippy configuration lives in three cooperating places.

First, workspace-wide hard denials in extras/deno/.cargo/config.toml, which
apply to every local `cargo build`, not only CI:

```toml
[target.'cfg(all())']
rustflags = [
  "-D", "clippy::all",
  "-D", "clippy::await_holding_refcell_ref",
  "-D", "clippy::missing_safety_doc",
  "-D", "clippy::undocumented_unsafe_blocks",
  "--cfg", "tokio_unstable",
]
```

Second, the CI lint driver extras/deno/tools/lint.js appends more denials
when it invokes clippy:

```toml
"--deny", "clippy::unused_async",
"--deny", "clippy::print_stderr",
"--deny", "clippy::print_stdout",
"--deny", "clippy::large_futures",
"--deny", "clippy::allow_attributes_without_reason",
```

`print_stdout`/`print_stderr` are banned because the std print macros panic
on `EPIPE` (see section 8), `large_futures` protects the single-threaded
runtime's stack, and `allow_attributes_without_reason` forces every escape
hatch to justify itself, e.g. in extras/deno/cli/lib.rs:
`#[allow(clippy::disallowed_types, reason = "definition")]`. The driver even
post-processes clippy output: when a print-macro violation appears it prints
a hint pointing at `drop_println!` replacements.

Third, per-crate `clippy.toml` files (61 of them) use `disallowed-methods`
and `disallowed-types` as an architectural boundary enforcement tool.
extras/deno/cli/clippy.toml bans ambient process state:

```toml
{ path = "std::process::exit", reason = "use deno_runtime::exit instead" },
{ path = "std::env::current_dir", reason = "use crate::util::env::resolve_cwd instead and prefer passing it the initial_cwd" },
{ path = "reqwest::Client::new", reason = "create an HttpClient via an HttpClientProvider instead" },
```

extras/deno/libs/core/clippy.toml bans nearly all of `std::fs`, `std::env`,
`std::time::SystemTime::now`, and `chrono::Utc::now` in favor of the
`sys_traits` capability traits, so lower layers stay testable and Wasm-safe.
The philosophy: clippy is not just style, it is a mechanized code review
that encodes project-specific invariants, and every rule states its
replacement in the `reason` field.

Custom lint infrastructure goes further. extras/deno/tools/lint.js also
runs: `deno lint` over JS/TS with custom plugin rules
(tools/lint_plugins/prefer_primordials.ts for runtime bootstrap code,
no_deno_api_in_polyfills.ts with per-file expected violation counts), a
copyright header check (tools/copyright_checker.js), a check that every
generated workflow YAML matches its generator
(`ensureWorkflowYmlsUpToDate()`), a check that no `.out` fixture is
unreferenced (`ensureNoUnusedOutFiles()`), a top-level directory allowlist,
a rule that every `ext/` and `libs/` crate has a `clippy.toml` with the
required disallowed methods (`ensureDisallowedMethodsEnforced()`), and a
CLI-design rule that uppercase short flags are reserved for permissions
(`ensureNoNonPermissionCapitalLetterShortFlags()`), asserted against the
actual parser tables in libs/cli_parser/src/defs.rs.

### 6. CI/CD

Every workflow under extras/deno/.github/workflows exists twice: a
TypeScript generator (`ci.ts`, `pr.ts`, `npm_publish.ts`, ...) and its
output (`*.generated.yml`), each headed
`# GENERATED BY ./ci.ts -- DO NOT DIRECTLY EDIT`. The generators use a
typed workflow-builder library (`jsr:@david/gagen` in
extras/deno/.github/workflows/ci.ts), so 2260 lines of TypeScript with
loops, shared step factories, and typed conditions produce the 7676-line
`ci.generated.yml`. Drift is impossible because `tools/lint.js` re-runs
every generator with `--lint` and fails if the YAML is stale.

Action pinning is automatic: the generator resolves tags to commit SHAs and
records the mapping as trailing comments, e.g. in ci.generated.yml:

```yaml
# gagen:pin actions/checkout@v6 = de0fac2e4500dabe0009e67214ff5f5447ce83dd
# gagen:pin dtolnay/rust-toolchain@master = 3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9
```

The main `ci` workflow (push to main/tags, pull_request) contains 39 jobs:

- `pre_build` gates everything: draft PRs skip CI unless the commit message
  contains `[ci]` or the PR has the `ci-draft` label; scripts
  `tools/check_deno_core_changes.js` and `tools/check_docs_only_changes.js`
  publish `skip_deno_core_test` and `docs_only` outputs that downstream
  `if:` conditions consume.
- Build and test are split per platform with artifact handoff: six targets
  (linux/macos/windows, each x86_64 and aarch64) times debug and release,
  with `build-*` jobs uploading `deno`, `denort`, and `test-server`
  artifacts that sharded `test-*` jobs download. Runner labels are chosen
  conditionally: XL runners only for `denoland/deno` on main, tags, or the
  `ci-full` label (extras/deno/.github/workflows/ci.ts, `Runners`).
- Test jobs are sharded per test crate on PRs (`CI_SHARD_INDEX` /
  `CI_SHARD_TOTAL` env vars), and the shard list is computed by parsing the
  workspace `Cargo.toml` at generation time (`resolveTestCrateTests()`).
- `wpt-release-linux-x86_64` runs the Web Platform Tests plus the Autobahn
  websocket fuzzing suite and uploads results to wpt.fyi.
- `deno-core-test` and `deno-core-miri` run the engine crates separately;
  Miri runs on a pinned nightly (`nightly-2025-11-12`) with
  `cargo miri test -p deno_core --features v8`.
- `lint` runs on a 3-OS matrix (linux, macos, windows) so
  platform-conditional code is linted on every platform, with format and
  jsdoc checks only on linux.
- `ci-status` is the single aggregator job branch protection keys on:

```yaml
- name: Ensure CI success
  run: |-
    if [[ "${{ contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') }}" == "true" ]]; then
      echo 'CI failed'
      exit 1
    fi
```

- `publish-canary` uploads `canary-latest.txt` to `dl.deno.land` on every
  main push, so a canary build exists for every commit.

Caching is deliberate: a manual `cacheVersion = 123` constant busts all
caches at once; cargo-home and target caches are restored by prefix
(`key: never_saved` plus `restore-keys`) but saved only on main with
`key: <prefix>-${{ github.sha }}` so PRs always restore the freshest main
cache, and a custom local action extras/deno/.github/mtime_cache restores
file mtimes so cargo's change detection works across cache restores. Every
job sets `timeout-minutes`, and the workflow-level concurrency group
cancels superseded runs except when the `ci-test-flaky` label asks runs to
be kept. There is even a step-level workaround pinned in every job:
`Pre-install rustup 1.28.2 (workaround broken 1.29.0)`.

Other workflows: `pr` lints PR titles against conventional-commit rules
(`tools/verify_pr_title.js`), `node_compat_test` runs Node's own suite
daily on a 3-shard linux/windows/macos matrix (cron `0 10 * * *`),
`ecosystem_compat_test` runs weekday crons against real-world frameworks,
and five release workflows are described in section 11. There is no merge
queue configuration in the repo; the `ci-status` job is the required check.

### 7. Testing

The test tree is a top-level concern, not a per-crate afterthought.
extras/deno/doc/testing.md maps the suites: spec tests (`tests/specs/`, the
main CLI end-to-end suite), JS unit tests (`tests/unit/`), Node compat
layers (`tests/unit_node/`, `tests/node_compat/` running Node's own suite
from a submodule), Web Platform Tests (`tests/wpt/`, submodule), and Rust
unit tests inline in each crate.

Spec tests are the standout design. Each of the 2087 directories under
extras/deno/tests/specs holds a `__test__.jsonc` manifest (validated by
`#[serde(deny_unknown_fields)]` structs in extras/deno/tests/specs/mod.rs
and a JSON schema at tests/specs/schema.json) describing steps: a CLI
invocation, an expected output (inline or a `.out` file), env vars, and a
`temp_dir` flag. Expected output uses a purpose-built matching language
implemented in extras/deno/tests/util/lib/wildcard.rs: `[WILDCARD]`,
`[WILDLINE]`, `[WILDCHARS(n)]`, `[UNORDERED_START]`/`[UNORDERED_END]` for
nondeterministic ordering, and `[# comment]` lines. The runner is a custom
harness (`harness = false` in extras/deno/tests/specs/Cargo.toml) built on
the `file_test_runner` crate, with a `FlakyTestTracker`
(tests/util/lib/test_runner.rs) that records and retries flaky tests, and
PR-only sharding driven by `CI_SHARD_INDEX`.

The helper crate extras/deno/tests/util/lib is a full testing SDK:
`PathRef` (a newtype over `PathBuf` with assertion helpers),
`TestContextBuilder` (a builder that provisions temp dirs, env vars, and
local registries: `TestContextBuilder::for_npm()` spins up the mock npm
registry), a PTY driver (tests/util/lib/pty.rs) for testing the REPL and
interactive prompts against a real terminal, and wildcard assertion
functions. extras/deno/tests/util/server hosts an entire fake internet:
mock npm and JSR registries, a Node.js download mirror, gRPC, websocket,
and TLS servers (tests/util/server/servers/).

Beyond example-based testing:

- Property testing: extras/deno/runtime/permissions/lib.rs has a `proptests`
  module using `proptest` to verify algebraic laws of the permission
  system, e.g. `net_descriptor_ord_transitivity` proving the manual `Ord`
  is transitive, and `net_query_never_granted_when_flag_denied` proving a
  security invariant over arbitrary allow/deny sets.
- Fuzzing: extras/deno/libs/npm/fuzz is a `cargo-fuzz` crate
  (`libfuzzer-sys`, target `fuzz_targets/packument_index.rs`) for npm
  registry metadata parsing.
- Miri: the `deno-core-miri` CI job (section 6) runs the engine under Miri.
- Benchmarks: extras/deno/tests/bench is a dedicated crate with
  `harness = false` benches (`deno_bench`, `lsp_bench_standalone`) that
  measure the real built binary (startup, HTTP throughput via installed
  `wrk`/`hyperfine`, LSP timings) and publish results to a `gh-pages`
  benchmark site from the `bench` CI job.

### 8. Error handling and API design

Error types are structured enums via `thiserror` 2, with an extra derive,
`deno_error::JsError`, that maps each Rust variant to the JavaScript error
class users see (extras/deno/ext/fs/ops.rs):

```rust
#[derive(Debug, Boxed, deno_error::JsError)]
pub struct FsOpsError(pub Box<FsOpsErrorKind>);

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum FsOpsErrorKind {
  #[class(inherit)]
  #[error("{0}")]
  Io(#[source] std::io::Error),
  ...
  #[class("InvalidData")]
  #[error("File name or path {0:?} is not valid UTF-8")]
  InvalidUtf8(std::ffi::OsString),
  #[class(type)]
  #[error("Invalid seek mode: {0}")]
  InvalidSeekMode(i32),
```

The `Boxed` derive (the `boxed_error` crate) wraps the large kind enum in a
one-pointer struct so `Result<T, FsOpsError>` stays cheap to return. At the
CLI boundary, `anyhow`-style `AnyError` (re-exported by deno_core) is used
for aggregation, and downcasting recovers structure: `exit_for_error` in
extras/deno/cli/lib.rs downcasts to JS errors for pretty formatting and
appends context-aware hints before exiting with code 1.

Result discipline is enforced by a small conversion trait
(extras/deno/cli/lib.rs):

```rust
/// Ensures that all subcommands return an i32 exit code and an [`AnyError`] error type.
trait SubcommandOutput {
  fn output(self) -> Result<i32, AnyError>;
}
```

so every subcommand funnels into one `Result<i32, AnyError>` shape, and the
process exits exactly once through `deno_runtime::exit(exit_code)`;
`std::process::exit` is clippy-banned (section 5). Panic policy: release
profiles use `panic = "abort"`, an optional `panic-trace` feature wires a
custom panic backtrace handler (extras/deno/cli/Cargo.toml), and the
EPIPE-panic of the std print macros is designed away by
extras/deno/libs/print/lib.rs:

```rust
//! Replacements for the std `print!`, `println!`, `eprint!` and `eprintln!`
//! macros that drop write errors instead of panicking, named after Cargo's
//! macros with the same semantics.
```

API-design patterns visible throughout: builders with chained `self`
methods (`TestContextBuilder` in extras/deno/tests/util/lib/builders.rs),
security newtypes with private fields
(`CheckedPath` in extras/deno/runtime/permissions/lib.rs, section 9),
visibility discipline like the `pub(crate) mod sys` alias that is the only
allowed spelling of the real filesystem in the CLI
(extras/deno/cli/lib.rs), and `#[doc(hidden)]` on internals re-exported
only for benchmarks.

### 9. Deep Rust usage: ten-plus cited idioms

1. Proc-macro ops with typed marshaling. The `#[op2]` attribute
   (extras/deno/libs/ops) turns a plain Rust function into a V8 binding;
   argument attributes choose the marshaling strategy and `fast` opts into
   V8 fast-calls (extras/deno/ext/web/lib.rs):

   ```rust
   #[op2(fast)]
   fn op_base64_decode_into(
     #[string(onebyte)] input: Cow<[u8]>,
     #[buffer] target: &mut [u8],
     #[smi] offset: u32,
   ) -> Result<i32, WebError> {
   ```

2. Declarative registration macros. `deno_core::extension!(deno_web, deps =
   [deno_webidl], ops = [op_base64_decode, ...])`
   (extras/deno/ext/web/lib.rs line 71) assembles ops, JS sources, and
   state into an `Extension` at compile time, keeping registration
   impossible to get out of sync with definitions.

3. Zero-copy `Cow` everywhere: 530 `Cow<` occurrences in `cli`, `libs`,
   and `ext`. The op above receives V8 one-byte strings as `Cow<[u8]>`
   without copying when the representation already matches, and the
   `Resource` trait returns `Cow<'_, str>` for names.

4. Newtypes with lifetime-encoded guarantees. Permission checking returns
   `CheckedPath<'a>` / `CheckedPathBuf` (extras/deno/runtime/permissions/lib.rs):

   ```rust
   pub struct CheckedPath<'a> {
     // these are private to prevent someone constructing this outside the crate
     path: PathWithRequested<'a>,
     canonicalized: bool,
   }
   ```

   Filesystem ops take `CheckedPath`, not `Path`, so an unchecked path is a
   type error; the borrowed/owned pair mirrors `Path`/`PathBuf` and the
   escape hatch is loudly named `unsafe_new`.

5. Compile-time sync/unsync polymorphism. extras/deno/libs/maybe_sync/lib.rs
   defines `MaybeSend`/`MaybeSync`/`MaybeArc`/`MaybeDashMap`: with the
   `sync` feature they alias `Send`/`Sync`/`Arc`/`DashMap`; without it they
   become no-op traits, `Rc`, and a `RefCell<HashMap>` wrapper. Shared
   crates are written once against the `Maybe*` names and each consumer
   picks thread-safety at feature-resolution time, paying zero cost.

6. Single-threaded async architecture. The whole runtime runs on a
   current-thread Tokio runtime (extras/deno/runtime/tokio_util.rs,
   `tokio::runtime::Builder::new_current_thread()`), so op state is
   `Rc<RefCell<OpState>>` (265 `Rc<RefCell<` occurrences under ext/) instead
   of `Arc<Mutex>`, and `clippy::await_holding_refcell_ref` is a hard deny
   to keep that safe. `spawn_subcommand` in extras/deno/cli/lib.rs pairs
   this with `.boxed_local()` and documents why: giant subcommand futures
   would otherwise blow the stack in debug builds.

7. Object-safe async resource trait. `pub trait Resource: Any + 'static`
   (extras/deno/libs/core/io/resource.rs) uses `self: Rc<Self>` receivers
   returning boxed futures, default methods that error with "not
   supported", and an opt-in `read_byob` fast path, giving JS a uniform
   handle table over files, sockets, and streams.

8. Static data over runtime construction. The CLI parser
   (extras/deno/libs/cli_parser/src/types.rs) models the entire command
   tree as `const` tables:

   ```rust
   pub struct CommandDef {
     pub name: &'static str,
     pub about: &'static str,
     pub aliases: &'static [&'static str],
     pub args: &'static [ArgDef],
     ...
   }
   ```

   Parsing, help rendering, and shell completions
   (libs/cli_parser/src/completions.rs) all walk the same static tables, so
   startup does no allocation to build a parser and help/completions can
   never disagree with parsing.

9. Property-based verification of unsafe-to-get-wrong logic. The
   permissions crate proves `Ord` transitivity and deny-precedence with
   `proptest` generators over descriptor types
   (extras/deno/runtime/permissions/lib.rs, `mod proptests`), an idiom
   worth copying wherever a manual `Ord`/matching implementation guards
   security decisions.

10. Audited unsafe. 1497 `SAFETY:` comments repo-wide, and the comments are
    not optional: `clippy::undocumented_unsafe_blocks` and
    `clippy::missing_safety_doc` are denied workspace-wide in
    extras/deno/.cargo/config.toml, so every `unsafe` block ships with its
    justification or the build fails.

11. Platform handling as structure, not scatter. Windows-only process code
    is a separate crate (extras/deno/runtime/subprocess_windows), Windows
    deps are scoped per crate with minimal feature lists
    (`[target.'cfg(windows)'.dependencies] windows-sys = { workspace =
    true, features = [...] }` in extras/deno/ext/ffi/Cargo.toml), and
    custom cfgs are registered via `check-cfg` lints so typos in `#[cfg]`
    warn (extras/deno/runtime/Cargo.toml).

12. Macro-by-example for policy, not cleverness. The `drop_println!`
    family (extras/deno/libs/print/lib.rs) is a five-line macro that
    encodes an OS-level policy (EPIPE tolerance), paired with the clippy
    denial that forces its use; the macro, the lint, and the CI hint form
    one closed loop.

### 10. Documentation practices

- In-repo contributor docs live in extras/deno/doc: `architecture.md` (the
  layer diagram and dependency rules), `ci.md` (how the generated CI works
  and its gating outputs), `codebase-map.md`, `testing.md` (which suite to
  reach for and when), `package-management.md`, and
  `desktop-architecture.md`. These are short, current, and referenced by
  CI itself (the docs-only fast path in section 6 keys on `doc/`).
- Rustdoc is used at API boundaries with real contracts: module docs like
  the `//!` header of extras/deno/libs/print/lib.rs explain the why
  (SIGPIPE semantics, links to the tracking issue), and trait docs in
  extras/deno/libs/core/io/resource.rs specify default-method behavior and
  when to override (`read_byob`).
- Every source file opens with a license header,
  `// Copyright 2018-2026 the Deno authors. MIT license.`, enforced by
  extras/deno/tools/copyright_checker.js in CI.
- extras/deno/.github/CONTRIBUTING.md documents prerequisites, the `./x`
  tool, and HMR-based iteration; extras/deno/.github/PULL_REQUEST_TEMPLATE.md
  demands conventional-commit titles with good/bad examples
  (`fix(ext/net): fix race condition in TCP listener` vs `fix #7123`), and
  the `pr` workflow lints the title mechanically.
- Issue templates (extras/deno/.github/ISSUE_TEMPLATE: `bug_report.md`,
  `feature_request.md`, `config.yml`) route support questions elsewhere.
- Many crates carry their own `README.md` (e.g. extras/deno/ext/url/README.md,
  extras/deno/libs/core/README.md) because they publish independently.

### 11. Release and distribution

Versioning: the CLI is semver (`2.9.5`), and every publishable workspace
crate carries its own version in the root `[workspace.dependencies]` table
(extras/deno/Cargo.toml), bumped together by tooling. The changelog,
extras/deno/Releases.md, is one bullet per merged PR in conventional-commit
form with PR numbers, per release:

```markdown
### 2.9.5 / 2026.08.06

- feat(add): `--unscoped` flag to alias packages by their unscoped name (#36319)
- feat(task): add --members flag to run tasks in workspace members only (#35748)
```

The release pipeline is a chain of `workflow_dispatch` workflows backed by
numbered scripts in extras/deno/tools/release
(`00_start_release.ts` through `05_create_release_notes.ts`, with the
runbook in extras/deno/tools/cut_a_release.md):

- `start_release` takes patch/minor/major input and generates a gist of
  step-by-step instructions for the release captain.
- `version_bump` bumps all crate versions (also patching the CI
  `cacheVersion` by regex) and opens a PR as a bot.
- `cargo_publish` publishes the crates and tags.
- `promote_to_release` promotes a canary build to `rc` or re-stamps a
  stable build as `lts` using a `patchver` binary-patching tool, then code
  signs `deno.exe` with Azure Trusted Signing and verifies with `signtool`
  (extras/deno/.github/workflows/promote_to_release.generated.yml).
- `npm_publish` (triggered by `release: published`, with a
  `dry_run: default: true` input) wraps the binaries into npm packages.

Distribution is multi-channel: GitHub Releases archives, `dl.deno.land`
(with a canary for every main commit via the `publish-canary` job), npm,
and an in-binary `deno upgrade` subcommand that distro packagers can
compile out by disabling the `upgrade` feature
(extras/deno/cli/Cargo.toml: `# This is typically disabled for (Linux)
distribution packages.`). Shell completions are first-class: static
generation plus dynamic completion where the shell calls back into the
binary with `COMPLETE=<shell>` set, including live `deno task` name
completion (extras/deno/cli/args/flags.rs, `handle_shell_completion` and
`handle_dynamic_shell_completion`).

### 12. Lessons for quinjet

quinjet already has a strict clippy wall, rustfmt, cargo-deny, taplo,
typos, coverage, miri, and mutants. What Deno adds on top of that:

1. Adopt `clippy.toml` `disallowed-methods`/`disallowed-types` as
   architecture enforcement, not just hygiene. quinjet has a clippy.toml
   already; add entries with `reason` strings the way
   extras/deno/cli/clippy.toml does: ban `std::process::exit` outside one
   exit function, ban direct `std::env::current_dir` in favor of a resolved
   repo root passed down, and ban raw `print!`/`println!` outside the CLI
   output layer.
2. Make stdout EPIPE-safe. A Git TUI whose subcommands get piped
   (`quinjet log | head`) will panic with the std macros. Copy the
   `drop_println!` pattern from extras/deno/libs/print/lib.rs (a 10-line
   macro_rules pair) and deny `clippy::print_stdout`/`print_stderr` in the
   Makefile lint target, exactly as extras/deno/tools/lint.js does.
3. Golden-file CLI spec tests. Every quinjet operation is a CLI subcommand,
   which is exactly the shape of extras/deno/tests/specs: a directory per
   scenario, a `__test__.jsonc` manifest, `.out` expectations with
   `[WILDCARD]`/`[WILDLINE]`/`[UNORDERED_START]` matching. Use the same
   `file_test_runner` crate with `harness = false` and port the wildcard
   matcher idea from extras/deno/tests/util/lib/wildcard.rs; add a lint
   that fails on unreferenced `.out` files (`ensureNoUnusedOutFiles()` in
   extras/deno/tools/lint.js).
4. PTY-based end-to-end tests for the TUI. extras/deno/tests/util/lib/pty.rs
   shows the harness: spawn the real binary in a pseudo-terminal, write
   keystrokes, `.expect(...)` on output between writes, strip ANSI codes
   before asserting. This is the missing end-to-end layer for a ratatui
   app; the `portable-pty` crate provides the primitive.
5. Property-test the security- and ordering-critical logic.
   extras/deno/runtime/permissions/lib.rs proves `Ord` transitivity and
   deny-precedence with `proptest`. quinjet equivalents: refspec/branch
   name parsing round-trips, keybinding chord resolution, and any manual
   `Ord`/`PartialOrd` impl.
6. Add a `cargo-fuzz` target for parser code, following
   extras/deno/libs/npm/fuzz/Cargo.toml (a tiny `publish = false` crate
   with `libfuzzer-sys` and one `[[bin]]` per target). Candidate targets:
   whatever quinjet parses from `git` output or config files.
7. Single aggregator CI job. Mirror `ci-status` from
   extras/deno/.github/workflows/ci.generated.yml: one job that `needs:`
   everything and fails on `contains(needs.*.result, 'failure') ||
   contains(needs.*.result, 'cancelled')`, and make it the only required
   branch-protection check, so adding or sharding jobs never requires
   touching repo settings.
8. Pin actions to commit SHAs with a tag comment
   (`# gagen:pin actions/checkout@v6 = de0fac2e...`), set
   `timeout-minutes` on every job, and add a `concurrency` group with
   `cancel-in-progress: true` keyed on the head ref.
9. Cache policy worth copying even single-crate: version-prefixed cache
   keys via one bump-able constant (`const cacheVersion = 123` in
   extras/deno/.github/workflows/ci.ts), restore-by-prefix
   (`key: never_saved` plus `restore-keys`), and save only on main with the
   SHA in the key so PRs always restore fresh.
10. Exit-code discipline via a conversion trait. Copy `SubcommandOutput`
    from extras/deno/cli/lib.rs: every subcommand returns
    `Result<i32, Error>` through one trait, and exactly one function ever
    exits the process. Pair it with the `boxed_error` crate plus a
    `thiserror` kind-enum when clippy's `result_large_err` starts firing.
11. Structured, drift-proof generated files. quinjet generates a wiki from
    docs; extend the pattern Deno uses in `ensureWorkflowYmlsUpToDate()`:
    every generated artifact (completions, man pages, wiki) gets a CI step
    that regenerates and fails on diff, and the file itself carries a
    `GENERATED BY <source> -- DO NOT DIRECTLY EDIT` header.
12. Dynamic shell completions from the binary itself. Deno registers a
    completion script that calls back into the binary with `COMPLETE` set
    and completes live data such as task names
    (extras/deno/cli/args/flags.rs). quinjet can complete branch names,
    remotes, and stash refs the same way with clap_complete's dynamic
    completion support, keeping completions in lockstep with the parser.
13. Release-profile tuning: quinjet ships a TUI binary, so copy the
    size/speed split from extras/deno/Cargo.toml: `opt-level = "z"`,
    `lto = true`, `codegen-units = 1`, `panic = "abort"`,
    `debug = "line-tables-only"` with `split-debuginfo = "packed"`, plus
    `[profile.release.package.<hot-crate>] opt-level = 3` for the hot path,
    and a `release-lite` profile (`inherits = "release"`, incremental,
    thin LTO) for local iteration.
14. A tiny PR-title lint workflow (extras/deno/.github/workflows/pr.generated.yml
    running one script against `github.event.pull_request.title`) keeps the
    conventional-commit history that quinjet's changelog and squash-merge
    flow depend on, at the cost of a 25-line workflow.
15. Draft-PR gating and docs-only fast paths: expose cheap `pre_build`
    outputs (`docs_only`, `skip_build`) and condition the expensive miri,
    mutants, and coverage jobs on them, as
    extras/deno/.github/workflows/ci.generated.yml does, so documentation
    changes do not pay the full CI bill.

---

## astral-sh/uv (88771 stars)

### 1. What the project is and how big it is

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage, cited

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

### 10. Documentation practices

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

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## zed-industries/zed (88670 stars)

### 1. What the project is and what it measures like

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage: cited idioms

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

### 10. Documentation practices

User documentation is an mdBook rooted at `extras/zed/docs/book.toml`, with a twist: the HTML renderer is wrapped by a first-party crate (`command = "cargo run -p docs_preprocessor -- postprocess"` under `[output.zed-html]`), because "post-processing is not possible with mdbook in the same way pre-processing is" (comment in the same file). Docs are built and link-checked in CI (`check_docs` job runs lychee twice, on sources and on rendered output, configured by `extras/zed/lychee.toml` with retry and status-code policy), and deployed by `deploy_docs.yml` and `deploy_nightly_docs.yml`.

Contributor documentation lives in `extras/zed/CONTRIBUTING.md`, which is unusually candid about culture ("The Zed culture values working code and synchronous conversations over long discussion threads") and routes big features to a written process doc at `extras/zed/docs/src/development/feature-process.md`. Per-platform development guides sit in `extras/zed/docs/src/development/` (`macos.md`, `linux.md`, `windows.md`, `freebsd.md`, `debugging-crashes.md`, `glossary.md`, `release-notes.md`).

Rustdoc conventions: public-surface crates deny or warn on `missing_docs` (Section 8); macro entry points carry executable usage documentation (the `#[gpui::test]` doc block in `extras/zed/crates/gpui_macros/src/gpui_macros.rs` enumerates every accepted argument with examples); and design rationale is written next to the code it justifies (the `NoSummary` impl comment in `sum_tree.rs`). Process templates: `extras/zed/.github/ISSUE_TEMPLATE/` uses YAML issue forms (`10_bug_report.yml`, `11_crash_report.yml`), and the PR template ends in a five-item self-review checklist covering security, unsafe justification, UI guidelines, tests, and performance. Reviewer routing is data, not tribal knowledge: `extras/zed/REVIEWERS.conl` maps code areas to volunteer reviewers.

### 11. Release and distribution

Zed releases on channels, modeled in code: `ReleaseChannel::{Dev, Nightly, Preview, Stable}` in `extras/zed/crates/release_channel/src/lib.rs`, resolved once through a `LazyLock`:

```rust
pub static RELEASE_CHANNEL: LazyLock<ReleaseChannel> =
    LazyLock::new(|| match ReleaseChannel::from_str(&RELEASE_CHANNEL_NAME) {
```

The product version is the `zed` crate version (`1.17.0` in `extras/zed/crates/zed/Cargo.toml`), bumped by generated workflows (`bump_zed_version.yml`, `bump_patch_version.yml`) and branched to `v[0-9]+.[0-9]+.x` stable branches with a `cherry_pick.yml` helper for hotfixes. The channel names even flow into platform identifiers (`ReleaseChannel::Nightly => "Zed-Editor-Nightly"` in the same file), so parallel installs of different channels never collide.

Pushing a `v*` tag triggers `extras/zed/.github/workflows/release.yml` (1,029 lines), which re-runs the full test suite on macOS and Linux, then fans out to bundling jobs (`run_bundling.yml` builds macOS, Linux, Windows, and FreeBSD artifacts), and finally drafts the GitHub release: release notes are assembled from merged PRs by `script/draft-release-notes` and published via `script/create-draft-release`, so changelog discipline is enforced upstream by the Danger job that checks each PR carries a "Release Notes:" section (Renovate PRs even get `"prFooter": "Release Notes:\n\n- N/A"` in `extras/zed/renovate.json`). Nightly is a cron: `0 */4 * * *` in `extras/zed/.github/workflows/release_nightly.yml` checks the nightly tag and rebuilds every four hours. Distribution beyond GitHub releases includes an in-app updater (`crates/auto_update`, `crates/auto_update_helper`), an install script (`extras/zed/script/install.sh`), Nix packaging (`extras/zed/flake.nix`, `extras/zed/nix/`), and Flatpak/snap scripts under `script/`.

The CLI generates completions for six shells, including two most projects forget: `extras/zed/crates/cli/src/completions.rs` defines a `Shell` value-enum covering Bash, Elvish, Fish, Nushell, PowerShell, and Zsh, implementing `clap_complete::Generator` by delegating to `clap_complete`'s shells plus `clap_complete_nushell`.

### 12. Lessons for quinjet

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

---

## BurntSushi/ripgrep (67319 stars)

### 1. What the project is and what the clone measures

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage: ten-plus cited idioms

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

### 10. Documentation practices

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

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## alacritty/alacritty (65390 stars)

### 1. What the project is and why it matters

Alacritty is a cross-platform, GPU-accelerated terminal emulator. The binary crate describes
itself in `extras/alacritty/alacritty/Cargo.toml` as "A fast, cross-platform, OpenGL terminal
emulator". It is one of the most widely deployed end-user Rust desktop applications: it ships on
Linux (X11 and Wayland), macOS, Windows, FreeBSD, and OpenBSD, renders with OpenGL, and is a
default terminal for a large population of developers. For industry, it is a reference for three
things at once: a long-lived Rust GUI application, a reusable terminal-emulation library
(`alacritty_terminal` is published on crates.io for other emulators to build on), and a
disciplined multi-platform release pipeline that produces DMGs, MSIs, man pages, terminfo, and
shell completions from one repository.

Measurable scale from the clone:

| Metric | Value | How measured |
| --- | --- | --- |
| Workspace members | 4 | `[workspace] members` in `extras/alacritty/Cargo.toml` |
| Rust source files | 88 | `find . -name "*.rs"` |
| Rust lines (all `.rs`) | 33,710 | `cat` piped to `wc -l` |
| Lines in `alacritty` (binary) | 20,984 | per-crate `wc -l` |
| Lines in `alacritty_terminal` | 11,877 | per-crate `wc -l` |
| Lines in `alacritty_config_derive` | 748 | per-crate `wc -l` |
| Lines in `alacritty_config` | 101 | per-crate `wc -l` |
| `Cargo.lock` lines | 2,831 | `wc -l` |
| Ref-test fixtures | 45 | `ls alacritty_terminal/tests/ref \| wc -l` |
| User changelog | 1,457 lines | `wc -l CHANGELOG.md` |

The striking ratio: a full terminal emulator in under 34k lines of Rust, with the emulation core
at under 12k lines. The project optimizes for a small, heavily reviewed core rather than breadth.

### 2. Repository layout

```text
extras/alacritty/
|-- Cargo.toml                  workspace root, profiles, patch table
|-- Cargo.lock                  committed (binary project)
|-- rustfmt.toml                nightly rustfmt configuration
|-- .editorconfig               whitespace rules for all file types
|-- Makefile                    macOS app/dmg packaging only
|-- CHANGELOG.md                user-facing keep-a-changelog
|-- CONTRIBUTING.md             testing, style, and release process
|-- INSTALL.md                  380 lines of per-OS build instructions
|-- docs/features.md            prose feature tour (vi mode, hints, search)
|-- scripts/                    color and flamegraph helper shell scripts
|-- extra/                      everything shipped that is not the binary
|   |-- man/                    5 scdoc man page sources
|   |-- completions/            checked-in bash/fish/zsh completions
|   |-- linux/                  .desktop entry, appdata.xml
|   |-- osx/Alacritty.app       app bundle template
|   |-- alacritty.info          terminfo source
|   `-- logo/, promo/           artwork
|-- .github/workflows/          GitHub Actions (Windows, macOS) + release
|-- .builds/                    sourcehut CI (Linux, FreeBSD)
|-- alacritty/                  the binary: UI, renderer, config, CLI
|   `-- src/{cli,config,display,input,renderer,migrate,macos,...}
|-- alacritty_terminal/         reusable emulation library: grid, tty, vte
|   |-- src/{grid,term,tty,event_loop.rs,index.rs,selection.rs,...}
|   |-- tests/ref.rs            golden "ref test" harness
|   `-- tests/ref/<name>/       45 recorded terminal sessions
|-- alacritty_config/           tiny trait crate (SerdeReplace)
`-- alacritty_config_derive/    proc-macro crate for config deserialization
```

The split works because each crate has one reason to exist. `alacritty_terminal` contains zero
windowing or rendering code, so it is testable headlessly and reusable (its manifest at
`extras/alacritty/alacritty_terminal/Cargo.toml` says "Library for writing terminal emulators").
The proc-macro crate is separated because proc macros must be their own crate, and
`alacritty_config` exists only so the derive crate and the binary can share the `SerdeReplace`
trait without a dependency cycle. Distribution assets live under `extras/alacritty/extra/`, away
from source, and the Makefile only handles the one platform (macOS bundling) that cargo cannot.

### 3. Cargo manifest practices

The root `extras/alacritty/Cargo.toml` is only 24 lines and is worth reading in full:

```toml
[workspace]
members = [
    "alacritty",
    "alacritty_terminal",
    "alacritty_config",
    "alacritty_config_derive",
]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.85.0"

[profile.release]
lto = "thin"
debug = 1
incremental = false

[workspace.dependencies]
toml = "0.9.11"
toml_edit = "0.24.0"

# TODO: Validation of fix for #6978. Remove before next release and use released `x11-clipboard`.
[patch.crates-io]
x11-clipboard = { git = "https://github.com/quininer/x11-clipboard.git", rev = "19ab2163cf0bd0db607e827a5214571990307866" }
```

Practices to note:

- `workspace.package` carries exactly the two fields every member must agree on: `edition` and
  `rust-version`. Members inherit with `edition.workspace = true` and
  `rust-version.workspace = true` (see `extras/alacritty/alacritty/Cargo.toml`). Versions,
  authors, and licenses stay per-crate because they genuinely differ: the binary is Apache-2.0
  while `alacritty_config` is `MIT OR Apache-2.0`.
- `workspace.dependencies` is used surgically, not exhaustively: only `toml` and `toml_edit` are
  hoisted, because three crates must deserialize configuration with the exact same TOML version.
  Everything else is declared where it is used.
- The release profile trades a little speed for debuggability: `lto = "thin"` plus `debug = 1`
  keeps line tables in release builds so user crash reports have symbols.
- `[patch.crates-io]` pins a dependency to an exact git revision to validate an upstream fix,
  with a TODO stating the removal condition ("Remove before next release"). Temporary patches
  carry their own expiry note.
- Dependency tables are split by target. `extras/alacritty/alacritty/Cargo.toml` has
  `[target.'cfg(not(windows))'.dependencies]`, `[target.'cfg(target_os = "macos")'.dependencies]`,
  and `[target.'cfg(windows)'.dependencies]`, so `objc2` never appears in a Linux build plan and
  `windows-sys` never appears on Unix.
- Big-surface dependencies get minimal feature lists. `windows-sys` enables only four
  `Win32_*` feature gates; `objc2-app-kit` sets `default-features = false` and lists five
  classes; `ahash` uses `features = ["no-rng"]`; `glutin` sets `default-features = false` with
  explicit `["egl", "wgl"]`.
- Feature design is user-facing: `default = ["wayland", "x11"]`, and each feature fans out into
  the features of five dependencies, e.g.:

```toml
wayland = [
    "copypasta/wayland",
    "glutin/wayland",
    "winit/wayland",
    "winit/wayland-dlopen",
    "winit/wayland-csd-adwaita-crossfont",
]
```

- The library uses the `dep:` syntax to keep optional deps out of the implicit feature list, in
  `extras/alacritty/alacritty_terminal/Cargo.toml`:

```toml
[features]
default = ["serde"]
serde = ["dep:serde", "bitflags/serde", "vte/serde"]
```

- Internal path dependencies always carry a version (`version = "0.26.1-dev"` next to
  `path = "../alacritty_terminal"`) so the crates remain publishable to crates.io.
- Development versions use a `-dev` suffix (`version = "0.18.0-dev"`), which the release process
  in `extras/alacritty/CONTRIBUTING.md` strips on the release branch.

There is no `[lints]` table anywhere; lint policy lives in crate roots (section 5).

### 4. Formatting

`extras/alacritty/rustfmt.toml` runs on nightly (`cargo +nightly fmt` in
`extras/alacritty/.builds/linux.yml`) and enables 15 unstable options:

```toml
format_code_in_doc_comments = true
match_block_trailing_comma = true
condense_wildcard_suffixes = true
use_field_init_shorthand = true
normalize_doc_attributes = true
overflow_delimited_expr = true
imports_granularity = "Module"
use_small_heuristics = "Max"
normalize_comments = true
reorder_impl_items = true
use_try_shorthand = true
newline_style = "Unix"
format_strings = true
wrap_comments = true
comment_width = 100
```

Setting by setting: `format_code_in_doc_comments` formats Rust inside doc examples;
`match_block_trailing_comma` forces a comma after block match arms (visible throughout, e.g. the
`grid_clamp` match in `extras/alacritty/alacritty_terminal/src/index.rs`);
`condense_wildcard_suffixes` turns `(a, _, _)` into `(a, ..)`; `use_field_init_shorthand`
rewrites `Foo { x: x }` to `Foo { x }`; `normalize_doc_attributes` converts `#[doc = "..."]` to
`///`; `overflow_delimited_expr` lets a trailing collection literal overflow instead of
indenting, which is why the `Registry::new(...).write_bindings(...)` call in
`extras/alacritty/alacritty/build.rs` reads naturally; `imports_granularity = "Module"` merges
imports per module, giving the consistent `use std::sync::{Arc, Mutex, OnceLock};` style;
`use_small_heuristics = "Max"` packs anything that fits into the width onto one line (the
one-line struct literal in `FairMutex::new` in
`extras/alacritty/alacritty_terminal/src/sync.rs` is a direct product);
`normalize_comments` converts `/* */` to `//`; `reorder_impl_items` groups consts, types, then
fns inside impls; `use_try_shorthand` replaces `r#try!`/`try!` with `?`;
`newline_style = "Unix"` forces LF even on Windows checkouts; `format_strings` breaks long
string literals; `wrap_comments` plus `comment_width = 100` rewraps prose comments at 100
columns while code stays at the default width.

Non-Rust files are governed by `extras/alacritty/.editorconfig`: UTF-8, LF, trimmed trailing
whitespace and final newline for every file, 4-space indent for `glsl`, `rs`, and `toml`, tabs
for `Makefile`, and tabs with `tab_width = 4` for the scdoc man sources:

```ini
[*.{glsl,rs,toml}]
indent_style = space
indent_size = 4

[Makefile]
indent_style = tab

[*.scd]
indent_style = tab
tab_width = 4
```

Beyond machine formatting, `extras/alacritty/CONTRIBUTING.md` adds a human rule: "All comments
should be fully punctuated with a trailing period. This applies both to regular and
documentation comments."

### 5. Linting

There is no `clippy.toml` and no `[lints]` table. The entire lint wall is three attribute lines
repeated at every crate root, e.g. `extras/alacritty/alacritty/src/main.rs`:

```rust
#![warn(rust_2018_idioms, future_incompatible)]
#![deny(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
#![cfg_attr(clippy, deny(warnings))]
```

The same lines appear in `extras/alacritty/alacritty_terminal/src/lib.rs` and
`extras/alacritty/alacritty_config_derive/src/lib.rs`. The philosophy:

- `clippy::all` is denied, not warned, so correctness and style lints are hard CI failures.
- Exactly two opt-in lints are added by name: `clippy::if_not_else` (readability of negated
  conditionals) and `clippy::enum_glob_use` (no `use Enum::*`). No pedantic or nursery groups:
  the project prefers a small, permanent set over a large set with many local `allow`s.
- `#![cfg_attr(clippy, deny(warnings))]` denies all rustc warnings only when the `clippy` cfg is
  set, so ordinary `cargo build` never fails on a new rustc warning but lint CI does. The
  sourcehut jobs get the same effect for feature builds via
  `RUSTFLAGS="-D warnings" cargo test --no-default-features --features=wayland`
  (`extras/alacritty/.builds/linux.yml`).
- Allows are local, justified, and rare. Generated code gets a blanket exemption in
  `extras/alacritty/alacritty/src/main.rs`:

```rust
mod gl {
    #![allow(clippy::all, unsafe_op_in_unsafe_fn)]
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}
```

  and a single-expression allow appears as `#[allow(clippy::iter_skip_zero)]` in
  `extras/alacritty/alacritty/src/string.rs`, where `skip(0)` is intentional to unify types.

- CI runs `cargo clippy --all-targets` on every platform (`extras/alacritty/.github/workflows/ci.yml`
  and both `.builds` manifests), so tests and examples are linted too.
- `extras/alacritty/alacritty/src/main.rs` also carries a build-time configuration lint:

```rust
#[cfg(not(any(feature = "x11", feature = "wayland", target_os = "macos", windows)))]
compile_error!(r#"at least one of the "x11"/"wayland" features must be enabled"#);
```

### 6. CI/CD

CI is split across two systems by operating system, which is the most unusual choice in the
repository.

`extras/alacritty/.github/workflows/ci.yml` covers Windows and macOS, triggered by
`on: [push, pull_request]`:

```yaml
jobs:
  build:
    strategy:
      matrix:
        os: [windows-latest, macos-latest]
```

Its steps: `cargo test` on stable; `cargo test -p alacritty_terminal --no-default-features` (the
library must build without serde); an "Oldstable" step that extracts the MSRV from the manifest
so there is one source of truth:

```yaml
- name: Oldstable
  run: |
    rustup default $(cat Cargo.toml | grep "rust-version" | sed 's/.*"\(.*\)".*/\1/')
    cargo test
```

and `cargo clippy --all-targets`. A second job, `check-macos-x86_64`, cross-checks the Intel
macOS target from an ARM runner (`rustup target add x86_64-apple-darwin` then
`cargo build --target=x86_64-apple-darwin`).

Linux and FreeBSD run on sourcehut builds, configured in `extras/alacritty/.builds/linux.yml`
(Arch image) and `extras/alacritty/.builds/freebsd.yml`. The Linux manifest adds jobs GitHub
does not run: nightly `rustfmt` (`rustup toolchain install nightly -c rustfmt` then
`cargo +nightly fmt -- --check`), man page compilation as a docs gate
(`cat extra/man/alacritty.1.scd | scdoc > /dev/null` for all five pages), the same
grep-the-manifest MSRV job, and a per-feature matrix run as separate tasks:

```yaml
- feature-wayland: |
    cd alacritty/alacritty
    RUSTFLAGS="-D warnings" cargo test --no-default-features --features=wayland
- feature-x11: |
    cd alacritty/alacritty
    RUSTFLAGS="-D warnings" cargo test --no-default-features --features=x11
```

Hardening and hygiene observations: the only third-party action used anywhere is
`actions/checkout@v4`, pinned by major tag; there is no dependency caching at all (build times
are acceptable because the tree is small, and no cache means no cache-poisoning surface); the
release workflow's token exposure is limited to `secrets.GITHUB_TOKEN`; there is no merge queue
or required-check configuration visible in-repo. Release automation
(`extras/alacritty/.github/workflows/release.yml`) triggers on tag pushes matching
`tags: ["v[0-9]+.[0-9]+.[0-9]+*"]`, and every packaging job re-runs `cargo test --release`
before building artifacts, so the exact optimized binaries being shipped are the ones tested.

### 7. Testing

Three layers, each in the conventional Rust location:

1. Inline unit tests. Nineteen files contain a `mod tests` block, found via
   `grep -rn "mod tests"`: parser-adjacent logic (`extras/alacritty/alacritty_terminal/src/term/search.rs`,
   `.../selection.rs`, `.../vi_mode.rs`, `.../grid/storage.rs`), and UI-side pure logic
   (`extras/alacritty/alacritty/src/config/bindings.rs`, `.../display/damage.rs`,
   `.../string.rs`, `.../migrate/mod.rs`). The grid gets a dedicated sibling file wired as
   `mod tests;` in `extras/alacritty/alacritty_terminal/src/grid/mod.rs`; that file even
   implements `GridCell for usize` (`extras/alacritty/alacritty_terminal/src/grid/tests.rs`) so
   grid algorithms are tested on plain integers instead of full cells.

2. Golden "ref tests", the project's signature harness. A real Alacritty run with the hidden
   `--ref-test` flag (declared in `extras/alacritty/alacritty/src/cli.rs` with
   `#[clap(long, conflicts_with("daemon"))]`) records the raw PTY byte stream and final state to
   disk. Each fixture directory, e.g.
   `extras/alacritty/alacritty_terminal/tests/ref/vim_large_window_scroll/`, holds four files:
   `alacritty.recording`, `grid.json`, `size.json`, `config.json`. The harness
   `extras/alacritty/alacritty_terminal/tests/ref.rs` replays the bytes through a headless
   `Term` and diffs the resulting grid cell by cell. Test registration is a declarative macro:

   ```rust
   macro_rules! ref_tests {
    ($($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                let test_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ref"));
                let test_path = test_dir.join(stringify!($name));
                ref_test(&test_path);
            }
        )*
    };
   }
   ```

   45 fixtures cover vttest sequences, tmux, vim, fish, zsh completion, hyperlinks, and named
   regressions like `issue_855`. Headless replay is possible because the UI is abstracted behind
   a no-op listener: `struct Mock; impl EventListener for Mock { fn send_event(&self, _e: Event) {} }`.
   `extras/alacritty/CONTRIBUTING.md` requires that a bug-fix ref test fail against the
   unpatched binary before it is accepted.

3. Integration tests for the proc macro. `extras/alacritty/alacritty_config_derive/tests/config.rs`
   (245 lines) exercises every derive attribute (`#[config(alias)]`, `deprecated`, `removed`,
   `skip`, `flatten`, `#[doc(hidden)]`) and asserts on the log records the derive emits for bad
   input, using a captured logger.

The CLI surface is tested end to end in an unusual way: the checked-in shell completions are
asserted byte-for-byte against what `clap_complete` generates, in
`extras/alacritty/alacritty/src/cli.rs`:

```rust
let mut generated = Vec::new();
clap_complete::generate(*shell, &mut clap, "alacritty", &mut generated);
...
assert_eq!(generated, completion);
```

so any CLI flag change that forgets to regenerate `extras/alacritty/extra/completions/` fails
`cargo test`. There is no fuzzing, property testing, or in-repo benchmark suite; throughput is
benchmarked with the external `vtebench` tool and latency with `typometer`, both mandated by the
Performance section of `extras/alacritty/CONTRIBUTING.md`.

### 8. Error handling and API design

Neither `thiserror` nor `anyhow` appears anywhere in the workspace (verified by grep across all
manifests). Errors are hand-written enums with manual `Display` and `source`, e.g.
`extras/alacritty/alacritty/src/config/mod.rs`:

```rust
/// Errors occurring during config loading.
#[derive(Debug)]
pub enum Error {
    /// Couldn't read $HOME environment variable.
    ReadingEnvHome(env::VarError),
    /// io error reading file.
    Io(io::Error),
    /// Invalid toml.
    Toml(TomlError),
    ...
}
```

with a module-local alias `pub type Result<T> = std::result::Result<T, Error>;`. The dependency
cost of an error-derive crate is judged not worth five variants.

Result discipline and exit behavior: `fn main() -> Result<(), Box<dyn Error>>`
(`extras/alacritty/alacritty/src/main.rs`) propagates startup errors; non-interactive
subcommands print to stderr and exit nonzero explicitly, as in
`extras/alacritty/alacritty/src/migrate/mod.rs`:

```rust
None => {
    eprintln!("No configuration file found");
    std::process::exit(1);
},
```

The migrate subcommand also runs a forced dry-run pass before any wet run touches the user's
file, and writes through `tempfile::NamedTempFile`. Panic policy: panics are reserved for
programmer errors (`expect("thread spawn works")` in
`extras/alacritty/alacritty_terminal/src/thread.rs`), and on Windows, where there is no console,
a custom hook mirrors the panic into a native dialog
(`extras/alacritty/alacritty/src/panic.rs`, `MessageBoxW` with "Press Ctrl-C to Copy").
Resource cleanup is typed: `struct TemporaryFiles` in `extras/alacritty/alacritty/src/main.rs`
removes the IPC socket and log file in its `Drop` impl, so every exit path cleans up.

API design: newtypes over raw indices (`Line`, `Column` in
`extras/alacritty/alacritty_terminal/src/index.rs`, with
`#[must_use = "this returns the result of the operation, without modifying the original"]` on
the pure arithmetic methods); visibility is deliberate, e.g. `Shell` in
`extras/alacritty/alacritty_terminal/src/tty/mod.rs` exposes `pub(crate)` fields behind a public
constructor; options structs (`tty::Options`, `term::Config`) with `Default` play the role of
builders; and the library re-exports its parser (`pub use vte;` in
`extras/alacritty/alacritty_terminal/src/lib.rs`) so downstreams never fight version skew on
shared types.

### 9. Deep Rust usage: ten cited idioms

1. Fairness-augmented mutex. `extras/alacritty/alacritty_terminal/src/sync.rs` composes two
   `parking_lot::Mutex`es into a `FairMutex<T>` with a `lease()` method, so the render thread
   can reserve the terminal lock while the PTY reader thread is hammering it. The comment in
   `lock()` even documents a subtle temporary-lifetime hazard: "Must bind to a temporary or the
   lock will be freed before going into data.lock()".

2. Newtype index arithmetic with generic defaults.
   `extras/alacritty/alacritty_terminal/src/index.rs` defines
   `pub struct Point<L = Line, C = Column>`, so the same type serves grid-space and
   viewport-space coordinates, and implements `Add`, `Sub`, `AddAssign`, `Ord`, and clamping
   (`grid_clamp` with a `Boundary` enum) so off-by-one line math is centralized and `#[must_use]`
   guarded.

3. A ring buffer with measured `unsafe`. `Storage<T>` in
   `extras/alacritty/alacritty_terminal/src/grid/storage.rs` scrolls by rotating a `zero` offset
   instead of moving memory, reimplements `Index`/`IndexMut` around the wraparound, and
   deliberately provides no `Deref` to `Vec` ("Anything from `Vec` that should be exposed must
   be done so manually"). Its custom `swap` copies four qwords through `MaybeUninit<usize>`
   pointers, justified by counted instructions ("The default implementation from swap generates
   8 movups and 4 movaps instructions") and guarded by
   `debug_assert_eq!(mem::size_of::<Row<T>>(), mem::size_of::<usize>() * 4)`.

4. Trait-decoupled core. `Term<U: EventListener>` sends UI-relevant events through the trait in
   `extras/alacritty/alacritty_terminal/src/event.rs` (window title, clipboard, bell), including
   callback-carrying variants like
   `ClipboardLoad(ClipboardType, Arc<dyn Fn(&str) -> String + Sync + Send + 'static>)`. This one
   boundary is what makes headless ref tests and the library's reuse by other emulators possible.

5. `Cow<'static, [u8]>` on the hot write path. The PTY event loop
   (`extras/alacritty/alacritty_terminal/src/event_loop.rs`) queues writes as
   `Msg::Input(Cow<'static, [u8]>)` and stores `write_list: VecDeque<Cow<'static, [u8]>>`, so
   static escape sequences are enqueued without allocation while dynamic input still fits the
   same channel.

6. Platform layering via `cfg` re-export. `extras/alacritty/alacritty_terminal/src/tty/mod.rs`
   presents one API from two implementations:

   ```rust
   #[cfg(not(windows))]
   mod unix;
   #[cfg(not(windows))]
   pub use self::unix::*;

   #[cfg(windows)]
   pub mod windows;
   ```

   The same discipline extends to docs: `extras/alacritty/alacritty/src/cli.rs` declares
   `config_file` three times under different `cfg`s purely so `--help` shows the correct default
   path per OS.

7. Build-script codegen plus embedded version. `extras/alacritty/alacritty/build.rs` generates
   OpenGL bindings with `gl_generator` into `OUT_DIR`, and computes
   `println!("cargo:rustc-env=VERSION={version}")` including the short git hash; the CLI then
   uses `#[clap(author, about, version = env!("VERSION"))]`
   (`extras/alacritty/alacritty/src/cli.rs`), so `alacritty --version` identifies dev builds by
   commit.

8. A purpose-built derive macro for resilient config.
   `extras/alacritty/alacritty_config_derive/src/lib.rs` ships
   `#[proc_macro_derive(ConfigDeserialize, attributes(config))]` and `SerdeReplace`. The derive
   makes every field individually recoverable: a bad value logs a warning and falls back to that
   field's default rather than rejecting the file, and attributes encode config lifecycle
   (`#[config(deprecated = "use field2 instead")]`, `#[config(removed = "it's gone")]`,
   `#[config(alias = ...)]`) as shown in
   `extras/alacritty/alacritty_config_derive/tests/config.rs`. `SerdeReplace` then lets a CLI
   dotted path like `-o window.opacity=0.5` splice into the deserialized struct
   (`extras/alacritty/alacritty_config/src/lib.rs`).

9. Lazy DFA regex over a ring buffer. Scrollback search
   (`extras/alacritty/alacritty_terminal/src/term/search.rs`) drops down from the `regex` crate
   to `regex-automata`'s hybrid lazy DFA, holding four DFAs (forward and reverse, left and
   right anchored) to find both ends of matches while iterating the grid bidirectionally, with
   smart-case derived by `search.chars().any(|c| c.is_uppercase())` and cache bounds copied from
   the meta engine with a citation link in the comment.

10. Small ergonomic infrastructure everywhere: named threads via a nine-line wrapper
    (`spawn_named` in `extras/alacritty/alacritty_terminal/src/thread.rs`, "Like
    `thread::spawn`, but with a `name` argument"); lazy statics through
    `OnceLock` reading `ALACRITTY_EXTRA_LOG_TARGETS` in
    `extras/alacritty/alacritty/src/logging.rs`; an `Iterator` implementation for width-aware
    string truncation (`StrShortener` in `extras/alacritty/alacritty/src/string.rs`) driven by a
    small `TextAction` state enum; and single-threaded interior mutability chosen deliberately
    (`OnceCell`, `RefCell`, `Rc` in `extras/alacritty/alacritty/src/config/ui_config.rs`) where
    the UI thread owns the data, versus `Arc<FairMutex<...>>` only where the PTY thread truly
    shares it.

### 10. Documentation practices

- Module-level `//!` docs open most files and state contracts, not just topics:
  `extras/alacritty/alacritty/src/logging.rs` begins "The main executable is supposed to call
  `initialize()` exactly once during startup." Doc comments use intra-doc links and reference
  definitions (see the `Storage` header in
  `extras/alacritty/alacritty_terminal/src/grid/storage.rs`).
- `extras/alacritty/CONTRIBUTING.md` is the process spine: how to add ref tests, which
  benchmarking tools to use, the API-guidelines pointer for style, the trailing-period comment
  rule, the rule that config changes must update the man pages, and a fully scripted release
  process (14 numbered steps for a feature release, 6 for a patch release).
- User docs are split by audience: `extras/alacritty/README.md` for the pitch,
  `extras/alacritty/INSTALL.md` (380 lines) for building on every OS,
  `extras/alacritty/docs/features.md` for feature discovery, and five scdoc man pages in
  `extras/alacritty/extra/man/` as the configuration reference, with
  `alacritty.5.scd` alone at 1,075 lines. Man pages are compiled in CI so they cannot rot
  (`extras/alacritty/.builds/linux.yml`).
- `extras/alacritty/CHANGELOG.md` opens by legislating its own format: "The sections should
  follow the order `Packaging`, `Added`, `Changed`, `Fixed` and `Removed`", and points to the
  separate `extras/alacritty/alacritty_terminal/CHANGELOG.md` for library consumers.
- There is no ARCHITECTURE.md and no issue-template directory; the PR template
  (`extras/alacritty/.github/pull_request_template.md`) is a single checkbox line asserting the
  patch complies with the project's contribution-provenance policy.

### 11. Release and distribution

Versioning follows a release-branch model documented step by step in
`extras/alacritty/CONTRIBUTING.md`: master only ever carries `X.Y.0-dev` versions; each major
release gets a `vX.Y` branch where `-rcN` tags, the final tag, and patch releases live; fixes
are cherry-picked from master into the branch; and the library crate is tagged separately as
`alacritty_terminal_vX.Y.Z` with suffixes kept in sync.

Tagging `vX.Y.Z` fires `extras/alacritty/.github/workflows/release.yml`:

- macOS: installs `scdoc`, adds both Apple targets, runs `cargo test --release` on x86_64,
  builds ARM, then `make dmg-universal`. The Makefile (`extras/alacritty/Makefile`) lipo-merges
  the two binaries, gzips the man pages, compiles terminfo with
  `tic -xe alacritty,alacritty-direct`, copies completions into the app bundle, and ad-hoc
  codesigns (`codesign --force --deep --sign -`).
- Windows: uploads a portable `Alacritty-vX.Y.Z-portable.exe` and builds an MSI with WiX 4 from
  `alacritty/windows/wix/alacritty.wxs`.
- Linux: uploads no binaries at all, only integration assets: gzipped man pages, the SVG logo,
  three shell completions, `Alacritty.desktop`, and `alacritty.info`. Distro packagers build the
  binary; the project ships the surrounding material.

Uploads go through `extras/alacritty/.github/workflows/upload_asset.sh`, a 100-line bash script
that finds or creates a draft release for the current tag via the GitHub API
(`-d "{\"tag_name\":\"$tag\",\"draft\":true}"`), so a human publishes the release after all
three OS jobs have attached their artifacts. Completions are not generated at release time; they
are checked in under `extras/alacritty/extra/completions/` and enforced by the unit test in
`extras/alacritty/alacritty/src/cli.rs` (section 7), which keeps packaging simple and diffs
reviewable.

### 12. Lessons for quinjet

Practices worth adopting, with mechanisms:

1. Completion drift test. Add `clap_complete` to `[dev-dependencies]`, check generated
   completions into `extra/completions/`, and add a `#[test]` that runs
   `clap_complete::generate(shell, &mut Options::command(), "quinjet", &mut buf)` and
   `assert_eq!` against the checked-in files, exactly as
   `extras/alacritty/alacritty/src/cli.rs` does. Every `clap` change then fails tests until
   completions are regenerated.

2. Golden replay tests for the TUI. Mirror the ref-test harness
   (`extras/alacritty/alacritty_terminal/tests/ref.rs`): a hidden `--ref-test`-style flag dumps
   the final ratatui buffer plus the inputs that produced it into a fixture directory under
   `tests/ref/<name>/`, and a `ref_tests! { ... }` declarative macro expands one `#[test]` per
   fixture that replays and diffs cell by cell, printing only mismatched cells before panicking.

3. Version with commit hash. In `build.rs`, run `git rev-parse --short HEAD`, emit
   `cargo:rustc-env=VERSION={pkg_version} ({hash})`, and use
   `#[clap(version = env!("VERSION"))]`, copying
   `extras/alacritty/alacritty/build.rs`. Dev-build bug reports become attributable.

4. Debuggable release profile. Set `debug = 1` alongside the existing optimizations in
   `[profile.release]` as in `extras/alacritty/Cargo.toml`, so user-reported backtraces from a
   Git TUI crash carry line numbers.

5. MSRV from one source in CI. Replace any hardcoded toolchain in the MSRV job with the
   grep-the-manifest pattern from `extras/alacritty/.github/workflows/ci.yml`
   (`rustup default $(grep "rust-version" Cargo.toml | sed ...)` then `cargo test`), so bumping
   `rust-version` is a one-line change.

6. Feature and no-default-features CI legs. Add `cargo test --no-default-features` (and one leg
   per meaningful feature with `RUSTFLAGS="-D warnings"`) as in
   `extras/alacritty/.builds/linux.yml`, so optional-dependency breakage is caught.

7. Docs compiled in CI. If quinjet ships man pages, write them as scdoc in `extra/man/` and add
   a CI step that pipes each through `scdoc > /dev/null`
   (`extras/alacritty/.builds/linux.yml`); a syntax error in docs then fails the build.

8. Changelog with legislated section order. Start `CHANGELOG.md` with the format rule itself,
   as `extras/alacritty/CHANGELOG.md` does ("sections should follow the order `Packaging`,
   `Added`, `Changed`, `Fixed` and `Removed`"), and require entries for user-visible changes in
   CONTRIBUTING.

9. Typed cleanup on exit. Wrap temp artifacts (sockets, log files, lock files) in a struct
   whose `Drop` removes them, like `TemporaryFiles` in
   `extras/alacritty/alacritty/src/main.rs`, instead of scattering cleanup across exit paths.

10. Dry-run before wet-run for destructive subcommands. Quinjet's history-rewriting operations
    can copy `extras/alacritty/alacritty/src/migrate/mod.rs`: force a silent dry-run first, exit
    nonzero on any failure, and only then perform the real mutation, writing through
    `tempfile::NamedTempFile`.

11. Named threads. Add a `spawn_named` helper identical to
    `extras/alacritty/alacritty_terminal/src/thread.rs` for any background Git work, so thread
    names show up in panics and debuggers.

12. `compile_error!` for invalid feature sets. If quinjet ever grows mutually optional
    backends, guard them at the crate root as `extras/alacritty/alacritty/src/main.rs` does.

13. Release asset script with draft releases. For binary distribution, adopt the
    find-or-create-draft pattern of `extras/alacritty/.github/workflows/upload_asset.sh`: jobs
    attach artifacts to a draft, and a human clicks publish, decoupling build success from
    release visibility.

Two practices quinjet already exceeds: Alacritty has no equivalent of quinjet's clippy
restriction wall, cargo-deny, taplo, typos, coverage floor, miri, or mutants; and Alacritty
performs no dependency caching or action SHA-pinning in CI. The traffic in lessons on those
axes flows the other way.

---

## sharkdp/bat (60188 stars)

### 1. What the project is and why it matters

bat is "a cat(1) clone with wings": a syntax-highlighting file printer with Git modification
markers, automatic paging, and theming. The manifest at extras/bat/Cargo.toml describes it in one
line:

```toml
description = "A cat(1) clone with wings."
categories = ["command-line-utilities"]
license = "MIT OR Apache-2.0"
```

Industry uses it for two reasons. First, as a daily-driver CLI it replaces `cat` and `less` for
code reading, and it ships in every major package manager (the CI builds Debian packages and
publishes to Winget directly, see section 6). Second, it is also a library: tools such as `delta`
depend on the `bat` crate for pretty-printing, which is why the crate carries an unusually
disciplined feature-flag surface separating "bat the application" from "bat the library"
(section 3).

Measured scale from the clone at extras/bat:

- Single crate, no workspace. One package `bat`, version 0.26.1, edition 2021, MSRV 1.88.
- 67 Rust source files, 18,173 lines of Rust total: 12,094 in extras/bat/src, roughly 5,452 in
  extras/bat/tests, 490 in the build script under extras/bat/build, the rest in
  extras/bat/examples and extras/bat/assets/theme_preview.rs.
- 93 git submodules (extras/bat/.gitmodules), almost all of them Sublime Text syntax and theme
  repositories vendored under extras/bat/assets.
- 273 `#[test]` functions in extras/bat/tests/integration_tests.rs alone.

The striking property of the codebase is leverage: a small core (about 12k lines) drives a very
large data surface (syntaxes, themes, mappings) through build-time code generation and binary
asset embedding.

### 2. Repository layout

Top level of extras/bat:

```text
bat/
|-- assets/            syntax/theme submodules, completions templates, man page template,
|                      pre-built binary assets (syntaxes.bin, themes.bin, acknowledgements.bin)
|-- build/             the build script, split into modules (main.rs, application.rs,
|                      syntax_mapping.rs, util.rs)
|-- diagnostics/       info.sh, the script behind `bat --diagnostic` bug reports
|-- doc/               assets.md, alternatives.md, release-checklist.md, long-help.txt,
|                      short-help.txt, translated READMEs (ja, ko, ru, zh)
|-- examples/          7 library-usage examples (cat.rs, advanced.rs, yaml.rs, ...)
|-- src/               the library crate root
|   |-- assets/        asset loading, lazy theme set, serialized syntax set
|   |-- bin/bat/       the application binary (app.rs, clap_app.rs, config.rs, main.rs, ...)
|   `-- syntax_mapping/ builtin.rs plus builtins/ TOML rule files per platform
|-- tests/             integration tests, snapshot tests, syntax regression corpus, benchmarks
|-- .cargo/            config.toml (crt-static for Windows), audit.toml (RUSTSEC ignores)
|-- .github/           two workflows, dependabot.yml, four issue templates
|-- Cargo.toml, Cargo.lock, rustfmt.toml, flake.nix, .envrc
`-- CHANGELOG.md, CONTRIBUTING.md, SECURITY.md, NOTICE, LICENSE-MIT, LICENSE-APACHE
```

Why this split works:

- Library and binary live in one crate but are physically separated: extras/bat/src/lib.rs is the
  library, and the application lives under extras/bat/src/bin/bat/ as eight modules (app.rs,
  clap_app.rs, config.rs, directories.rs, input.rs, completions.rs, assets.rs, main.rs). CLI
  parsing, config-file merging, and environment handling never leak into the library.
- The build script is a directory, not a single file. extras/bat/build/main.rs is 17 lines and
  delegates to `syntax_mapping.rs` (368 lines of code generation) and `application.rs` (man page
  and completion rendering), keeping each build concern reviewable.
- Data lives next to the code that owns it: syntax mapping rules are TOML files under
  extras/bat/src/syntax_mapping/builtins/{common,unix-family,bsd-family,linux,macos,windows},
  with a README.md in that directory explaining the format. 27 TOML files exist in common/ alone.
- Tests own their fixtures: extras/bat/tests/examples is a small fake filesystem (config files,
  control_characters.txt, a git directory), extras/bat/tests/mocked-pagers holds fake `more` and
  `most` executables, and extras/bat/tests/snapshots holds committed expected outputs.

### 3. Cargo manifest practices

extras/bat/Cargo.toml is a single-package manifest, so there is no `workspace.package`
inheritance, but it demonstrates several practices worth copying.

MSRV is explicit, with a policy comment right in the manifest:

```toml
edition = '2021'
# You are free to bump MSRV as soon as a reason for bumping emerges.
rust-version = "1.88"
```

CI reads that value back out with `cargo metadata` so the MSRV is stated in exactly one place
(section 6).

Feature flags encode the library/application split:

```toml
[features]
default = ["application", "git"]
# Feature required for bat the application. Should be disabled when depending on
# bat as a library.
application = [
    "bugreport",
    "build-assets",
    "minimal-application",
]
# Mainly for developers that want to iterate quickly
minimal-application = [
    "clap",
    "etcetera",
    "paging",
    "regex-onig",
    "wild",
]
git = ["gix"] # Support indicating git modifications
paging = [ "shell-words", "grep-cli", "minus"] # Support applying a pager on the output
lessopen = ["execute"] # Support $LESSOPEN preprocessor
```

Notice that nearly every heavyweight dependency (clap, gix, minus, grep-cli, wild, bugreport,
regex, walkdir) is `optional = true` and pulled in only via a feature. A library consumer that
disables default features gets a much smaller dependency tree, and the manifest tells them what
they must choose: `regex-onig` or `regex-fancy`, the two syntect regex engines.

Dependency hygiene details:

- Transitive default features are trimmed aggressively: `gix` is declared with
  `default-features = false, features = ["sha1", "blob-diff"]`, `syntect` with
  `default-features = false, features = ["parsing"]`, `clircle` and `path_abs` with
  `default-features = false`.
- Platform-conditional dependencies are used instead of cfg-gated code with unused deps:
  `[target.'cfg(target_os = "macos")'.dependencies] plist = "1.9.0"` and
  `[target.'cfg(unix)'.dev-dependencies] nix = { ... features = ["term"] }`.
- Packaging excludes the huge submodule trees: `exclude = ["assets/syntaxes/*",
  "assets/themes/*"]`, so the crates.io tarball ships only the pre-built .bin assets.
- The build script has its own substantial dependency set (`prettyplease`, `proc-macro2`,
  `quote`, `syn`, `serde_with`, `indexmap`, `toml`) under `[build-dependencies]` because the
  build script does real code generation (section 9).

The release profile is tuned for a distributed binary:

```toml
[profile.release]
lto = true
strip = true
codegen-units = 1
```

There is no `[lints]` table and no clippy.toml; lint policy lives in CI flags and crate
attributes (section 5). Cargo-level config that does exist is in extras/bat/.cargo/config.toml:

```toml
# On Windows MSVC, statically link the C runtime so that the resulting EXE does
# not depend on the vcruntime DLL.
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

with the same for i686 and aarch64 MSVC, and extras/bat/.cargo/audit.toml pins accepted advisory
exceptions: `ignore = ["RUSTSEC-2024-0320", "RUSTSEC-2024-0421"]`.

### 4. Formatting

extras/bat/rustfmt.toml is a single comment line:

```toml
# Defaults are used
```

This is a deliberate statement, not an omission: the file exists so that editors and CI agree
that stock rustfmt is the standard, and so nobody adds unstable options later without a visible
diff to this file. CI enforces it with `cargo fmt -- --check` in the lint job of
extras/bat/.github/workflows/CICD.yml.

There is no .editorconfig and no formatter config for the shell, Python, YAML, or TOML files in
the repository. Non-Rust quality is enforced through behavior instead: the shell scripts use
strict modes (`set -o errexit -o nounset -o pipefail` in extras/bat/tests/scripts/license-checks.sh,
`set -euo pipefail` in extras/bat/assets/create.sh), and the syntax-comparison tooling is Python
scripts under extras/bat/tests/syntax-tests that are themselves exercised by CI.

### 5. Linting

bat's lint setup is minimal and CI-centric. There is no clippy.toml, no deny.toml, and no
`[lints]` table in extras/bat/Cargo.toml. Instead:

1. CI runs clippy as a hard wall over every target and feature combination
   (extras/bat/.github/workflows/CICD.yml, lint job):

   ```yaml
   - run: cargo fmt -- --check
   - run: cargo clippy --locked --all-targets --all-features -- -D warnings
   ```

2. Both crate roots forbid unsafe code at the source level. extras/bat/src/lib.rs line 22 and
   extras/bat/src/bin/bat/main.rs line 1 carry:

   ```rust
   #![deny(unsafe_code)]
   ```

   and there is not a single `unsafe` block anywhere under extras/bat/src.

3. Allows are narrow, local, and justified. The whole of src contains only a handful, for
   example extras/bat/src/vscreen.rs uses `#[allow(clippy::upper_case_acronyms)]` on individual
   ANSI-sequence enum variants, and extras/bat/build/syntax_mapping.rs uses
   `#[allow(clippy::enum_variant_names)]` on one enum. Nothing is allowed crate-wide.

4. Rustdoc is linted as strictly as code: the documentation CI job runs
   `cargo doc --locked --no-deps --document-private-items --all-features` with
   `RUSTDOCFLAGS: -D warnings`.

The philosophy: default clippy at `-D warnings` with `--all-targets --all-features`, kept
green permanently, beats a curated lint list that drifts. The custom check infrastructure that
does exist targets project-specific invariants that no lint can see:

- extras/bat/tests/scripts/license-checks.sh greps the whole tree, submodules included, for
  "General Public License" to prevent GPL contamination of an MIT/Apache project, with an
  explicit exclude list for false positives.
- extras/bat/tests/no_duplicate_extensions.rs asserts that no two embedded syntaxes claim the
  same file extension, with a `KNOWN_EXCEPTIONS` list documenting each collision that is allowed
  (`.h`, `.js`, `.sass`, `.fs`, `.v`) and why.

### 6. CI/CD

There are exactly two workflows in extras/bat/.github/workflows: CICD.yml (464 lines, the whole
pipeline) and require-changelog-for-PRs.yml (33 lines).

#### CICD.yml

Triggers: `workflow_dispatch`, `pull_request`, and `push` to `master` plus all tags. One
workflow covers PR validation, master builds, and tag releases; release-only steps are gated by
`if: startsWith(github.ref, 'refs/tags/v')` style conditions rather than a separate file.

The jobs:

- `all-jobs`: a required-check aggregator. It `needs` every other job and asserts they all
  succeeded:

```yaml
all-jobs:
  if: always() # Otherwise this job is skipped if the matrix job fails
  needs:
    - crate_metadata
    - lint
    - min_version
    - license_checks
    - test_with_new_syntaxes_and_themes
    - test_with_system_config
    - documentation
    - cargo-audit
    - build
  steps:
    - run: jq --exit-status 'all(.result == "success")' <<< '${{ toJson(needs) }}'
```

  Branch protection needs to require only this one job. And bat closes the obvious failure mode
  (someone adds a job and forgets to list it) with a meta-test:
  extras/bat/tests/github-actions.rs parses CICD.yml with serde_yaml and asserts that
  `all-jobs.needs` equals the full job list minus documented exceptions (`all-jobs` itself and
  the release-only `winget` job). The CI config is under test by the test suite it runs.

- `crate_metadata`: extracts name, version, maintainer, homepage, and MSRV from
  `cargo metadata --no-deps --format-version 1` piped through jq into `$GITHUB_OUTPUT`. Every
  downstream job (MSRV toolchain selection, artifact naming, Debian control files) consumes
  these outputs, so Cargo.toml is the single source of truth.

- `min_version`: installs the exact MSRV toolchain with
  `dtolnay/rust-toolchain@master` and `toolchain: ${{ needs.crate_metadata.outputs.msrv }}`,
  then runs the test suite with a reduced feature set defined once at the top of the file:
  `MSRV_FEATURES: --no-default-features --features minimal-application,bugreport,build-assets`.

- `lint`, `license_checks`, `documentation`, `cargo-audit`: as described in section 5, plus
  `cargo install cargo-audit --locked` and a step that renders the built man page with
  `man $(find . -name bat.1)` so a broken roff template fails CI visibly.

- `test_with_new_syntaxes_and_themes`: checks out with `submodules: true`, `cargo install`s bat,
  regenerates all binary assets from the 93 submodules via `bash assets/create.sh`, reinstalls,
  runs the normal suite plus the `--ignored` asset tests plus
  `tests/syntax-tests/regression_test.sh`. This catches breakage introduced by upstream syntax
  submodule updates before they ship.

- `test_with_system_config`: sets `BAT_SYSTEM_CONFIG_PREFIX` to a fixture directory and runs the
  two `--ignored` tests in extras/bat/tests/system_wide_config.rs.

- `build`: a 13-target matrix with `fail-fast: false` covering
  x86_64/i686/aarch64/arm on gnu, musl, MSVC (including windows-11-arm), and both macOS
  architectures. ARM and AArch64 Linux targets build via `cross`, which is pinned to a commit:

```yaml
- name: Install cross
  if: matrix.job.use-cross
  run: cargo install cross --git https://github.com/cross-rs/cross --rev 588b3c99db52b5a9c5906fab96cfadcf1bde7863
```

  Each matrix leg also runs the tests (reduced to `--lib --bin bat` on emulated ARM), smoke-runs
  the real binary (`bat --paging=never --color=always ... --diagnostic`), and then `cargo check`s
  five feature combinations (`regex-onig`, `regex-onig,git`, `regex-onig,paging`,
  `regex-onig,git,paging`, `minimal-application`) so the optional-dependency matrix can never
  silently rot on any platform.

  The same job stages release artifacts inline: a tarball or zip containing the binary, README,
  licenses, CHANGELOG, generated man page, and all four shell completions pulled out of the
  build script's OUT_DIR; on Ubuntu legs it additionally assembles a full Debian package
  (control file, gzipped changelog, copyright file) with `fakeroot dpkg-deb`. On tags matching
  `refs/tags/v[0-9].*` the artifacts are attached to the GitHub release via
  `softprops/action-gh-release@v2`.

- `winget`: runs only on version tags and publishes the MSVC zip to Winget using a third-party
  action pinned by full commit SHA:
  `vedantmgoyal9/winget-releaser@19e706d4c9121098010096f9c495a70a7518b30f`.

Notable absences: there is no cargo build caching (correctness and reproducibility are preferred
over speed; every build is `--locked` from a committed Cargo.lock), and no merge queue. Action
pinning is pragmatic: first-party and dtolnay actions by major tag, third-party publishing
actions by SHA.

#### require-changelog-for-PRs.yml

Runs on every PR (skipping dependabot), fetches the PR submitter from the GitHub API, diffs
CHANGELOG.md against the base branch, and greps the added lines for the PR number and the
submitter's handle:

```yaml
run: |
  ADDED=$(git diff -U0 "origin/${PR_BASE}" HEAD -- CHANGELOG.md | grep -P '^\+[^\+].+$')
  grep "#${PR_NUMBER}\\b.*${PR_SUBMITTER}\\b" <<< "$ADDED"
```

This mechanically enforces the changelog format documented in extras/bat/CONTRIBUTING.md
(`- Short description of what has been changed, see #123 (@user)`).

#### Dependabot

extras/bat/.github/dependabot.yml updates three ecosystems monthly on the same schedule: cargo,
gitsubmodule (the 93 syntax/theme submodules), and github-actions. Dependabot PRs are auto-merged
when CI passes, which is exactly why the changelog gate excludes them and the release checklist
reminds maintainers to backfill their entries.

### 7. Testing

The test architecture is layered, all under extras/bat/tests:

- End-to-end CLI tests: extras/bat/tests/integration_tests.rs (4,644 lines, 273 tests) drives
  the compiled binary with `assert_cmd` and `predicates`. Every test goes through a factory in
  extras/bat/tests/utils/command.rs that sanitizes the environment first:

```rust
pub fn bat_raw_command_with_config() -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("bat"));
    cmd.current_dir("tests/examples");
    cmd.env_remove("BAT_CACHE_PATH");
    cmd.env_remove("BAT_CONFIG_PATH");
    cmd.env_remove("BAT_PAGER");
    cmd.env_remove("PAGER");
    cmd.env_remove("NO_COLOR");
    ...
}
```

  Tests that must mutate process-global state (PATH, env vars) are marked `#[serial]` from the
  `serial_test` crate; extras/bat/tests/utils/mocked_pagers.rs temporarily prepends
  tests/mocked-pagers to PATH, verifies the fakes respond ("I am most"), runs the test closure,
  and restores PATH.

- Real-terminal tests: on Unix, integration_tests.rs opens a genuine PTY with
  `nix::pty::openpty` (see the `unix` module at the top of the file, with
  `CHILD_WAIT_TIMEOUT: Duration = Duration::from_secs(15)` and `wait_timeout` guarding hangs) to
  test interactive-output behavior that cannot be observed through pipes.

- Snapshot tests: extras/bat/tests/snapshot_tests.rs generates 26 tests from a declarative
  macro, one per `--style` component combination:

```rust
snapshot_tests! {
    changes:                     "changes",
    grid:                        "grid",
    ...
    changes_grid_header_numbers_rule: "changes,grid,header,numbers,rule",
    full:                        "full",
    plain:                       "plain",
}
```

  The harness in extras/bat/tests/tester/mod.rs builds a real temporary git repository
  programmatically with `gix` (writes a blob, a tree, a commit, then modifies the working copy)
  so the "changes" gutter markers are exercised against genuine git state, then compares stdout
  to committed files under tests/snapshots/output/.

- Golden-file help tests: the `-h` and `--help` outputs are snapshotted with `expect-test`
  against extras/bat/doc/short-help.txt and extras/bat/doc/long-help.txt
  (`expect_test::expect_file![expect_file].assert_eq(...)` in integration_tests.rs around line
  726). Any flag change shows up as a reviewable diff to documentation files.

- Invariant and meta tests: no_duplicate_extensions.rs and github-actions.rs (sections 5 and 6),
  plus extras/bat/tests/assets.rs, an `#[ignore]`d test listing all 26 themes that must be
  present, run in CI only after assets are rebuilt.

- Syntax regression corpus: extras/bat/tests/syntax-tests holds a source/ directory with one
  sample file per language and a highlighted/ directory with the expected ANSI output;
  regression_test.sh regenerates and diffs them via two Python scripts. update.sh re-blesses.

- Unit tests live inline in src modules under `#[cfg(test)]` (for example the detector stubs in
  extras/bat/src/theme.rs, section 8, and the parser tests at the bottom of
  extras/bat/src/less.rs).

- Benchmarks: extras/bat/tests/benchmarks/run-benchmarks.sh uses hyperfine (startup time,
  many-small-files, highlighting throughput), unsets the same env vars as the test factory, and
  writes a markdown report. Performance is measured out-of-band rather than gating CI.

There is no fuzzing or property testing in-repo; the syntax corpus plus the `--ignored`
asset-rebuild jobs fill that role for bat's actual risk surface (upstream syntax updates).

### 8. Error handling and API design

Errors: one public `thiserror` enum in extras/bat/src/error.rs, marked `#[non_exhaustive]` so
new variants are not semver breaks, with `#[error(transparent)] #[from]` wrappers for foreign
errors and feature-gated variants that only exist when the corresponding subsystem is compiled:

```rust
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] ::std::io::Error),
    ...
    #[cfg(feature = "paging")]
    #[error(transparent)]
    MinusError(#[from] ::minus::MinusError),
}

pub type Result<T> = std::result::Result<T, Error>;
```

`From<&'static str>` and `From<String>` fold ad-hoc messages into `Error::Msg`, so internal code
can write `.ok_or("Empty line range")?` (extras/bat/src/line_range.rs). The build script, which
is not public API, uses `anyhow` instead; the boundary between thiserror (library) and anyhow
(application-ish code) is drawn exactly where the textbooks say.

Exit codes and panic policy: the binary's `main` in extras/bat/src/bin/bat/main.rs returns
`Result<bool>` from `run()` and maps it explicitly: error prints via `default_error_handler`
then `process::exit(1)`, `Ok(false)` (some input failed) exits 1, `Ok(true)` exits 0. The
error handler special-cases the one error a well-behaved CLI must swallow:

```rust
Error::Io(ref io_error) if io_error.kind() == ::std::io::ErrorKind::BrokenPipe => {
    ::std::process::exit(0);
}
```

so `bat file | head` never reports a spurious failure.

API design:

- Builder with `&mut Self` chaining: extras/bat/src/pretty_printer.rs exposes `PrettyPrinter`
  whose setters (`input_file`, `language`, `term_width`, ...) return `&mut Self`, with generic
  ergonomic bounds like `pub fn input_files<I, P>(&mut self, paths: I) where I: IntoIterator<Item = P>, P: AsRef<Path>`.
- `#[non_exhaustive]` on public data types (`Syntax` in pretty_printer.rs) reserves the right to
  add fields.
- Visibility discipline: extras/bat/src/lib.rs re-exports a curated surface
  (`pub use pretty_printer::{Input, PrettyPrinter, Syntax}`) while whole modules stay
  `pub(crate)` (`printer`, `syntax_mapping`, `wrapping`, `nonprintable_notation`), and the crate
  doc explicitly warns that the deeper `controller` API "is much more likely to change".
- Testable seams via traits: extras/bat/src/theme.rs defines `ColorSchemeDetector`, implemented
  by `TerminalColorSchemeDetector` in production (with a long comment explaining the OSC 10/11
  race with pagers) and by `DetectorStub`/`ConstantDetector` in the inline tests, `DetectorStub`
  using `Cell<bool>` to record invocation. The public `theme()` function is a thin wrapper over
  `theme_impl(options, &TerminalColorSchemeDetector)`.

### 9. Deep Rust usage: ten cited idioms

1. Build-time code generation with the proc-macro toolchain outside a proc macro.
   extras/bat/build/syntax_mapping.rs deserializes the builtins TOML files, then implements
   `quote::ToTokens` for its domain types and emits a static table, pretty-printed with
   `prettyplease` and included via
   `include!(concat!(env!("OUT_DIR"), "/codegen_static_syntax_mappings.rs"))` in
   extras/bat/src/syntax_mapping/builtin.rs:

   ```rust
   let t = quote! {
    /// Generated by build script from /src/syntax_mapping/builtins/.
    pub(crate) static BUILTIN_MAPPINGS: [(Lazy<Option<GlobMatcher>>, MappingTarget); #len] = [#(#array_items),*];
   };
   ```

2. Lazy statics as a startup-latency strategy. The generated table stores
   `Lazy<Option<GlobMatcher>>` so glob compilation happens on first match, and builtin.rs
   contains a 30-line comment explaining why a cleaner-looking `BuiltinMatcher` enum was tried
   and rejected ("Because there was. I tried it and threw it out."), a model of documenting
   negative design results where the temptation will recur.

3. Interior mutability chosen precisely. extras/bat/src/assets.rs caches the deserialized
   `SyntaxSet` in `once_cell::unsync::OnceCell` (no threading, no atomic cost), while
   extras/bat/src/assets/lazy_theme_set.rs pairs serde with lazy init:

   ```rust
   struct LazyTheme {
    serialized: Vec<u8>,
    #[serde(skip, default = "OnceCell::new")]
    deserialized: OnceCell<syntect::highlighting::Theme>,
   }
   ```

   and loads via `lazy_theme.deserialized.get_or_try_init(|| lazy_theme.deserialize())`.

4. Embedded compressed assets with documented tradeoffs. extras/bat/src/assets.rs embeds
   syntaxes and themes with `include_bytes!("../assets/syntaxes.bin")` behind
   `pub(crate) const COMPRESS_LAZY_THEMES: bool = true;` style constants, each carrying a
   measured justification ("Compress for size of ~40 kB instead of ~200 kB without much
   difference in performance due to lazy-loading").

5. Trait objects with lifetimes for input polymorphism. extras/bat/src/input.rs models sources
   as an enum embedding a borrowed reader:

   ```rust
   pub(crate) enum InputKind<'a> {
    OrdinaryFile(PathBuf),
    StdIn,
    CustomReader(Box<dyn Read + 'a>),
   }
   ```

   letting `PrettyPrinter<'a>` accept byte slices, files, and arbitrary readers uniformly.

6. Small strategy traits instead of match trees. extras/bat/src/decorations.rs defines
   `pub(crate) trait Decoration { fn generate(...); fn width(...); }` with
   `LineNumberDecoration`, `LineChangesDecoration`, and `GridBorderDecoration` impls
   (LineNumberDecoration caches its wrapped-line filler and invalidates at
   `cached_wrap_invalid_at: 10000`); extras/bat/src/printer.rs has `trait Printer` with
   `SimplePrinter` and `InteractivePrinter` implementations selected by config.

7. Conversion traits as API glue. extras/bat/src/assets/lazy_theme_set.rs implements
   `TryFrom<LazyThemeSet> for ThemeSet` and `TryFrom<ThemeSet> for LazyThemeSet` so users can
   add custom themes to the lazily-loaded set; extras/bat/build/syntax_mapping.rs implements
   `FromStr` with `type Err = Infallible` where parsing cannot fail, and derives deserialization
   from it via `serde_with::DeserializeFromStr`.

8. Platform handling in three tiers. Target-conditional dependencies in Cargo.toml (plist on
   macOS, nix on Unix); 128 `#[cfg(...)]` attributes in src including whole-file gates like
   `#![cfg(feature = "git")]` at the top of extras/bat/src/diff.rs; and data-level platform
   splits, where extras/bat/build/syntax_mapping.rs selects builtins subdirectories with inline
   cfg on array elements (`#[cfg(target_family = "unix")] "unix-family",`). Divergent behavior
   gets paired cfg functions: `color_scheme_from_system()` in extras/bat/src/theme.rs has a
   macOS implementation reading `.GlobalPreferences.plist` and a non-macOS one that warns.

9. Declarative macros for both product and tests. extras/bat/src/macros.rs exports
   `bat_warning!` to standardize the yellow "[bat warning]" prefix; the `snapshot_tests!` macro
   in extras/bat/tests/snapshot_tests.rs stamps out one named `#[test]` per style permutation so
   failures name the exact combination.

10. Iterator pipelines over fallible data. extras/bat/build/syntax_mapping.rs walks TOML files
    with `WalkDir` and itertools' `filter_map_ok(...).collect::<Result<Vec<_>, _>>()?`,
    propagating IO errors through the pipeline instead of unwrapping;
    extras/bat/src/preprocessor.rs decodes UTF-8 incrementally with an `Option` combinator
    chain (`input.get(0..1).and_then(str_from_utf8).map(|c| (c, 1)).or_else(...)`) and
    `expand_tabs` copies escape sequences by byte-range slicing (`&line[seq.index_of_start()..
    seq.index_past_end()]`) rather than re-parsing, with a capacity hint
    (`String::with_capacity(line.len() * 2)`).

Two bonus idioms: `wild::args_os()` is used throughout extras/bat/src/bin/bat/app.rs so glob
patterns expand on Windows exactly as a Unix shell would, and the thread-based builtin pager in
extras/bat/src/output.rs holds `handle: Option<JoinHandle<Result<()>>>` so the pager thread's
error is joined and propagated rather than dropped.

### 10. Documentation practices

- The crate doc in extras/bat/src/lib.rs opens with a runnable doctest ("Hello world" through
  `PrettyPrinter`) and an honest stability statement about the internal modules. Public items in
  extras/bat/src/theme.rs show the house rustdoc style: intra-doc links
  (`[`crate::theme::ThemeOptions::theme`]`), doctested constructors, and a `pub mod env` that
  documents environment variable names as constants.
- extras/bat/doc is a real documentation directory: assets.md (how the syntax/theme pipeline
  works, including how to write syntax tests), alternatives.md, release-checklist.md, and four
  translated READMEs (ja, ko, ru, zh). long-help.txt and short-help.txt are not prose, they are
  test fixtures asserted by expect-test, so the docs cannot drift from the binary.
- extras/bat/CONTRIBUTING.md is operational, not ceremonial: it specifies the exact changelog
  entry format that CI greps for, says when an entry is not needed, and states "You are
  **strongly encouraged** to add regression tests" with a pointer to integration_tests.rs.
- Data formats are documented next to the data: extras/bat/src/syntax_mapping/builtins/README.md
  explains the TOML rule schema, file organization, and dynamic env-var replacement.
- extras/bat/.github/ISSUE_TEMPLATE has four templates (bug_report, feature_request, question,
  syntax_request); the bug template preempts the most-reported known issue inline. SECURITY.md
  gives a private disclosure contact. There is no PR template and no ARCHITECTURE.md; the module
  doc in lib.rs and doc/assets.md carry that weight.
- Developer environment is reproducible: extras/bat/flake.nix defines dev shells for four
  systems and extras/bat/.envrc (`use flake`) wires it to direnv.

### 11. Release and distribution

- Versioning: semver in Cargo.toml, released as git tags `vX.Y.Z`. Pushing the tag is the
  release trigger; extras/bat/.github/workflows/CICD.yml detects
  `$GITHUB_REF =~ ^refs/tags/v[0-9].*` and uploads all 13 targets' archives plus Debian packages
  to the GitHub release.
- The process is codified in extras/bat/doc/release-checklist.md as literal checkboxes: bump
  version, re-derive MSRV via `cargo metadata | jq`, reconcile CHANGELOG.md against
  auto-generated release notes (dependabot PRs are auto-merged and therefore missing), rebuild
  binary assets with assets/create.sh, review -h/--help/man, `cargo publish --dry-run`, tag,
  create the GitHub release from the changelog section, verify artifacts, `cargo publish` from a
  clean clone, then reset the "unreleased" changelog skeleton (Features / Bugfixes / Other /
  Syntaxes / Themes / "bat as a library").
- Changelog discipline is enforced by machine on the way in (the changelog workflow, section 6)
  and consumed on the way out (release notes are copied from CHANGELOG.md).
- Man page and completions are build outputs, not hand-maintained artifacts:
  extras/bat/build/application.rs renders extras/bat/assets/manual/bat.1.in and four completion
  templates (bash, fish, zsh, PowerShell) with a tiny variable-substitution engine
  (`PROJECT_NAME`, `PROJECT_EXECUTABLE`, `PROJECT_VERSION`), honoring `BAT_ASSETS_GEN_DIR` so
  packagers can redirect output. The generated files are gitignored
  (extras/bat/.gitignore lists `/assets/manual/bat.1` and the completion outputs) and packaged
  from OUT_DIR by CI into both tarballs and .deb layouts
  (`usr/share/bash-completion/completions/bat`, `usr/share/man/man1/bat.1.gz`).
- Distribution breadth: GitHub release archives for every target, self-built Debian packages
  with correct Provides/Conflicts between `bat` and `bat-musl`, Winget publishing, crates.io for
  the library, plus static CRT linking on Windows and musl builds on Linux so binaries run
  anywhere.

### 12. Lessons for quinjet

quinjet already exceeds bat on static analysis (clippy wall, cargo-deny, taplo, typos, miri,
mutants, coverage floor). What bat adds is CI architecture, end-to-end CLI testing, and release
mechanics. Concrete adoptions:

1. Add an `all-jobs` aggregator job that `needs` every other job with `if: always()` and the
   `jq --exit-status 'all(.result == "success")'` step from extras/bat/.github/workflows/CICD.yml,
   then make it the only required branch-protection check.
2. Copy the meta-test pattern from extras/bat/tests/github-actions.rs: a `#[test]` that parses
   the workflow YAML with `serde_yaml` (dev-dependency) and asserts the aggregator's `needs`
   list matches the set of defined jobs, with an explicit exceptions array.
3. Add a `crate_metadata` CI job that extracts `rust-version` via
   `cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].rust_version'` and feed it
   to an MSRV job using `dtolnay/rust-toolchain@master` with `toolchain: ${{ outputs.msrv }}`,
   so the MSRV lives only in Cargo.toml.
4. Snapshot `quinjet --help` and every subcommand's `--help` into committed `doc/*.txt` files
   using the `expect-test` crate's `expect_file!` (bless with `UPDATE_EXPECT=1`), exactly as
   extras/bat/tests/integration_tests.rs does against doc/long-help.txt. CLI surface changes
   become reviewable diffs.
5. Build the CLI end-to-end suite on `assert_cmd` + `predicates` with a single command factory
   that `env_remove`s every quinjet- and git-relevant variable (`GIT_DIR`, `GIT_CONFIG_*`,
   `HOME`-scoped config) per extras/bat/tests/utils/command.rs, and mark PATH/env-mutating tests
   `#[serial]` with the `serial_test` crate.
6. For TUI behavior that depends on a real terminal, use `nix::pty::openpty` behind
   `[target.'cfg(unix)'.dev-dependencies]` plus `wait-timeout` for hang protection, following
   the `unix` module at the top of extras/bat/tests/integration_tests.rs.
7. Steal the snapshot-permutation macro from extras/bat/tests/snapshot_tests.rs, and its harness
   idea: construct a throwaway git repository programmatically in the test (quinjet can shell
   out to `git init` or use `gix` like extras/bat/tests/tester/mod.rs) so every rendered view is
   tested against real repository state, not mocks.
8. Handle `ErrorKind::BrokenPipe` by exiting 0 in the top-level error handler, per
   extras/bat/src/error.rs `default_error_handler`; a Git CLI whose output feeds `head`, `fzf`,
   or a pager must not report failure on early pipe close.
9. Generate the man page and bash/zsh/fish completions from `build.rs` (quinjet can use
   `clap_mangen`/`clap_complete` rather than bat's templates) and package them in release
   archives under an `autocomplete/` directory as CICD.yml's "Create tarball" step does.
10. Add a release build matrix in the same workflow as PR CI, gated on `refs/tags/v*`:
    linux gnu+musl (via `cross` pinned to a commit), macOS both arches, Windows MSVC with
    `crt-static` rustflags in `.cargo/config.toml`, uploading archives with
    `softprops/action-gh-release@v2`; pin third-party publishing actions by full SHA.
11. Enforce changelog entries mechanically with a 33-line workflow cloned from
    extras/bat/.github/workflows/require-changelog-for-PRs.yml: diff CHANGELOG.md against the
    base branch and grep added lines for `#<PR> ... <submitter>`.
12. Gate rustdoc in CI with `RUSTDOCFLAGS: -D warnings` and
    `cargo doc --locked --no-deps --document-private-items --all-features`
    (extras/bat/.github/workflows/CICD.yml documentation job); this catches broken intra-doc
    links that clippy does not.
13. Add invariant tests in the style of extras/bat/tests/no_duplicate_extensions.rs: assert no
    duplicate clap subcommand names/aliases and no conflicting keybindings, each with a
    documented `KNOWN_EXCEPTIONS` list if any.
14. Add a `hyperfine`-based startup benchmark script like
    extras/bat/tests/benchmarks/run-benchmarks.sh; keyboard-first tools live and die on startup
    latency, and a markdown report per release makes regressions visible without gating CI.
15. Configure dependabot for `cargo` and `github-actions` ecosystems on a monthly schedule
    (extras/bat/.github/dependabot.yml), and write the release process as a checkbox file
    `doc/release-checklist.md` so releases are reproducible by any maintainer.

---

## starship/starship (59420 stars)

### 1. What the project is and measurable scale

Starship is the cross-shell prompt: a single Rust binary that renders a fast, customizable
shell prompt for bash, zsh, fish, PowerShell, nushell, elvish, ion, tcsh, xonsh and cmd.
Industry adoption follows from three properties: it is one static binary with no runtime,
it is configured by one TOML file that behaves identically on every shell and OS, and it is
fast enough to run on every keystroke of a prompt redraw. The package metadata states the
pitch directly in extras/starship/Cargo.toml:

```toml
description = """
The minimal, blazing-fast, and infinitely customizable prompt for any shell! ☄🌌️
"""
```

Scale measured from the clone:

- One crate, no workspace: extras/starship/Cargo.toml is the only Cargo manifest in the
  repository (a `find` for `Cargo.toml` returns exactly one file). Version `1.26.0`,
  edition `2024`, `rust-version = "1.95"`.
- 246 Rust source files totaling 51,327 lines of Rust under extras/starship/src.
- 109 entries in extras/starship/src/modules, one file per prompt module (git status,
  language versions, cloud contexts, battery, and so on), mirrored by a matching config
  struct file per module in extras/starship/src/configs.
- 428 `name =` entries in extras/starship/Cargo.lock, so roughly 427 transitive
  dependencies resolve for the full feature set.
- 1,302 `#[test]` attributes across the tree, organized into 114 inline `mod tests`
  blocks, with 864 call sites of the `ModuleRenderer` test harness.
- A 5,314 line configuration reference at extras/starship/docs/config/README.md and a
  7,253 line generated JSON schema at extras/starship/.github/config-schema.json.

### 2. Repository layout

The real top level of extras/starship:

```text
extras/starship/
|-- build.rs                    build-time codegen (shadow-rs, presets, Windows resources)
|-- Cargo.toml                  the single crate manifest
|-- Cargo.lock                  committed lockfile, enforced with --locked in CI
|-- clippy.toml                 disallowed-methods list
|-- deny.toml                   cargo-deny advisories/licenses/bans/sources policy
|-- typos.toml                  spell-check dictionary extensions
|-- .rustfmt.toml               intentionally blank (see section 4)
|-- .dprint.json                formatter for Markdown, TOML, JSON, TypeScript
|-- .codecov.yml                coverage thresholds
|-- .gitattributes              forced LF line endings for shell scripts
|-- crowdin.yml                 translation pipeline config
|-- release-please-config.json  release automation config
|-- starship.exe.manifest       Windows application manifest, embedded by build.rs
|-- CHANGELOG.md                generated by release-please
|-- CONTRIBUTING.md             architecture notes and testing conventions
|-- docs/                       VitePress docs site plus 25+ translated copies
|-- install/                    install.sh, macOS pkg scripts, Windows choco/wix files
|-- media/                      icons and screenshots
|-- src/                        all Rust code
`-- .github/                    workflows, templates, config-schema.json, renovate.json5
```

Inside src the split is by responsibility, not by layer:

```text
extras/starship/src/
|-- main.rs        clap CLI definition and dispatch
|-- lib.rs         public library surface ("Lib is present to allow for benchmarking")
|-- print.rs       prompt assembly, rayon parallel module rendering, explain/timings
|-- module.rs      Module type, ALL_MODULES registry
|-- segment.rs     styled text segments
|-- config.rs      ModuleConfig trait and TOML deserialization plumbing
|-- configs/       one default-config struct per module (109 files)
|-- modules/       one renderer per module (109 files)
|-- context/       Context: cwd, env, git repo discovery, mocking hooks
|-- formatter/     pest-based format-string engine (spec.pest, string_formatter.rs)
|-- init/          per-shell init scripts embedded with include_str!
|-- test/          ModuleRenderer harness and VCS fixture bundles
`-- utils/         command execution with timeouts, env access, serde helpers
```

The split works because the project is a plugin architecture without dynamic plugins:
every module is a pair of files (renderer in src/modules, config in src/configs) plus one
registry entry in `ALL_MODULES` in extras/starship/src/module.rs. A contributor adding a
language module touches a known, small set of files and copies an existing pair. The
comment at the top of extras/starship/src/modules/mod.rs encodes the convention:

```rust
// While adding out new module add out module to src/module.rs ALL_MODULES const array also.
```

### 3. Cargo manifest practices

extras/starship/Cargo.toml is a model of a well-tended single-crate manifest.

Packaging discipline: an explicit `include` list keeps the crates.io tarball minimal and
documents a subtle detail inline:

```toml
# Keep `/` in front of `README.md` to exclude localized readmes
include = [
  "src/**/*",
  "/starship.exe.manifest",
  ...
  "docs/public/presets/toml/",
  ".github/config-schema.json",
]
```

MSRV is declared and its meaning is scoped honestly:

```toml
# Note: MSRV is only intended as a hint, and only the latest version is officially supported in starship.
rust-version = "1.95"
```

Feature flags exist only where a dependency fails to build somewhere, and each optional
dependency carries a comment explaining exactly why, with an issue link:

```toml
[features]
default = ["battery", "notify"]
battery = ["starship-battery"]
config-schema = ["schemars"]
notify = ["notify-rust"]
```

```toml
# battery is optional (on by default) because the crate doesn't currently build for Termux
# see: https://github.com/svartalf/rust-battery/issues/33
starship-battery = { version = "0.11.1", optional = true }
```

Dependency hygiene worth copying:

- Feature-trimming heavy dependencies: `gix` is pulled with `default-features = false`
  and only `max-performance-safe`, `revision`, `status`, `sha1`, `sha256`; the comment
  cites the issue that motivated the restriction. `regex` drops default features down to
  `perf`, `std`, `unicode-perl`.
- Exact pins where semver trust broke: `systemstat = "=0.2.7"` and the dev-dependency
  `mockall = "=0.15.0"`.
- Rationale comments on non-obvious picks: `parking_lot` is annotated
  `# ... This is for poison-free locks.` and `os_info` carries
  `# update os module config and tests when upgrading os_info`.
- Platform-conditional dependencies: `[target.'cfg(windows)'.dependencies.windows]`
  enumerates exactly five Win32 feature gates; `[target.'cfg(not(windows))'.dependencies]`
  pulls `nix` with just `feature`, `fs`, `user`.

Profiles are tuned for a latency-critical binary:

```toml
[profile.release]
codegen-units = 1
lto = true
strip = true

[profile.bench]
codegen-units = 16
lto = "thin"
strip = false
```

Finally there is a minimal `[lints]` table: `[lints.clippy] use_self = "warn"`. The heavy
lint policy lives elsewhere (section 5).

### 4. Formatting

The most interesting formatting artifact is a file that is deliberately empty.
extras/starship/.rustfmt.toml:

```toml
# This file intentionally left almost blank
#
# The empty `rustfmt.toml` makes rustfmt use the default configuration,
# overriding any which may be found in the contributor's home or parent
# folders.
```

This is a defensive trick: rustfmt walks parent directories and the home directory for
configuration, so an empty repo-level file pins the project to rustfmt defaults no matter
what a contributor has configured globally. CI enforces it with `cargo fmt --all -- --check`
in extras/starship/.github/workflows/workflow.yml.

Non-Rust formatting is delegated to dprint via extras/starship/.dprint.json, which formats
Markdown (line width 100), TOML, JSON and the TypeScript of the docs site, excluding the
generated CHANGELOG.md and all translated docs directories (`docs/??-??/**` patterns).
The dprint plugins are WASM binaries referenced by release URL, and the Renovate config
(section 6) has a custom regex manager to keep those URLs updated.

Shell scripts get their own formatter and linter: the install-script workflow runs
`shfmt -d install/**/*.sh` and `shellcheck --severity=warning install/**/*.sh`
(extras/starship/.github/workflows/install-script.yml). Line endings are pinned at the Git
layer in extras/starship/.gitattributes:

```text
/src/init/* text eol=lf
*.sh text eol=lf
/.github/config-schema.json text eol=lf
```

The init scripts are embedded into the binary with `include_str!`, so a CRLF checkout on
Windows would otherwise corrupt what the binary emits into a user's shell.

### 5. Linting

Starship's clippy philosophy is the opposite of a broad severity wall: run default clippy
at `-D warnings` on three operating systems, and add a small number of surgical,
project-specific bans. The bans live in extras/starship/clippy.toml, each with its reason:

```toml
disallowed-methods = [
  # std::process::Command::new may inadvertly run executables from the current working directory
  "std::process::Command::new",
  # Setting environment variables can cause issues with non-rust code
  "std::env::set_var",
  # use `dunce` to avoid UNC/verbatim paths, where possible
  "std::fs::canonicalize",
]
```

These are real security and correctness policies, not style: on Windows,
`Command::new("git")` can execute `git.exe` from the current directory, which for a
program that runs automatically in every directory a user visits is an attack vector.
Both binary roots opt in at the crate level: extras/starship/src/main.rs and
extras/starship/src/lib.rs each begin with `#![warn(clippy::disallowed_methods)]`.
The one sanctioned call site is the wrapper itself in extras/starship/src/utils/mod.rs,
which first resolves the binary through `which::which` and then suppresses the lint
locally:

```rust
    #[allow(clippy::disallowed_methods)]
    let mut cmd = Command::new(full_path);
    cmd.stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .stdin(Stdio::null());
```

CI runs `cargo clippy --workspace --locked -- -D warnings` on ubuntu, macOS and windows
(extras/starship/.github/workflows/workflow.yml), because a cfg-gated module can lint
clean on Linux and fail on Windows. Complementary check infrastructure: `typos` runs on
every PR with a curated exception list in extras/starship/typos.toml (including the
honest entry `extentions = "extentions" # TODO: should be extensions` for a
config key that is a frozen public misspelling), and `taplo lint` validates every preset
TOML file against the generated JSON schema (section 6).

### 6. CI/CD

There are eight workflows in extras/starship/.github/workflows. Every third-party action
in every workflow is pinned to a full commit SHA with a trailing version comment, for
example `uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1`. That
is supply-chain hardening: tags are mutable, SHAs are not, and the comment keeps the pin
human-readable and Renovate-updatable.

workflow.yml (the main gate) runs on push and pull_request with
`paths-ignore: ["docs/**", "**.md"]` and sets a shared env of
`CARGO_INCREMENTAL: 0`, `CARGO_NET_RETRY: 10`, `RUST_BACKTRACE: short`,
`CARGO_BUILD_WARNINGS: deny`, `RUSTUP_MAX_RETRIES: 10`. Jobs:

- `rustfmt`: format check on stable.
- `clippy`: 3-OS matrix, `-D warnings`, cached with `Swatinem/rust-cache`.
- `cargo_check`: fast compile gate; the two feature-matrix checks
  (`--no-default-features`, `--all-features`) and the test suite all declare
  `needs: cargo_check` so expensive jobs never start on code that does not compile
  (the file comments this: `# First check then run expansive tests`).
- `check_if_config_schema_up_to_date`: a generated-artifact drift check. It regenerates
  the schema and fails with an actionable annotation:

```yaml
      - name: Run | Generate Schema
        run: cargo run --locked --features config-schema -- config-schema > .github/config-schema.json

      - name: Check | Detect Changes
        run: |
          if ! git diff --exit-code .github/config-schema.json; then
            echo "::error file=.github/config-schema.json::config-schema.json is out of date. ..."
            exit 1
          fi
```

- `test`: 3 OS x {stable, nightly} with `fail-fast: false`, Windows adding
  `RUSTFLAGS: -C target-feature=+crt-static`. Tests run under coverage:
  `cargo llvm-cov --all-features --locked --workspace --lcov ... -- --include-ignored`,
  installing Mercurial on macOS and Windows first so the `#[ignore]`d VCS tests can run.
  Nightly breakage is tolerated without losing the stable gate via
  `CARGO_BUILD_WARNINGS: ${{ matrix.rust == 'stable' && 'deny' || 'allow' }}`.
  The same job also smoke-builds the Windows MSI with cargo-wix, exercises the Chocolatey
  packaging script against dummy artifacts, and submits debug binaries to SignPath test
  signing, all with `continue-on-error: true` so packaging drift is visible but not
  blocking. Coverage uploads to Codecov, with thresholds in extras/starship/.codecov.yml
  (`target: auto`, `threshold: 5%` for project and patch, `comment: false`).

security-audit.yml runs cargo-deny only when `**/Cargo.toml` or `**/Cargo.lock` change,
splitting checks into a matrix and making the inherently time-dependent one non-blocking:

```yaml
    # Prevent sudden announcement of a new advisory from failing ci:
    continue-on-error: ${{ matrix.checks == 'advisories' }}
```

format-workflow.yml is path-filtered to docs and config-like files and runs four jobs:
dprint check, `taplo lint --schema "file://${GITHUB_WORKSPACE}/.github/config-schema.json"
docs/public/presets/toml/*.toml` (presets are validated against the schema the code
generates, so a preset can never reference a config key that does not exist), a
`block-crowdin` job that fails any PR touching translated docs unless it comes from the
translation branch, and a full VitePress build. install-script.yml lints, formats and then
actually executes the curl-piped installer on ubuntu and macOS and asserts
`"$HOME/.test-install/starship" --version` works. spell-check.yml runs `crate-ci/typos`.
crowdin-pretranslate.yml is a nightly cron guarded by
`if: github.repository == 'starship/starship'` so forks do not burn failing runs.

Dependency automation is Renovate, configured in extras/starship/.github/renovate.json5
with `config:best-practices` and `security:openssf-scorecard`, automerge for minor
updates, `minimumReleaseAge: '4 days'` (a supply-chain cooldown against hijacked
releases), grouped update PRs per ecosystem (clap, gix, pest, toml, unicode crates), and
three custom regex managers that version-track things Renovate cannot see natively:
dprint WASM plugin URLs, `cargo install --version` pins inside workflows, and the
`ziglang==` pip pin in the release workflow.

### 7. Testing

All tests are inline unit tests: there is no `tests/` directory. Instead the crate ships
a purpose-built harness in extras/starship/src/test/mod.rs that makes an inline test read
like an end-to-end test. The core is `ModuleRenderer`, a builder over the real `Context`:

```rust
    #[test]
    fn folder_with_go_file() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        File::create(dir.path().join("main.go"))?.sync_all()?;

        let actual = ModuleRenderer::new("golang").path(dir.path()).collect();

        let expected = Some(format!("via {}", Color::Cyan.bold().paint("🐹 v1.12.1 ")));
        assert_eq!(expected, actual);
        dir.close()
    }
```

(from extras/starship/src/modules/golang.rs). Three pieces of infrastructure make this
possible:

1. Mock seams live on the production type, gated by `cfg(test)`. `Context` in
   extras/starship/src/context/mod.rs carries `pub env: Env<'a>` plus
   `#[cfg(test)] pub cmd: HashMap<&'a str, Option<CommandOutput>>` and
   `#[cfg(test)] pub root_dir: tempfile::TempDir`. Modules never call `std::env` or spawn
   processes directly; they go through `context.get_env` and `utils::exec_cmd`, and
   `exec_cmd` short-circuits into a mock table in tests
   (extras/starship/src/utils/mod.rs):

   ```rust
    #[cfg(test)]
    if let Some(o) = mock_cmd(&cmd, args) {
        return o;
    }
    internal_exec_cmd(cmd, args, time_limit)
   ```

   `mock_cmd` is a big match of canned outputs (`"go version" => ... "go version
   go1.12.1 linux/amd64\n"`), which is why the golang test above renders `v1.12.1`
   without Go installed.

2. Deterministic VCS fixtures. extras/starship/src/test/fixtures contains
   `git-repo.bundle`, `git-repo-sha256.bundle` and `hg-repo.bundle`; `fixture_repo`
   unbundles them into a `TempDir` per test. Fixture matrices such as
   `COMMON_GIT_PROVIDERS` re-run the same test against classic refs and reftable
   backends, bare and non-bare. A hard-won flake fix is codified as data in
   `TEST_GIT_CONFIG`:

   ```rust
    // Prevent intermittent test failures and ensure that the result of git commands
    // are available during I/O-contentious tests, by having git run `fsync`.
    // This is especially important on Windows.
    ("core.fsync", "all"),
   ```

3. Environment-dependent tests are `#[ignore]`d (39 of them, mostly Mercurial and
   Fossil), and CI installs the tools and runs `--include-ignored`, so local
   `cargo test` stays dependency-free while CI covers everything.

Other testing details: the logger is swapped once per process via `static LOGGER: Once`
and writes to `/dev/null`/`nul`; the battery module defines
`#[cfg_attr(test, automock)] pub trait BatteryInfoProvider` and injects a `mockall` mock
through the `Context` field (extras/starship/src/modules/battery.rs); property-level
assertions appear as doc-adjacent inline tests such as the grapheme-width test in
extras/starship/src/print.rs, which pins emoji-cluster width behavior. Performance is
handled as an observable product feature rather than a benchmark suite: the binary
records a `duration: Duration` per module (extras/starship/src/module.rs) and exposes a
`timings` subcommand that prints a per-module latency table
(extras/starship/src/print.rs, `pub fn timings`), and extras/starship/src/lib.rs notes
`// Lib is present to allow for benchmarking` to explain why a binary crate exports a
library at all.

### 8. Error handling and API design

Starship uses neither `anyhow` nor `thiserror` (neither appears in
extras/starship/Cargo.toml). The design constraint is unusual and drives everything: a
prompt must never fail. The result is a three-tier policy:

- Module tier: everything is `Option`. A module renderer returns
  `Option<Module>`; missing binaries, timeouts and parse failures all become `None`
  plus a log line. `exec_timeout` in extras/starship/src/utils/mod.rs converts every
  failure mode (spawn error, timeout via the `process_control` crate's
  `controlled_with_output().time_limit(...).terminate_for_timeout()`, non-UTF8 output,
  nonzero exit) into a logged `None`, including a user-actionable hint on timeout
  ("You can set command_timeout in your config to a higher value ...").
- Config tier: degrade to defaults, loudly. `ModuleConfig::load` in
  extras/starship/src/config.rs logs `log::warn!("Failed to load config value: {e}")`
  and returns `Self::default()`; unknown keys produce a warning and a second
  deserialization pass that tolerates them, so a typo in `starship.toml` degrades one
  module instead of blanking the prompt.
- Library tier: real error enums where callers can act. The format-string engine defines
  `pub enum StringFormatterError { Custom(String), Parse(Box<PestError<Rule>>) }` with
  manual `Display`, `Error` and `From<String>` impls
  (extras/starship/src/formatter/string_formatter.rs); the pest error is boxed to keep
  the enum small. Fallible repo discovery is stored as
  `OnceLock<Result<GitRepo, Box<gix::discover::Error>>>` on `Context`, again boxed.

Exit codes are deliberate and documented at the call site in extras/starship/src/main.rs:
informational invocations exit 0, argument errors exit 2 with a comment citing clap's
convention (`// clap exits with status 2 on error`), and genuine failures use
`std::process::exit(1)` (configuration editing in extras/starship/src/configure.rs).
Panic policy: `expect`/`unwrap` only where failure means the process cannot do its job at
all (writing the prompt to stdout, initializing the rayon pool), and even stderr writes
are shielded: `// avoid panicking in case of stderr closing` followed by
`let _ = writeln!(stderr, ...)`.

API design: the visibility split in extras/starship/src/lib.rs is exact. `pub mod` for
what the binary and tests need (`config`, `configs`, `context`, `formatter`, `print`,
...), private `mod modules`, `mod segment`, `mod utils` for internals. Builders appear
where construction is combinatorial: `ModuleRenderer` chains `.path().config().env().cmd()`
in tests, and `Properties` is a single `clap::Parser` struct
(extras/starship/src/context/mod.rs) that is simultaneously the CLI argument surface
(`#[clap(short = 's', long = "status")]` and friends) and the programmatic construction
API, so shell flags and test setup can never drift apart.

### 9. Deep Rust usage

Ten-plus concrete idioms, each cited:

1. Lazy per-render caching with `OnceLock` fields. `Context` computes directory listings
   and git discovery at most once per prompt render:
   `dir_contents: OnceLock<Result<DirContents, std::io::Error>>` and
   `git_repo: OnceLock<Result<GitRepo, Box<gix::discover::Error>>>`
   (extras/starship/src/context/mod.rs). Modules that need the data pay for it; modules
   that do not, never trigger it. There are 27 `OnceLock` uses and 35 `LazyLock` uses in
   src overall.

2. Cross-module shared state through a static `parking_lot::Mutex` holding an `Arc`.
   `git_status` and `git_metrics` render in parallel but need the same expensive repo
   scan, so extras/starship/src/modules/git_status.rs keeps
   `static REPO_STATUS: parking_lot::Mutex<Option<(Arc<RepoStatus>, PathBuf)>>` keyed by
   path, returning `Arc::clone` so a holder survives cache trashing. The dependency
   choice is justified in the manifest: parking_lot for poison-free locks.

3. Data parallelism with rayon, bounded deliberately. Modules render via `.par_iter()`
   in extras/starship/src/print.rs, and extras/starship/src/lib.rs caps the pool:
   `available_parallelism().map_or(1, usize::from).min(8)`, overridable through
   `STARSHIP_NUM_THREADS`. A prompt should not oversubscribe a laptop.

4. `Cow<'a, str>` as the default string currency of the formatter. The parsed format AST
   is `FormatElement::Text(Cow<'a, str>)` (extras/starship/src/formatter/model.rs) and
   variable values default to `Self::Plain(Cow::Borrowed(""))`
   (extras/starship/src/formatter/string_formatter.rs), so literal format text is never
   copied; only computed values allocate.

5. Zero-copy config structs borrowing from the TOML document. Every module config is
   lifetime-parameterized over the deserializer input, for example
   `pub struct RustConfig<'a> { pub format: &'a str, pub symbol: &'a str, ... }` with
   `#[serde(default)]` (extras/starship/src/configs/rust.rs), which combined with a
   `Default` impl gives partial-override semantics without owned strings.

6. A blanket trait impl to erase serde boilerplate. `ModuleConfig<'a, E>` gets one
   implementation for every `T: Deserialize<'a> + Default`
   (extras/starship/src/config.rs), which is why 109 config files contain nothing but a
   struct and its defaults. The same file shows newtype-driven deserialization:
   `VecOr<T>` accepts scalar-or-list TOML via an untagged `Either<A, B>` enum.

7. Extension traits over `AsRef<str>` for domain measurement. Terminal width is
   grapheme-cluster width, not char count, so extras/starship/src/print.rs defines
   `trait UnicodeWidthGraphemes` with a blanket `impl<T> ... where T: AsRef<str>`, backed
   by the newtype `pub struct Grapheme<'a>(pub &'a str)`, and pins the behavior with an
   inline test asserting a family emoji has width 2.

8. Iterator pipelines in place of manual loops. `FillSegment::ansi_string` fills
   remaining width by cycling a pattern: `.graphemes(true).cycle().scan(0usize, |len, g|
   { *len += Grapheme(g).width(); if *len <= w { Some(g) } else { None } })`
   (extras/starship/src/segment.rs). Session ids are
   `rand::rng().sample_iter(rand::distr::Alphanumeric).take(16)` in
   extras/starship/src/main.rs.

9. A pest PEG grammar as the format-string engine. extras/starship/src/formatter/spec.pest
   defines variables, escapes, text groups and conditionals as a commented grammar file,
   compiled by `pest_derive`; errors surface as `Box<PestError<Rule>>`. Parsing config
   syntax with a real grammar instead of regexes keeps escaping rules exact.

10. Build-time code generation with `shadow_rs` hooks. extras/starship/build.rs registers
    `gen_presets_hook`, which scans docs/public/presets/toml, emits a generated
    `get_preset_content` match of `include_str!` arms, and prints
    `cargo:rerun-if-changed=docs/public/presets/toml`. Presets are docs and embedded data
    from one source of truth; `shadow!` also feeds `CLAP_LONG_VERSION` build metadata
    into `--version` (extras/starship/src/main.rs).

11. Contained unsafe with RAII cleanup. The only `unsafe` in the tree is
    extras/starship/src/modules/utils/directory_win.rs, wrapping Win32 ACL checks; the
    raw handle is a newtype whose `Drop` calls `CloseHandle` (line 25), so every unsafe
    resource has a safe owner. Platform splits otherwise happen at the Cargo level
    (`[target.'cfg(windows)'.dependencies]`) and via `#[cfg(windows)]` blocks in
    extras/starship/build.rs.

12. Modern std combinators and let-else for terse fallible flows:
    `is_some_and(|p| p.len() == 1 && p[0].is_empty())` while normalizing pipestatus in
    extras/starship/src/context/mod.rs, `is_none_or` in the git-status cache check, and
    `let Ok(log_files) = fs::read_dir(log_dir) else { return; }` in the log cleanup of
    extras/starship/src/logger.rs, commented "Avoid noisily handling errors in this
    cleanup function."

13. `PhantomData` to keep an API stable across feature combinations. `Context<'a>` ends
    with `_marker: PhantomData<&'a ()>` documented as
    `/// Avoid issues with unused lifetimes when features are disabled`
    (extras/starship/src/context/mod.rs): with `battery` off, no field would use `'a`,
    and the type would stop compiling.

### 10. Documentation practices

The repository treats documentation as a build artifact with its own CI. The VitePress
site lives in extras/starship/docs (config at docs/.vitepress/config.mts) and is built on
every relevant PR by the `vitepress` job in
extras/starship/.github/workflows/format-workflow.yml, then deployed to Netlify by
extras/starship/.github/workflows/publish-docs.yml on `workflow_dispatch`. Translations
are first-class: 25+ locale directories (docs/ja-JP, docs/zh-CN, ...) are machine-managed
through Crowdin (extras/starship/crowdin.yml, nightly pretranslation workflow), and the
`block-crowdin` CI job rejects direct edits to translated files so the pipeline is the
only writer.

The configuration reference (extras/starship/docs/config/README.md, 5,314 lines)
documents every option of every module, and its machine twin, the schemars-generated
extras/starship/.github/config-schema.json, is kept honest by the CI drift check and
reused to validate presets. Editors get completion for `starship.toml` from the same
schema.

extras/starship/CONTRIBUTING.md (309 lines) is unusually operational: it defines a
glossary (module, segment), states the performance philosophy, sketches the architecture
starting from main.rs, and then teaches the house testing style with full code samples of
`context.get_env`, `context.exec_cmd` and `ModuleRenderer`, so contributors learn the
mock seams before writing a module. Issue templates
(extras/starship/.github/ISSUE_TEMPLATE/Bug_report.md) push users toward
`starship bug-report`, a subcommand (extras/starship/src/bug_report.rs) that pre-fills a
GitHub issue with version, config and environment. The PR template
(extras/starship/.github/PULL_REQUEST_TEMPLATE.md) asks for conventional-commit-typed
titles (they become the changelog) and includes a per-OS testing checklist. Rustdoc is
used pragmatically rather than exhaustively: public seams carry doc comments (the
`StringFormatter::map` contract in extras/starship/src/formatter/string_formatter.rs
documents the three-state `None` / `Some(Err)` / `Some(Ok)` protocol), while internal
modules rely on precise names.

### 11. Release and distribution

Versioning and changelog are fully automated around conventional commits.
extras/starship/.github/workflows/release.yml runs on every push to main:
`googleapis/release-please-action` (release-type `rust`) maintains a rolling release PR;
merging it bumps Cargo.toml, tags, and regenerates extras/starship/CHANGELOG.md with
scope-grouped entries linking commit and PR
(`* **git:** enable sha256 support (#7531)`). Releases are created as drafts
(extras/starship/release-please-config.json sets `"draft": true`) and only published by
`gh release edit ... --draft=false` after all binaries exist, so users never see a
half-populated release.

The build matrix covers 13 targets: glibc and musl Linux on x86_64/i686/aarch64/arm,
riscv64gc via `cargo-zigbuild` (with a pinned `pip install ziglang==0.16.0`), macOS
x86_64/aarch64, three Windows MSVC targets with `-C target-feature=+crt-static`, and a
FreeBSD cross build. Windows binaries and MSI installers are signed through SignPath;
macOS binaries are signed and notarized in a dedicated job that builds a temporary
keychain from secrets, runs `xcrun notarytool`, and deletes the keychain in an
`if: always()` cleanup step. Every artifact gets an `openssl dgst -sha256` checksum file.
Publication to crates.io uses OIDC trusted publishing rather than a long-lived token:
the `cargo_publish` job requests `permissions: id-token: write` and exchanges it via
`rust-lang/crates-io-auth-action` for a short-lived token. Downstream package managers
are updated in the same workflow: Homebrew formula bump, winget manifest via
`wingetcreate`, and Chocolatey via a checked-in PowerShell script that CI also
smoke-tests on ordinary pushes.

As a CLI, distribution polish is in-product: `starship completions <shell>` generates
completions through `clap_complete` plus `clap_complete_nushell`
(extras/starship/src/main.rs), `starship init <shell>` emits the embedded per-shell hook
scripts from extras/starship/src/init, `starship preset -o file` writes any of the 12
bundled presets, and the curl-pipe installer at extras/starship/install/install.sh is
linted, formatted and executed in CI before it ever reaches a user.

### 12. Lessons for quinjet

Quinjet already has the strict-wall side covered (clippy all+pedantic+nursery+cargo plus
restrictions, rustfmt, cargo-deny, taplo, typos, coverage floor, miri, mutants). What
starship adds is mostly seams, fixtures and automation:

1. Adopt `disallowed-methods` in a clippy.toml with reasons per entry. For a Git tool the
   starship set transfers almost verbatim: ban `std::process::Command::new` (route every
   git spawn through one wrapper that resolves the binary with the `which` crate first),
   ban `std::env::set_var`, ban `std::fs::canonicalize` in favor of `dunce::canonicalize`
   for Windows path sanity. Enforce with `#![warn(clippy::disallowed_methods)]` at the
   crate root and one `#[allow(clippy::disallowed_methods)]` inside the wrapper, as in
   extras/starship/src/utils/mod.rs.

2. Wrap external git invocations in an `exec_timeout` built on the `process_control`
   crate (`controlled_with_output().time_limit(...).terminate_for_timeout().wait()`), so
   a hung hook or credential helper can never freeze the TUI; copy the logged-`None`
   degradation contract from extras/starship/src/utils/mod.rs.

3. Build a `ModuleRenderer`-style harness: put `#[cfg(test)]` mock fields (env map,
   command-output map, temp root dir) directly on quinjet's context/app-state type, make
   production code read env and spawn processes only through it, and expose a chainable
   builder in a `src/test/mod.rs`. This is what lets starship keep 1,302 tests inline
   with zero test-only production branches beyond the seams.

4. Ship deterministic repo fixtures as `git bundle` files (like
   extras/starship/src/test/fixtures/git-repo.bundle) unbundled into a `TempDir` per
   test, run each scenario across a provider matrix (reftable/non-reftable, bare/non-bare
   as in `COMMON_GIT_PROVIDERS`), and set `core.fsync=all`, `commit.gpgsign=false` and a
   dummy identity via a shared `TEST_GIT_CONFIG` table to kill flakes, especially on
   Windows.

5. Add a generated-artifact drift check to CI: run
   `cargo run -- <subcommand that emits schema/docs/completions> > checked-in-file` then
   `git diff --exit-code checked-in-file` with a `::error file=...::` annotation telling
   the contributor the exact regeneration command
   (extras/starship/.github/workflows/workflow.yml,
   `check_if_config_schema_up_to_date`). Apply it to quinjet's CLI reference docs and, if
   a config schema exists, to a schemars-generated JSON schema behind an optional
   `config-schema` feature (`schemars` with `preserve_order`).

6. Pin every GitHub Action to a full commit SHA with a `# vX.Y.Z` comment, and add
   Renovate (extras/starship/.github/renovate.json5) with `config:best-practices`,
   `minimumReleaseAge: '4 days'`, grouped crate updates, and weekly action-digest bumps
   so the pins stay fresh without PR noise.

7. Split cargo-deny in CI as starship does
   (extras/starship/.github/workflows/security-audit.yml): a matrix of `advisories` vs
   `bans licenses sources`, with `continue-on-error` only on the advisories leg so a
   newly published RUSTSEC advisory cannot redden unrelated PRs, and path-filter the
   workflow to `**/Cargo.toml` and `**/Cargo.lock`.

8. Add `cargo check --locked --no-default-features` and `--all-features` jobs gated by a
   fast `cargo_check` via `needs:`, and run clippy and tests on a 3-OS matrix; a
   crossterm TUI has enough `cfg(windows)` surface that Linux-only linting will
   eventually lie. Include a nightly test leg with warnings demoted
   (`CARGO_BUILD_WARNINGS: ${{ matrix.rust == 'stable' && 'deny' || 'allow' }}`).

9. Automate releases with release-please (`googleapis/release-please-action`,
   release-type `rust`, `"draft": true` in release-please-config.json), publish binaries
   for a target matrix with `--locked`, attach `sha256` files, and publish to crates.io
   through OIDC trusted publishing (`permissions: id-token: write` plus
   `rust-lang/crates-io-auth-action`) instead of a stored token.

10. Steal the product-level ergonomics: a `completions <shell>` subcommand via
    `clap_complete` (+ `clap_complete_nushell`), build metadata in `--version` via
    `shadow-rs` in build.rs, a `bug-report` subcommand that pre-fills an issue with
    version and environment, and a `timings`-style self-profiling subcommand that prints
    per-operation durations, backed by a `Duration` recorded on each rendered component
    as in extras/starship/src/module.rs.

11. Two small hygiene wins: an intentionally empty `.rustfmt.toml`-style guard is
    unnecessary for quinjet (it has real rustfmt config, which shields it the same way),
    but `.gitattributes` entries forcing `eol=lf` on any file embedded via
    `include_str!` (extras/starship/.gitattributes) and a `.codecov.yml` with
    `comment: false` plus explicit project/patch thresholds are directly copyable.

12. Profile tuning: mirror starship's release profile (`codegen-units = 1`, `lto = true`,
    `strip = true`) for the shipped binary; startup latency and binary size matter for a
    keyboard-first tool exactly as they do for a prompt.

---

## meilisearch/meilisearch (58979 stars)

### 1. What the project is and how big it is

Meilisearch is a search engine shipped as a single HTTP server binary. Industry uses it as a
self-hostable, typo-tolerant, millisecond-latency alternative to Elasticsearch for product and
site search, and increasingly for hybrid keyword-plus-vector search. The binary embeds an
actix-web HTTP layer, an LMDB-backed storage engine (`milli`), a task scheduler, an
authentication store, and a bundled web dashboard.

Scale indicators measured directly from the clone at `extras/meilisearch`:

- 707 Rust source files under `extras/meilisearch/crates` and
  `extras/meilisearch/external-crates`, totaling roughly 261,000 raw lines.
- 23 workspace members: 21 first-party crates under `extras/meilisearch/crates` plus two
  vendored crates under `extras/meilisearch/external-crates` (`async-openai` and
  `async-openai-macros`, forked so they can depend on the in-tree `http-client` and `routes`
  crates, as seen in `extras/meilisearch/external-crates/async-openai/Cargo.toml`).
- Workspace version `1.53.1` in `extras/meilisearch/Cargo.toml`, on edition 2021, with the
  toolchain pinned to Rust 1.91.1 in `extras/meilisearch/rust-toolchain.toml`.
- 16 CI workflow files in `extras/meilisearch/.github/workflows`.

### 2. Repository layout

```text
extras/meilisearch/
|-- Cargo.toml               workspace root: members, shared metadata, profiles
|-- rust-toolchain.toml      pinned channel 1.91.1 + clippy component
|-- clippy.toml              disallowed-methods configuration
|-- .rustfmt.toml            four-line formatting policy
|-- .cargo/config.toml       the `cargo xtask` alias
|-- config.toml              annotated default runtime configuration for the server
|-- Dockerfile               two-stage alpine build
|-- download-latest.sh       curl-installable binary fetcher
|-- crates/                  21 first-party crates
|-- external-crates/         vendored forks (async-openai, reqwest-eventsource)
|-- workloads/               JSON benchmark workloads + workloads/tests declarative tests
|-- documentation/           internal process docs (release, versioning, prototypes)
|-- .github/                 workflows, scripts, templates, dependabot
|-- BENCHMARKS.md, TESTING.md, PROFILING.md, CONTRIBUTING.md, SECURITY.md
```

The crate split under `extras/meilisearch/crates` maps cleanly to runtime layers:

```text
crates/
|-- meilisearch         the HTTP binary: routes, options, analytics, tests
|-- meilitool           offline maintenance CLI (dump export, offline upgrade)
|-- meilisearch-types   shared API types, error codes, settings DTOs
|-- meilisearch-auth    API key store
|-- index-scheduler     task queue, batching, scheduler state machine
|-- milli               the core indexing and search engine (largest crate)
|-- filter-parser       nom parser for the filter DSL
|-- flatten-serde-json  JSON flattening (own README used as crate docs)
|-- json-depth-checker  fast depth probing for JSON values
|-- permissive-json-pointer  JSON pointer selection
|-- dump / file-store   dump import-export and update file storage
|-- meili-snap          in-house snapshot-testing layer over insta
|-- fuzzers             long-running in-tree fuzzer binaries
|-- tracing-trace       span-level profiling transport
|-- xtask               workspace automation (bench, declarative tests, tags)
|-- build-info          vergen-based git metadata as a library
|-- openapi-generator / routes / routes-macros / http-client
```

Why this split works: the boundary between `milli` (pure engine, no HTTP) and `meilisearch`
(HTTP surface) lets the engine be tested and benchmarked without a server, while small
leaf crates like `filter-parser` and `flatten-serde-json` get their own fuzz targets and
criterion benches. Test-only machinery (`meili-snap`), automation (`xtask`), and build
metadata (`build-info`) are real crates, not scripts, so they are compiled and type-checked
on every CI run.

### 3. Cargo manifest practices

`extras/meilisearch/Cargo.toml` uses `[workspace.package]` for shared metadata, and every
member inherits it:

```toml
[workspace.package]
version = "1.53.1"
authors = [
    "Quentin de Quelen <quentin@dequelen.me>",
    "Clément Renault <clement@meilisearch.com>",
]
description = "Meilisearch HTTP server"
edition = "2021"
license = "MIT"
```

Member manifests such as `extras/meilisearch/crates/meilisearch/Cargo.toml` start with
`publish = false` and a block of `key.workspace = true` lines. A deliberate oddity: the
workspace centralizes almost no dependency versions. `[workspace.dependencies]` contains
exactly one entry, `mimalloc`, because the allocator must be byte-identical across crates;
everything else is declared per crate with explicit versions and pruned features
(`default-features = false` appears throughout, for example `actix-web`, `rustls`, `sysinfo`,
`grenad`, `heed` in the manifests above). Version drift is accepted in exchange for
per-crate independence.

Profiles in `extras/meilisearch/Cargo.toml` are tuned per package:

```toml
[profile.release]
codegen-units = 1

[profile.release-with-debug]
inherits = "release"
debug = true

# We now compile heed without the NDEBUG define for better performance.
# However, we still enable debug assertions for a better detection of
# disk corruption on the cloud or in OSS.
[profile.release.package.heed]
debug-assertions = true

[profile.dev.package.flate2]
opt-level = 3
```

`grenad`, `roaring`, and `gemm-f16` also get `opt-level = 3` in dev so tests over compressed
and bitmap data stay fast. There is no `rust-version` key; the MSRV story is instead the hard
pin in `extras/meilisearch/rust-toolchain.toml` (`channel = "1.91.1"`), which every CI job
mirrors exactly.

Feature flags in `extras/meilisearch/crates/meilisearch/Cargo.toml` model product
dimensions: one feature per optional tokenizer language (`chinese`, `japanese`, `hebrew`,
`swedish-recomposition`, ...), each just forwarding to `meilisearch-types/<lang>`, plus
`mini-dashboard` (which gates eight optional build-dependencies), `swagger`, `test-ollama`
(a test-only feature), and `enterprise`. The dashboard assets are declared as data in the
manifest and verified in `build.rs`:

```toml
[package.metadata.mini-dashboard]
assets-url = "https://github.com/meilisearch/mini-dashboard/releases/download/v0.4.2/build.zip"
sha1 = "6a8a76d9ed79357959bc6d4e3816f753a605a27b"
```

`extras/meilisearch/crates/meilisearch/build.rs` downloads that zip, checks the sha1, and
caches it under `OUT_DIR`. Another manifest detail worth copying: a pinned dev-dependency
with the reason attached, in the same file:

```toml
# fixed version due to format breakages in v1.40
insta = { version = "=1.39.0", features = ["redactions"] }
```

No `[lints]` tables exist anywhere; lint policy lives in CI flags and crate attributes
(section 5). `extras/meilisearch/crates/milli/Cargo.toml` shows a migration trace:
`edition = "2021"` is set explicitly with `# edition.workspace = true` commented out.

### 4. Formatting

`extras/meilisearch/.rustfmt.toml` is four lines:

```toml
unstable_features = true

use_small_heuristics = "max"
imports_granularity = "Module"
group_imports = "StdExternalCrate"
```

- `use_small_heuristics = "max"` makes rustfmt keep any construct on one line as long as it
  fits in the width limit, producing dense struct literals and match arms; you can see the
  effect in one-liners like `Server { service, _dir: Some(dir), _marker: PhantomData }` in
  `extras/meilisearch/crates/meilisearch/tests/common/server.rs`.
- `imports_granularity = "Module"` merges imports per module path, one `use` per module.
- `group_imports = "StdExternalCrate"` enforces the visible three-block import order (std,
  external, crate) at the top of every file, for example
  `extras/meilisearch/crates/milli/src/index.rs`.
- `unstable_features = true` opts into the two nightly-gated options above; the CI `fmt` job
  in `extras/meilisearch/.github/workflows/test-suite.yml` runs `cargo fmt --all -- --check`
  on the pinned stable toolchain.

There is no `.editorconfig` and no formatter for YAML, TOML, or Markdown; only Rust is
machine-formatted. `extras/meilisearch/.gitattributes` marks `Cargo.lock -linguist-generated`
so lockfile churn collapses in review.

### 5. Linting

There is no `[lints]` table and no crate-wide `#![deny]` wall. The policy has three layers:

1. Warnings are errors globally in CI via an environment variable, not attributes.
   `extras/meilisearch/.github/workflows/test-suite.yml` sets `RUSTFLAGS: "-D warnings"` for
   every job, so plain `cargo build` and `cargo test` also fail on warnings.
2. Clippy runs at default lint levels, escalated on the command line:

   ```yaml
   - name: Run cargo clippy
     run: cargo clippy --all-targets ${{ matrix.features }} -- --deny warnings -D clippy::todo
   ```

   `-D clippy::todo` is the one targeted escalation: `todo!()` cannot reach `main`.
3. Targeted, machine-checked API bans in `extras/meilisearch/clippy.toml`:

   ```toml
   disallowed-methods = [
       { path = "tar::Archive::unpack", reason = "prefer using the ArchiveExt::safe_unpack function" }
   ]
   ```

   The one sanctioned call site in
   `extras/meilisearch/crates/meilisearch-types/src/archive_ext.rs` carries
   `#[allow(clippy::disallowed_methods)]` with the comment "This is the only place where we
   use `unpack` directly and we do the verification just after", and then checks that no
   unpacked symlink escapes the destination directory. This is lint configuration used as a
   security control: the safe wrapper is the only way to unpack a tarball.

Allows are scarce, top-of-crate, and always about accepted trade-offs rather than silencing:
`#![allow(clippy::result_large_err)]` in
`extras/meilisearch/crates/meilisearch/src/lib.rs`,
`extras/meilisearch/crates/meilisearch-types/src/lib.rs`,
`extras/meilisearch/crates/index-scheduler/src/lib.rs`, and
`extras/meilisearch/crates/meilitool/src/main.rs` (their error enums are deliberately rich),
and `#![allow(clippy::type_complexity)]` in `extras/meilisearch/crates/milli/src/lib.rs`.
The philosophy: keep the default lint set, make it blocking, and reserve custom machinery
for bans that encode a real invariant. Custom check infrastructure beyond clippy lives in
the `openapi-generator` crate, which CI runs with `--check-summaries`,
`--check-descriptions`, `--check-paths`, `--check-docs`, and `--check-params`
(`extras/meilisearch/.github/workflows/check-openapi-file.yml`): self-written consistency
lints for the documented API surface, plus third-party `spectral` linting against
`crates/openapi-generator/.spectral.yaml`.

### 6. CI/CD

All 16 workflows live in `extras/meilisearch/.github/workflows`. Cross-cutting habits:

- Every third-party action is pinned to a full commit SHA with a human-readable comment,
  for example `uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803 # v6` and
  `uses: dtolnay/rust-toolchain@38ae5351029910ad7674ccfad89c37cbd636f3c4 # 1.91.1`
  throughout `test-suite.yml`. `extras/meilisearch/.github/dependabot.yml` keeps only the
  `github-actions` ecosystem updated, monthly, with a 7-day cooldown and
  `rebase-strategy: disabled`.
- Caching is `Swatinem/rust-cache` keyed by the feature matrix
  (`key: ${{ matrix.features }}`), and Linux jobs reclaim disk first by deleting the
  preinstalled GHC, dotnet, Android, and boost trees.
- Workflows that only need to read set `permissions: contents: read` (release assets,
  OpenAPI check) or `permissions: {}` (docker build job).

`test-suite.yml` is the PR gate. Triggers are `pull_request`, `merge_group` (the repository
uses the GitHub merge queue; `extras/meilisearch/README.md` even carries a "Merge Queues
enabled" badge), a daily 5am cron, and `workflow_dispatch`. The job graph makes the expensive
tail conditional:

- `test-linux`: matrix over `ubuntu-22.04` and `ubuntu-22.04-arm`, each with and without
  `--features enterprise`; builds `--no-default-features` first, then tests `--locked --all`.
- `test-windows`: runs on PRs but `if: github.event_name != 'merge_group'`.
- `test-macos` and `test-all-features`: only on schedule or manual dispatch, so macOS cost
  and the near-full feature build (`cargo xtask list-features --exclude-feature
  cuda,test-ollama` feeding `cargo test --features "$(...)"`) never slow a PR.
- `ollama-ubuntu`: installs a real Ollama server, pulls two embedding models, and runs the
  `test-ollama` feature tests against it.
- `test-legacy-indexer`: the whole suite again under
  `MEILI_EXPERIMENTAL_NO_EDITION_2024_FOR_SETTINGS: "true"`, exercising a compatibility
  code path.
- `test-disabled-tokenization`: asserts with `cargo tree -f '{p} {f}' -e normal
  --no-default-features | grep -qz lindera` that a heavy tokenizer dependency cannot leak
  into the default graph. Dependency-graph shape is treated as a testable invariant.
- `declarative-tests`: `cargo xtask test workloads/tests/*.json` (section 7).
- `clippy` and `fmt` jobs as described above, plus a `build` job compiling release mode.

Merge-quality workflows: `db-change-missing.yml` fails any PR that carries neither the
`db change` nor the `no db change` label, and `db-change-comments.yml` posts a canned
checklist (forward-compatibility proof, dumpless-upgrade declarative test) when `db change`
is applied. Benchmarks run from PR comments: `bench-pr.yml` triggers on
`issue_comment` bodies starting with `/bench`, and before running anything it verifies that
both the PR author and the comment author have push permission, that the PR is not from a
fork, and that the head commit was pushed before the comment was written (defeating a
push-after-approval race). Scheduled hygiene: `flaky-tests.yml` runs `cargo flaky -i 100
--release` nightly across four crates, `fuzzer-indexing.yml` runs the in-tree fuzzer for up
to 72 hours (`timeout-minutes: 4320`) on every push to main, `sdks-tests.yml` tests the
nightly Docker image against every official SDK daily, and `dependency-issue.yml` files an
"Upgrade dependencies" issue every six months from a template in
`extras/meilisearch/.github/templates`.

### 7. Testing

Four distinct test tiers exist, each with its own infrastructure.

Unit tests live next to code, and `milli` has an in-crate harness:
`extras/meilisearch/crates/milli/src/test_index.rs` defines `pub(crate) struct TempIndex`
wrapping a temporary LMDB environment with `Deref` to `Index`, and
`extras/meilisearch/crates/milli/src/snapshot_tests.rs` renders databases to text for
snapshotting. Committed snapshot directories sit beside the modules that own them, for
example `extras/meilisearch/crates/milli/src/search/facet/snapshots` and
`extras/meilisearch/crates/index-scheduler/src/scheduler/snapshots`.

Integration tests drive the real HTTP surface.
`extras/meilisearch/crates/meilisearch/tests` has one directory per API area (`auth`,
`documents`, `search`, `settings`, `tasks`, `vector`, ...) and a `common` module whose
`Server` type boots the actual `setup_meilisearch` entry point inside a `TempDir`. The
harness uses a typestate to distinguish throwaway servers from process-wide shared ones
(`extras/meilisearch/crates/meilisearch/tests/common/server.rs`):

```rust
pub struct Server<State = Owned> {
    pub service: Service,
    // hold ownership to the tempdir while we use the server instance.
    _dir: Option<TempDir>,
    _marker: PhantomData<State>,
}
```

with `pub enum Shared {}` and `pub enum Owned {}` as uninhabited marker types in
`extras/meilisearch/tests/common/mod.rs` (path
`extras/meilisearch/crates/meilisearch/tests/common/mod.rs`); mutating helpers are only
implemented for `Server<Owned>`, so a test cannot corrupt the shared fixture server by
construction. The same file wraps `serde_json::Value` in a newtype with `#[track_caller]`
assertion helpers (`succeeded()`, `batch_uid()`) so failures point at the test line.

Snapshot testing is industrialized in the dedicated crate
`extras/meilisearch/crates/meili-snap`. Its `snapshot!` and `snapshot_hash!` macros wrap
insta: `snapshot_hash!` stores only an inline md5 hash in the source while writing the full
snapshot to disk when `MEILI_TEST_FULL_SNAPS=true`, keeping thousands of assertions cheap in
the repository. The helper derives the snapshot path from
`std::panic::Location::caller()` and the test function name (captured with the
`type_name_of_val` trick inside the macro), and installs dynamic redactions that rewrite
UUIDs in messages and JSON keys to `[uuid]` so snapshots are stable across runs
(`extras/meilisearch/crates/meili-snap/src/lib.rs`).

Declarative tests are the fourth tier, documented in `extras/meilisearch/TESTING.md`:
JSON files under `extras/meilisearch/workloads/tests` describe a binary to download
("source": "release", a version, an edition), a chain of HTTP commands, and expected
responses; `cargo xtask test` executes them, which is how dumpless upgrades from released
versions are verified against the working tree.

Fuzzing exists in two forms: classic cargo-fuzz targets in
`extras/meilisearch/crates/filter-parser/fuzz`, `crates/milli/fuzz`,
`crates/flatten-serde-json/fuzz`, and `crates/json-depth-checker/fuzz` (the filter-parser
target panics only on `ErrorKind::InternalError`, treating user-facing parse errors as
success), and the stateful `extras/meilisearch/crates/fuzzers` crate whose `fuzz-indexing`
binary CI runs for up to three days. Benchmarks: criterion harnesses in
`extras/meilisearch/crates/flatten-serde-json/Cargo.toml` (`[[bench]] harness = false`) and
`crates/json-depth-checker`, plus the workload-based end-to-end system described in
`extras/meilisearch/BENCHMARKS.md`, which measures tracing spans from a live server and
uploads results to a dashboard.

### 8. Error handling and API design

The library crates use `thiserror` 2.x exclusively (declared in seven manifests, for example
`extras/meilisearch/crates/milli/Cargo.toml`); `anyhow` appears only at binary and
automation boundaries (`meilisearch`'s `main`, `xtask`, build scripts). `milli`'s root error
in `extras/meilisearch/crates/milli/src/error.rs` splits by audience, not by module:

```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("internal: {0}.")]
    InternalError(#[from] InternalError),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    UserError(#[from] UserError),
}
```

`InternalError` variants indicate engine bugs; `UserError` variants are for humans. One
layer up, `extras/meilisearch/crates/meilisearch-types/src/error.rs` turns any error into a
stable HTTP contract: a `make_error_codes!` declarative macro generates the `Code` enum, an
HTTP status, a snake_case name, and a documentation URL per code, plus one marker unit type
per code so deserialization errors can be typed as `DeserrJsonError<MyErrorCode>`. A single
blanket impl bridges the two worlds:

```rust
impl<T> From<T> for ResponseError
where
    T: std::error::Error + ErrorCode,
{
    fn from(other: T) -> Self {
        Self::from_msg(other.to_string(), other.error_code())
    }
}
```

Panic policy: the server installs a hook that routes panics into structured logs
(`fn on_panic(info: &std::panic::PanicHookInfo)` in
`extras/meilisearch/crates/meilisearch/src/main.rs`), and `main` delegates to
`try_main(...).await.inspect_err(...)`, walking `error.source()` to log the full causal
chain before exiting nonzero through `anyhow::Result`. Inside the engine, panics in worker
threads are captured as values: `InternalError::PanicInThreadPool(#[from] CaughtPanic)`
backed by `extras/meilisearch/crates/milli/src/thread_pool_no_abort.rs`.

Configuration API design in `extras/meilisearch/crates/meilisearch/src/option.rs` is a
model for CLI servers: every flag is one struct field with doc comment (which becomes
`--help` text), a `#[clap(long, env = MEILI_...)]` attribute using a named constant for the
env var, and serde attributes for the TOML config file. `Opt::try_build` implements
precedence "toml < env vars < cli args" by parsing once, reading the config file, injecting
its values into the environment via `export_to_env`, and parsing again. Small string enums
get hand-written `FromStr` with error types that enumerate valid values
(`LogModeError` reports "Unsupported log mode level {0}. Supported values are HUMAN and
JSON."). Visibility discipline shows in `extras/meilisearch/crates/milli/src/index.rs`:
the LMDB `env` and untyped `main` database are `pub(crate)`, while each typed database is
`pub` with a doc comment; the tri-state `Setting<T>` enum (`Set`, `Reset`, `NotSet`) in
`extras/meilisearch/crates/milli/src/update/settings.rs` distinguishes "reset to default"
from "leave untouched" across the settings API instead of overloading `Option`.

### 9. Deep Rust usage: ten-plus cited idioms

1. Typed key-value schema. `extras/meilisearch/crates/milli/src/index.rs` gives every LMDB
   database a typed codec pair, so reads and writes are type-checked at compile time:

   ```rust
   /// A word and all the documents ids containing the word.
   pub word_docids: Database<Str, CboRoaringBitmapCodec>,
   /// Maps the proximity between a pair of words with all the docids where this relation appears.
   pub word_pair_proximity_docids: Database<U8StrStrCodec, CboRoaringBitmapCodec>,
   ```

2. Size-adaptive custom codecs.
   `extras/meilisearch/crates/milli/src/heed_codec/roaring_bitmap/cbo_roaring_bitmap_codec.rs`
   encodes bitmaps of up to `THRESHOLD = 7` integers as raw native-endian `u32`s and larger
   ones as serialized roaring bitmaps, choosing the decoder purely from byte length. The
   codec module distinguishes borrowed decoding (`Cow<[u8]>`, zero-copy from LMDB pages)
   from owned decoding via the separate `BytesDecodeOwned` trait.

3. A documented unsafe marker trait.
   `extras/meilisearch/crates/milli/src/update/new/thread_local.rs` defines
   `pub unsafe trait MostlySend {}` for types that are `!Send` only because sending would
   allow concurrent access to `!Sync` data, with a full safety contract in rustdoc, a
   `FullySend<T>` bridge (`unsafe impl<T> MostlySend for FullySend<T> where T: Send {}`),
   and structural impls for `RefCell<T>` and `Option<T>`. This is what lets the parallel
   indexer keep per-thread `Bump` arenas without `Mutex`es.

4. Interior mutability tuned to the scheduler.
   `extras/meilisearch/crates/milli/src/update/new/ref_cell_ext.rs` extends `RefCell` with
   `borrow_mut_or_yield`, which reacts to a failed dynamic borrow by calling
   `rayon::yield_local()` in a loop, cooperating with work stealing instead of panicking.

5. Arena allocation for indexing. The new indexer threads `&'indexer Bump` allocators
   through the pipeline, collecting into arena-backed containers:
   `hashbrown::HashSet<DocumentId, hashbrown::DefaultHashBuilder, &'extractor Bump>` in
   `extras/meilisearch/crates/milli/src/update/new/indexer/document_deletion.rs`.

6. Lazy streaming iterators over storage.
   `extras/meilisearch/crates/milli/src/search/facet/facet_sort_ascending.rs` implements
   `impl<'t> Iterator for AscendingFacetSort<'t, '_>` over an LMDB read transaction, so
   facet ordering never materializes intermediate collections; the same pattern appears in
   `facet_sort_descending.rs` and `documents/sort.rs`.

7. Grammar-first parsing. `extras/meilisearch/crates/filter-parser/src/lib.rs` opens with
   the complete BNF of the filter language as module docs (`filter = expression EOF`, ...),
   then implements it with `nom` + `nom_locate` so error spans point into user input, and
   dedicates parse rules to producing good errors for known mistakes (a `geoPoint` used as
   a value).

8. Exhaustiveness as a change detector.
   `extras/meilisearch/crates/index-scheduler/src/insta_snapshot.rs` destructures the whole
   `IndexScheduler { processing_tasks, env, version, queue, ..., features: _, webhooks: _, }`
   struct field by field; adding a field breaks compilation of the snapshot function until
   the author decides whether it must be snapshotted or explicitly ignored.

9. Deterministic concurrency testing with rendezvous channels.
   `extras/meilisearch/crates/index-scheduler/src/test_utils.rs` defines a `Breakpoint`
   enum (`Start`, `BatchCreated`, `ProcessBatchFailed`, ...) and sends each breakpoint twice
   over a zero-capacity crossbeam channel, so the scheduler thread blocks until the test
   explicitly advances it; tests then call `advance_till([...])` and snapshot the state.

10. Proc macros for a self-documenting router.
    `extras/meilisearch/crates/routes-macros/src/lib.rs` provides `#[routes::routes(...)]`,
    which registers actix handlers and simultaneously implements `utoipa::OpenApi`, keeping
    the OpenAPI document and the real routing table generated from one declaration
    (checked in CI by `check-openapi-file.yml`).

11. Platform cfg kept at the edges. The allocator swap is two lines in
    `extras/meilisearch/crates/meilisearch/src/main.rs`
    (`#[cfg(not(windows))] #[global_allocator] static ALLOC: mimalloc::MiMalloc`), and the
    test harness sets `TMP` on Windows versus `TMPDIR` elsewhere
    (`extras/meilisearch/crates/meilisearch/tests/common/server.rs`).

12. Build-time git metadata as a typed library.
    `extras/meilisearch/crates/build-info/src/lib.rs` reads `option_env!("VERGEN_GIT_*")`
    (emitted by `vergen-gitcl` in `build.rs`, overridable with `MEILI_NO_VERGEN` to keep
    incremental builds warm) and parses the describe string into a
    `DescribeResult::{Prototype, Release, Prerelease, NotATag}` enum instead of exposing raw
    strings.

13. Unsafe with receipts. Only 88 `unsafe` occurrences exist across roughly 261,000 lines,
    and load-bearing ones carry `// SAFETY:` comments, for example
    `// SAFETY: we are not keeping any reference to LMDB's data` in
    `extras/meilisearch/crates/milli/src/sharding/mod.rs` and
    `// SAFETY: precondition, the grenad value was saved from a string` in
    `extras/meilisearch/crates/milli/src/update/index_documents/extract/extract_vector_points.rs`.

### 10. Documentation practices

Process documentation is versioned next to the code. `extras/meilisearch/documentation`
holds `release.md` (the weekly release checklist), `versioning-policy.md`,
`experimental-features.md` (what qualifies as experimental and how users opt in), and
`prototypes.md` (Docker-image prototypes named `prototype-v<version>-<name>.<iteration>`,
enforced by `cargo xtask generate-prototype`). Root-level guides split by task:
`CONTRIBUTING.md` (build, test, `LINDERA_CACHE` and `MEILI_NO_VERGEN` build accelerators,
snapshot workflow with `cargo insta` and `MEILI_TEST_FULL_SNAPS`), `TESTING.md` (the
declarative test format), `BENCHMARKS.md` (its opening states the design philosophy:
"integration benchmarks, in the sense that they spawn an actual Meilisearch server and
measure its performance end-to-end"), and `PROFILING.md` (Puffin-based span profiling).

Rustdoc conventions: small crates make their README the crate documentation
(`#![doc = include_str!("../README.md")]` in
`extras/meilisearch/crates/permissive-json-pointer/src/lib.rs` and
`crates/flatten-serde-json/src/lib.rs`), which keeps README examples compile-tested. Public
macros document argument lists and behavior with examples
(`extras/meilisearch/crates/meili-snap/src/lib.rs`), and every database field of the core
`Index` struct carries a one-line doc. The PR template
(`extras/meilisearch/.github/pull_request_template.md`) is a requirements checklist:
automated tests added, upgrade tested with `--upgrade-db` when the DB changed, search
availability during upgrade verified, docs and integrations ready. Issue templates live in
`extras/meilisearch/.github/ISSUE_TEMPLATE` with a `config.yml` routing questions elsewhere.

### 11. Release and distribution

Versioning is workspace-wide semver (`1.53.1` in `extras/meilisearch/Cargo.toml`) under the
policy in `extras/meilisearch/documentation/versioning-policy.md`. There is no CHANGELOG
file; release notes are GitHub Releases, with a `skip changelog` PR label for exclusions
(applied by the automation in `update-cargo-toml-version.yml`). The release chain is fully
scripted:

- Version bumps are themselves a workflow:
  `extras/meilisearch/.github/workflows/update-cargo-toml-version.yml` rewrites the
  workspace version with `sd`, rebuilds to refresh `Cargo.lock`, and opens a PR.
- `extras/meilisearch/.github/scripts/check-release.sh` gates every publish job by checking
  the git tag against both `Cargo.toml` and `Cargo.lock` (read via
  `grep -A 1 '^name = "meilisearch-auth"'`), so a mismatched tag cannot ship.
- `publish-release-assets.yml` builds binaries for a 6-target matrix (macOS Intel and ARM,
  Windows, Linux amd64/aarch64/riscv64, the latter via `cross`) times two editions
  (community, enterprise as a cargo feature), uploads them to the GitHub release, and also
  runs the whole matrix nightly as a dry run so release day cannot discover a broken build.
- `publish-docker-images.yml` builds per-architecture images on native runners
  (`ubuntu-24.04` and `ubuntu-24.04-arm`), pushes them by digest, then merges digests into
  a multi-arch manifest; `latest` and `vX.Y` floating tags are only applied for stable
  releases, and a `nightly` tag is rebuilt daily by cron.
- `publish-apt-brew-pkg.yml` builds a Debian package with `cargo-deb` inside an
  `ubuntu:22.04` container (pinning glibc 2.35) and bumps the Homebrew formula.
- `latest-git-tag.yml` force-moves a `latest` git tag on stable releases, which is what
  `extras/meilisearch/download-latest.sh`, the curl-install script at the repository root,
  resolves.

The Dockerfile is a two-stage alpine build that injects `VERGEN_GIT_*` values as build args
(because the image builds outside a git checkout), ships both `meilisearch` and `meilitool`,
and runs under `tini` (`extras/meilisearch/Dockerfile`).

### 12. Lessons for quinjet

quinjet already has a stricter lint wall than Meilisearch, plus cargo-deny, taplo, typos,
coverage, miri, and mutants. The practices still worth importing, with mechanisms:

1. Hash-based snapshot testing for TUI frames and CLI output. Add `insta` (features
   `redactions`) plus a small `snapshot!`/`snapshot_hash!` macro module modeled on
   `extras/meilisearch/crates/meili-snap/src/lib.rs`: md5 hash inline in the test, full
   snapshot written only when an env toggle like `QUINJET_TEST_FULL_SNAPS=true` is set, and
   dynamic redactions for volatile values (commit hashes, timestamps) the way meili-snap
   rewrites UUIDs. This keeps hundreds of rendered-frame assertions out of the diff.
2. `disallowed-methods` as an invariant enforcer. quinjet's clippy wall is level-based;
   Meilisearch shows the complementary tool: entries in `clippy.toml` such as
   `{ path = "std::process::exit", reason = "..." }` or banning direct
   `crossterm::terminal::disable_raw_mode` outside the terminal-guard module, with exactly
   one `#[allow(clippy::disallowed_methods)]` at the sanctioned call site, as in
   `extras/meilisearch/crates/meilisearch-types/src/archive_ext.rs`.
3. Pin every GitHub Action to a commit SHA with a version comment, and add a
   `.github/dependabot.yml` with `package-ecosystem: "github-actions"`, monthly interval,
   and `cooldown.default-days: 7`, copying `extras/meilisearch/.github/dependabot.yml`.
4. Add `merge_group:` to the CI trigger list and enable the GitHub merge queue, with the
   expensive jobs skipped inside the queue via
   `if: github.event_name != 'merge_group'` as in
   `extras/meilisearch/.github/workflows/test-suite.yml`.
5. Declarative end-to-end tests of the CLI surface. Since every quinjet operation is a
   subcommand, encode scenario files (JSON: fixture-repo setup, subcommand invocations,
   expected output) executed by a test binary, mirroring
   `extras/meilisearch/workloads/tests` plus `cargo xtask test`. For a single crate this can
   be one integration test that walks a `tests/scenarios/*.json` glob.
6. Typestate fixtures in integration tests. Use uninhabited marker enums and
   `PhantomData<State>` (`Server<Owned>` versus `Server<Shared>` in
   `extras/meilisearch/crates/meilisearch/tests/common/server.rs`) for shared read-only
   fixture git repositories versus per-test mutable clones, so a test cannot mutate the
   shared fixture by construction.
7. Profile tuning in `Cargo.toml`: `codegen-units = 1` under `[profile.release]`, a
   `[profile.release-with-debug]` with `inherits = "release"` and `debug = true` for
   profiling sessions, and `[profile.dev.package.<dep>] opt-level = 3` for hot dependencies
   so debug-mode tests stay fast, following `extras/meilisearch/Cargo.toml`.
8. A nightly flaky-test hunt: a scheduled workflow installing `cargo-flaky` and running
   `cargo flaky -i 100 --release`, as in
   `extras/meilisearch/.github/workflows/flaky-tests.yml`. TUI event-loop tests are prime
   flake candidates.
9. Release gate script: a `check-release.sh` that verifies the pushed tag equals the version
   in both `Cargo.toml` and `Cargo.lock` before any artifact job runs, plus a scheduled dry
   run of the full binary build matrix so releases cannot fail on release day
   (`extras/meilisearch/.github/workflows/publish-release-assets.yml`).
10. Fuzz the parsers. Any quinjet input parser (refspec, filter, or keybinding syntax)
    should get a committed `fuzz/` directory with a `libfuzzer-sys` target that, like
    `extras/meilisearch/crates/filter-parser/fuzz/fuzz_targets/parse.rs`, panics only on
    internal-error variants and treats user-facing parse errors as success.
11. `-D clippy::todo` in the CI clippy invocation (cheap even on top of a strict wall, it
    also covers test and bench targets via `--all-targets`), and `RUSTFLAGS: "-D warnings"`
    at workflow env level so plain builds fail on warnings too.
12. Documented safe-wrapper pattern for the one dangerous call: when an operation is risky
    (deleting refs, force pushes), expose it only through an extension trait like
    `ArchiveExt::safe_unpack` and ban the raw method repo-wide via `clippy.toml`.
13. Structured panic reporting: install a panic hook that logs `PanicHookInfo` through the
    tracing pipeline before the TUI teardown, and have `main` walk and log the
    `Error::source()` chain on exit, as in
    `extras/meilisearch/crates/meilisearch/src/main.rs`.
14. `#![doc = include_str!("../README.md")]` on the crate root so README examples are
    compile-tested, following
    `extras/meilisearch/crates/permissive-json-pointer/src/lib.rs`.

---

## astral-sh/ruff (49222 stars)

### 1. What the project is and how big it is

Ruff is an extremely fast Python linter and code formatter written in Rust, and the same
repository also hosts ty, Astral's Python type checker. The package description in
extras/ruff/crates/ruff/Cargo.toml reads:

```toml
[package]
name = "ruff"
version = "0.16.3"
description = "An extremely fast Python linter and code formatter"
```

Industry adopted Ruff because it replaces an entire stack of Python tools (Flake8 and its
plugin ecosystem, isort, pyupgrade, Black-compatible formatting) with one native binary that
is orders of magnitude faster, distributed on PyPI, and driven by a single configuration
file. The repository is therefore a compiler-shaped codebase: a lexer, parser, AST, semantic
model, several hundred lint rules, a formatter, an LSP server, and an incremental type
checker all live here.

Measured directly from the clone:

- 51 workspace crates under extras/ruff/crates (the workspace is `members = ["crates/*"]`
  in extras/ruff/Cargo.toml).
- 1957 Rust source files, 766,844 lines of Rust in total; 766,361 of those lines are inside
  extras/ruff/crates.
- The two biggest crates are extras/ruff/crates/ruff_linter (199,648 lines) and
  extras/ruff/crates/ty_python_semantic (180,290 lines).
- 3,703 insta snapshot files (`*.snap`) across 84 `snapshots` directories.
- 6 libFuzzer targets in extras/ruff/fuzz/fuzz_targets.
- 20 GitHub workflow files totaling 4,393 lines in extras/ruff/.github/workflows.

### 2. Repository layout

```text
extras/ruff/
|-- Cargo.toml              workspace root: members, shared deps, lints, profiles
|-- Cargo.lock
|-- clippy.toml             disallowed methods, doc-valid-idents
|-- rustfmt.toml            edition pinning for the formatter
|-- rust-toolchain.toml     pinned stable toolchain (1.97.1)
|-- _typos.toml             spell-checker configuration
|-- dist-workspace.toml     cargo-dist release configuration
|-- .config/nextest.toml    test-runner profiles (ci profile, serial groups)
|-- .cargo/config.toml      cargo aliases (cargo dev, cargo benchmark)
|-- .pre-commit-config.yaml prioritized hook pipeline
|-- crates/                 51 crates, ruff_* and ty_* prefixes
|-- fuzz/                   separate cargo-fuzz workspace
|-- python/                 py-fuzzer and ruff-ecosystem helper packages
|-- scripts/                release.sh, add_rule.py, PGO build, docs generators
|-- docs/                   mkdocs source (linter.md, formatter.md, versioning.md)
|-- playground/             web playground built on the wasm crates
|-- changelogs/             archived changelog per minor series (0.1.x.md .. 0.15.x.md)
`-- .github/                workflows, CODEOWNERS, templates, renovate, zizmor config
```

The split works because every architectural layer is its own crate with an explicit
dependency direction: `ruff_text_size` and `ruff_source_file` at the bottom, then
`ruff_python_ast`, `ruff_python_parser`, `ruff_python_semantic`, then `ruff_linter` and
`ruff_python_formatter`, and finally the `ruff` CLI crate that only wires commands together.
The naming convention is documented in extras/ruff/AGENTS.md: `ruff_*` for linter code,
`ty_*` for type-checker code, with ty reusing the parser and AST crates. Crate boundaries
also drive CI: the `determine_changes` job in extras/ruff/.github/workflows/ci.yaml maps
directories to flags (parser, linter, formatter, ty, fuzz) and downstream jobs run only when
their layer changed.

Two details are worth copying. First, the fuzz targets live in a separate workspace
(extras/ruff/fuzz/Cargo.toml declares `[workspace] members = ["."]` with the comment
"Prevent this from interfering with workspaces") so nightly-only fuzz dependencies never
infect the main lockfile. Second, developer tooling is itself a crate:
extras/ruff/crates/ruff_dev is an internal CLI exposed through a cargo alias in
extras/ruff/.cargo/config.toml:

```toml
[alias]
dev = "run --package ruff_dev --bin ruff_dev"
benchmark = "bench -p ruff_benchmark --bench linter --bench formatter --"
```

`cargo dev generate-all` regenerates the JSON schema, CLI help, options reference, and rules
table (see extras/ruff/crates/ruff_dev/src/generate_all.rs and its sibling modules), and CI
fails if the committed output drifts.

### 3. Cargo manifest practices

The root extras/ruff/Cargo.toml is the single source of truth for metadata, versions, and
lints:

```toml
[workspace.package]
# Please update rustfmt.toml when bumping the Rust edition
edition = "2024"
rust-version = "1.95"
homepage = "https://docs.astral.sh/ruff"
license = "MIT"
```

Notable practices:

- Every third-party dependency is declared once under `[workspace.dependencies]` with an
  explicit version, and member crates only write `anyhow = { workspace = true }`. Internal
  crates are also declared there with both `version` and `path`, so `cargo publish` works
  for the whole graph.
- 48 of the 51 crates end their manifest with `[lints] workspace = true`
  (for example extras/ruff/crates/ruff/Cargo.toml), so lint policy lives in exactly one
  place.
- MSRV is separated from the development toolchain: `rust-version = "1.95"` in the
  workspace manifest, while extras/ruff/rust-toolchain.toml pins `channel = "1.97.1"`.
  CONTRIBUTING documents the policy as "latest minus two" (extras/ruff/CONTRIBUTING.md,
  "Upgrading Rust" section), and CI reads the MSRV out of the manifest with a TOML action
  rather than hardcoding it.
- Feature flags gate integrations, not behavior: the linter exposes `clap`, `serde`,
  `schemars`, and a `test-rules` feature that the CLI crate enables only in
  `[dev-dependencies]` (extras/ruff/crates/ruff/Cargo.toml: "Enable test rules during
  development").
- Allocators are selected per platform with `[target.'cfg(...)'.dependencies]`:
  `tikv-jemallocator` on 64-bit Unix, `mimalloc` on Windows
  (extras/ruff/crates/ruff/Cargo.toml).
- Profiles are tuned deliberately. `release` uses `lto = "fat"` with `codegen-units = 16`,
  but hot crates get their own override:

```toml
[profile.release.package.ruff_python_parser]
codegen-units = 1
```

  There is a documented `profiling` profile (release minus fat LTO, with full debug info)
  for benchmarks, a `minimal-size` profile (`opt-level = "z"`), a `fast-test` profile, and
  `[profile.dev.package.insta]` bumps snapshot-diffing dependencies to `opt-level = 3` so
  tests stay fast in dev builds.

- Unused-dependency policy is machine-checked: `[workspace.metadata.cargo-shear]` lists the
  few intentional exceptions, and CI runs `cargo shear --deny-warnings`.
- The CLI library sets `[lib] doctest = false` to keep the test matrix intentional.

### 4. Formatting

extras/ruff/rustfmt.toml is deliberately tiny:

```toml
edition = "2024"
style_edition = "2024"
```

The philosophy is default rustfmt, no bikeshedding; the only reason the file exists is to
pin the edition for editors that run rustfmt standalone, and the workspace manifest carries
a reminder comment to keep the two in sync. Formatting of everything else is layered in
extras/ruff/.pre-commit-config.yaml, which runs hooks in explicit priority order:
`rustfmt` for Rust, `prettier` for YAML, `mdformat` plus `markdownlint-fix` for Markdown
(priority 1 so it runs after mdformat), Ruff itself for the repository's own Python
(`ruff-format`, `ruff-check --fix`), and `uv-lock` to keep the Python lockfile fresh. Every
hook revision is pinned to a full commit SHA with a `# frozen: vX.Y.Z` comment.

extras/ruff/.editorconfig sets the baseline for all editors: UTF-8, LF, final newline,
2-space indent, with overrides:

```ini
[*.{rs,py,pyi,toml}]
indent_size = 4

[*.snap]
trim_trailing_whitespace = false
```

The `.snap` override matters: snapshot files must be byte-exact, so editors must not "fix"
them. Spelling is enforced with typos via extras/ruff/_typos.toml, which shows how to make a
spell checker viable at scale: `extend-exclude` for vendored code and snapshots,
`extend-words` for legitimate oddities (`arange = "arange"  # e.g. numpy.arange`), and a
regex line-escape (`spellchecker:disable-line`).

### 5. Linting

Clippy policy lives in `[workspace.lints]` in extras/ruff/Cargo.toml and is enforced in CI
as errors: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
(extras/ruff/.github/workflows/ci.yaml, plus a second wasm-target clippy run). The shape of
the policy:

```toml
[workspace.lints.rust]
unsafe_code = "warn"
unreachable_pub = "warn"
unexpected_cfgs = { level = "warn", check-cfg = [
    "cfg(fuzzing)",
    "cfg(codspeed)",
] }

[workspace.lints.clippy]
pedantic = { level = "warn", priority = -2 }
```

Pedantic is on wholesale at low priority, then individual pedantic lints are allowed back
with one line each (`too_many_lines`, `similar_names`, `module_name_repetitions`, and so
on), several with a rationale comment, for example:

```toml
needless_continue = "allow" # An explicit continue can be more readable, especially if the alternative is an empty block.
```

On top of that, a curated set of restriction and nursery lints is opted in:
`iter_over_hash_type`, `print_stdout`, `print_stderr`, `dbg_macro`, `exit`, `get_unwrap`,
`rc_buffer`, `rc_mutex`, `rest_pat_in_fully_bound_structs`, `redundant_clone`,
`debug_assert_with_mut_call`, `unused_peekable`. Because `print_stdout` is warned
workspace-wide, the crates that legitimately print (the CLIs) opt out at the crate root:
extras/ruff/crates/ruff/src/lib.rs opens with `#![allow(clippy::print_stdout)]` and
extras/ruff/crates/ruff_dev/src/main.rs with
`#![allow(clippy::print_stdout, clippy::print_stderr)]`. Everywhere else, printing is a
lint error, which forces output through the `Printer` abstraction.

extras/ruff/clippy.toml carries the semantic configuration: `doc-valid-idents` so rustdoc
prose can say "NumPy" without a lint, `ignore-interior-mutability` with per-type
justification comments, and, most interestingly, `disallowed-methods` used as an
architectural fence:

```toml
disallowed-methods = [
    { path = "std::env::var", reason = "Use System::env_var instead in ty crates" },
    { path = "std::fs::read_to_string", reason = "Use System::read_to_string instead in ty crates" },
    { path = "std::path::Path::exists", reason = "Use System::path_exists instead in ty crates" },
]
```

This bans direct filesystem and environment access so all IO goes through the `System`
abstraction that makes ty testable against an in-memory filesystem. The workspace table
sets `disallowed_methods = "allow"` with the comment "Enabled at the crate level", so only
the crates that opted in pay the cost. Suppressions prefer `#[expect]` with reasons, for
example extras/ruff/crates/ruff_python_parser/src/parser/mod.rs:

```rust
#[expect(clippy::inline_always, reason = "reduces list-parser branch misses")]
```

There are 355 `#[expect(...)]` attributes in the tree, each self-expiring if the lint stops
firing. Custom check infrastructure goes beyond clippy: `cargo shear` for unused deps,
shellcheck over all `*.sh` in CI, actionlint plus zizmor for the workflows themselves, and
the `scripts` CI job runs the code generators and fails on `git status --porcelain` drift.

### 6. CI/CD

extras/ruff/.github/workflows/ci.yaml (1,392 lines) is the core pipeline. Structure:

- Top of file: `permissions: {}` (zero default token permissions; jobs opt in, for example
  the CodSpeed jobs request `id-token: write` for OIDC), a concurrency group that cancels
  superseded runs, and `defaults: run: shell: bash`.
- `determine_changes` computes a merge base, then runs a series of `git diff --quiet`
  checks that emit boolean outputs (`parser`, `linter`, `formatter`, `ty`, `fuzz`,
  `playground`, `benchmarks`, `release`, `code`). Nearly every later job is gated on one of
  these flags, so a docs-only PR compiles nothing.
- Test jobs: `cargo-test-linux` (nextest plus `cargo insta test --unreferenced reject`, so
  orphaned snapshots fail CI), `cargo-test-linux-release` under the `profiling` profile,
  `cargo-test-other` with a matrix over Windows and macOS runners, `cargo-test-wasm` via
  `wasm-pack test --node` for both wasm crates, and `cargo-build-msrv`, which reads
  `workspace.package.rust-version` from Cargo.toml with `SebRollen/toml-action` and builds
  the test suite on that toolchain.
- Docs discipline inside CI: `cargo doc --all --no-deps` with `RUSTDOCFLAGS: "-D warnings"`,
  plus a second `--document-private-items` pass over an allowlist of already-clean crates
  "to prevent regression" (extras/ruff/.github/workflows/ci.yaml around line 400).
- Behavioral regression jobs unique to this project: `ecosystem` builds the baseline and PR
  binaries and diffs `ruff check`/`ruff format` output across a corpus of real repositories,
  uploading a markdown report artifact; `fuzz-ty` builds the merge-base and PR `ty` binaries
  and fuzzes for new panics only; `check-formatter-instability-and-black-similarity` runs
  scripts/formatter_ecosystem_checks.sh; `check-ruff-lsp` runs the downstream ruff-lsp test
  suite against the PR binary.
- Benchmarks run on every relevant PR through CodSpeed (instrumented and walltime modes),
  with build and run split into separate jobs that pass the benchmark binary as an artifact.
- Caching is `Swatinem/rust-cache` everywhere with
  `save-if: ${{ github.ref == 'refs/heads/main' }}`, so PRs read the cache but only main
  writes it, plus `shared-key: ruff-linux-debug` so sibling jobs share one cache.
- Every third-party action is pinned to a full commit SHA with a version comment, every
  checkout sets `persist-credentials: false`, and jobs carry `timeout-minutes`.
- The required-checks pattern: a final `required-checks-passed` job with `if: always()`
  needs the core jobs and fails if any dependency result is neither success nor skipped, so
  branch protection points at one check while path-filtered jobs stay skippable.

Security hardening is itself linted: zizmor runs as a pre-commit hook with exceptions
tracked in extras/ruff/.github/zizmor.yml, actionlint runs with shellcheck integration
(extras/ruff/.github/actionlint.yaml whitelists the custom Depot and Namespace runner
labels), and `check-jsonschema` validates workflow syntax. Renovate
(extras/ruff/.github/renovate.json5) updates actions, cargo, pre-commit hooks, npm, and
Python deps on a weekly schedule. Scheduled workflows do the long-tail work:
daily_fuzz.yaml fuzzes the parser with 1,000 random seeds every night and auto-files an
issue on failure; sync_typeshed.yaml vendors typeshed weekly; typing_conformance.yaml and
the ty-ecosystem workflows track type-checker behavior; memory_report.yaml posts memory
profiles on PRs touching ty internals.

### 7. Testing

The test strategy is snapshot-first, at three levels:

1. Rule and unit tests live inside each crate (`#[cfg(test)] mod tests`), and lint rules
   assert their diagnostics with insta snapshots; the 3,703 `.snap` files under
   `snapshots/` directories sit next to the code they verify, for example
   extras/ruff/crates/ruff_linter/src/rules/*/snapshots.
2. CLI integration tests in extras/ruff/crates/ruff/tests use `insta_cmd` to snapshot the
   full process contract. From extras/ruff/crates/ruff/tests/cli/lint.rs:

   ```rust
   assert_cmd_snapshot!(test.check_command()
        .arg("--config")
        .arg("ruff.toml")
        .args(["--stdin-filename", "test.py"])
        .arg("-")
        .pass_stdin(r#"a = "abcba".strip("aba")"#), @"
   success: false
   exit_code: 1
   ----- stdout -----
   test.py:1:5: Q000 [*] Double quotes found but single quotes preferred
   ```

   One assertion pins exit code, stdout, and stderr at once. The shared fixture in
   extras/ruff/crates/ruff/tests/cli/main.rs (`CliTest`) creates a temp project dir,
   canonicalizes it (with `dunce` to avoid Windows UNC paths), and installs insta filters
   that rewrite the temp path to `[TMP]/` so snapshots are cross-platform stable.
3. ty's type-inference tests are Markdown files: any fenced `py` block with
   `# revealed: ...` comments is executed as a test by the framework in
   extras/ruff/crates/ty_test (documented in extras/ruff/crates/ty_test/README.md, "Any
   Markdown file can be a test suite"). Tests are literate specifications, which is why
   thousands of behaviors are covered without Rust boilerplate.

Supporting infrastructure: extras/ruff/.config/nextest.toml defines a `ci` profile
(`failure-output = "immediate-final"`, `fail-fast = false`, a 60-second
`terminate-after` as a deadlock stopgap) and a `serial` test group pinning the file-watcher
tests to one thread. Property-style testing exists via `quickcheck` (declared in the
workspace deps) and six libFuzzer targets in extras/ruff/fuzz/fuzz_targets
(`ruff_parse_idempotency.rs`, `ruff_formatter_validity.rs`, `ty_check_invalid_syntax.rs`,
and friends), plus the Python-based differential fuzzer in python/py-fuzzer that CI runs on
parser changes. Benchmarks are a dedicated crate, extras/ruff/crates/ruff_benchmark, with
criterion/divan benches per subsystem (`benches/linter.rs`, `parser.rs`, `formatter.rs`,
`ty.rs`) wired to CodSpeed for continuous regression tracking. Test-only rules ship behind
the linter's `test-rules` cargo feature so the CLI test suite can trigger every fix
pathway.

### 8. Error handling and API design

The pattern is thiserror (or hand-written `std::error::Error` impls) in library crates,
anyhow only at the binary boundary. extras/ruff/crates/ruff_python_parser/src/error.rs
defines a structured `ParseError { error: ParseErrorType, location: TextRange }` with
`Deref` to its kind and a manual `Display`; a dozen crates such as
extras/ruff/crates/ty_project/src/metadata/options.rs use `thiserror::Error` derives.
The CLI's process contract is a dedicated enum in extras/ruff/crates/ruff/src/lib.rs:

```rust
#[derive(Copy, Clone)]
pub enum ExitStatus {
    /// Linting was successful and there were no linting errors.
    Success,
    /// Linting was successful but there were linting errors.
    Failure,
    /// Linting failed.
    Error,
}

impl From<ExitStatus> for ExitCode {
    fn from(status: ExitStatus) -> Self {
        match status {
            ExitStatus::Success => ExitCode::from(0),
            ExitStatus::Failure => ExitCode::from(1),
            ExitStatus::Error => ExitCode::from(2),
        }
    }
}
```

`main` (extras/ruff/crates/ruff/src/main.rs) returns `ExitCode`, never calls
`process::exit`, and its `report_error` handler exits 0 on `BrokenPipe` (crediting
ripgrep), writes `ruff failed` in red bold to a locked stderr with `writeln!(...).ok()` so
a broken stderr cannot panic, and prints the full `err.chain()` of causes. Panic policy is
cultural and enforced in review: extras/ruff/AGENTS.md instructs "Try hard to avoid
patterns that require `panic!`, `unreachable!`, `.unwrap()` or `.expect()`. Instead, try to
encode those constraints in the type system." Visibility discipline is mechanical:
`unreachable_pub = "warn"` workspace-wide plus the documented preference for narrow
visibility, with `pub(crate)` used pervasively (see the module list in
extras/ruff/crates/ruff/src/lib.rs where only `args` and `resolve` are `pub`). API design
favors small vocabulary types over primitives: `TextSize`/`TextRange` newtypes
(extras/ruff/crates/ruff_text_size), typed indexes via `IndexVec` (extras/ruff/crates/
ruff_index), and the `Violation` trait family in
extras/ruff/crates/ruff_linter/src/violation.rs, where each rule is a struct with
`message()`, optional `fix_title()`, and a `FIX_AVAILABILITY` associated const, letting the
framework derive docs, codes, and fix metadata per rule.

### 9. Deep Rust usage: ten cited idioms

1. Trait-per-rule design: `Violation: ViolationMetadata + Sized` with an associated
   `const FIX_AVAILABILITY: FixAvailability` and a default `into_diagnostic` implementation
   (extras/ruff/crates/ruff_linter/src/violation.rs). Hundreds of rule structs implement
   one narrow trait, and a derive macro (`ViolationMetadata` in
   extras/ruff/crates/ruff_macros/src/violation_metadata.rs) extracts each rule's rustdoc
   as its user-facing explanation, so docs and behavior cannot drift apart.
2. Lifetime-parameterized visitors: `pub trait Visitor<'a>` with `visit_stmt(&mut self,
   stmt: &'a Stmt)` and free `walk_*` functions (extras/ruff/crates/ruff_python_ast/src/
   visitor.rs). Borrowing the AST for `'a` lets rule state hold references into the tree
   with zero copies.
3. Newtypes with proc-macro leverage: `#[newtype_index]` (extras/ruff/crates/ruff_macros/
   src/newtype_index.rs) generates dense u32-backed ID types consumed through
   `IndexVec`/`IndexSlice` (extras/ruff/crates/ruff_index/src/lib.rs, "Inspired by
   rustc_index"), giving array indexing that is type-checked per arena.
4. Zero-copy string handling: `Cow<'_, str>` returns from trivia utilities such as
   `pub fn dedent(text: &str) -> Cow<'_, str>` and `expand_tabs`
   (extras/ruff/crates/ruff_python_trivia/src/textwrap.rs, whitespace.rs); 192 `Cow<`
   occurrences across the crates, allocating only when a transformation actually changes
   the text.
5. Lazy interior mutability chosen per need: `Locator` wraps the source with
   `index: OnceCell<LineIndex>` so the line index is computed only if a diagnostic needs it
   (extras/ruff/crates/ruff_linter/src/locator.rs), and even marks the expensive path
   `#[deprecated(note = "This is expensive, avoid using outside of the diagnostic
   phase...")]` so misuse is a compile-time warning. 99 `LazyLock` uses cover static
   regexes and tables.
6. Data parallelism at the file level: `paths.par_iter().filter_map(|resolved_file| ...)`
   in extras/ruff/crates/ruff/src/commands/check.rs runs the whole linter per file on
   rayon, with the cache layer using `into_par_iter` for persistence
   (extras/ruff/crates/ruff/src/cache.rs); ty instead builds on salsa queries
   (`#[salsa::tracked]` throughout extras/ruff/crates/ty_python_semantic) for incremental,
   demand-driven computation.
7. Memory layout as a tested invariant: `assert_eq_size!(FormatElement, [u8; 16])` and
   friends in extras/ruff/crates/ruff_formatter/src/format_element.rs pin the size of hot
   enums, so an accidental variant growth fails the build rather than slowing the printer.
8. Linear-type emulation with `drop_bomb::DebugDropBomb` in the formatter printer, the
   parser scratch buffer, and ty's diagnostic context (extras/ruff/crates/ruff_formatter/
   src/printer/mod.rs, extras/ruff/crates/ruff_python_parser/src/parser/scratch_buffer.rs,
   extras/ruff/crates/ty_python_semantic/src/types/context.rs): a guard that panics in
   debug builds if dropped without being defused, encoding "you must finish this" in the
   API.
9. Unsafe as an audited exception: `unsafe_code = "warn"` workspace-wide, upgraded to
   `#![forbid(unsafe_code)]` in leaf crates like extras/ruff/crates/ruff_text_size/src/
   lib.rs, and the rare use sites carry both an `#[expect]` and a SAFETY comment:
   `#[expect(unsafe_code, reason = "reconstructs a type-erased AST reference")]` above a
   `// SAFETY: The caller guarantees that pointer is readable...` block in
   extras/ruff/crates/ruff_python_ast/src/generated.rs.
10. Platform cfg handled once, at the allocator and linker level: the cascading
    `#[cfg(all(not(target_os = "windows"), ..., any(target_arch = "x86_64", ...)))]`
    global-allocator selection in extras/ruff/crates/ruff/src/main.rs, plus static CRT
    linking for MSVC via `rustflags = ["-C", "target-feature=+crt-static"]` in
    extras/ruff/.cargo/config.toml with an issue link explaining why.
11. Bitflags for option sets crossing function boundaries: `PrinterFlags` with documented
    bits (extras/ruff/crates/ruff/src/printer.rs), and serde-aware view structs like
    `ExpandedStatistics<'a>` borrowing `&'a str` fields to serialize without cloning.
12. Codegen where Rust macros would obscure: the entire AST (`generated.rs`) is produced by
    extras/ruff/crates/ruff_python_ast/generate.py from a declarative
    extras/ruff/crates/ruff_python_ast/ast.toml, and CI regenerates it and diffs
    (`test -z "$(git status --porcelain)"` in the `scripts` job of ci.yaml), keeping the
    generator honest.

### 10. Documentation practices

- Crate-level `//!` docs state intent and stability up front:
  extras/ruff/crates/ruff_linter/src/lib.rs opens with "This is the library for the [Ruff]
  Python linter. **The API is currently completely unstable**".
  extras/ruff/crates/ruff_text_size/src/lib.rs even documents when not to use the crate.
- Rule docs are the user docs: each rule struct's rustdoc ("What it does / Why is this
  bad?" sections, visible throughout extras/ruff/crates/ruff_linter/src/rules) is
  extracted by the `ViolationMetadata` derive and rendered into the docs site by
  extras/ruff/crates/ruff_dev/src/generate_docs.rs. One source, two audiences.
- The user site is mkdocs (extras/ruff/mkdocs.yml plus extras/ruff/docs), built with
  `mkdocs build --strict` in CI; generated pages come from `cargo dev` generators and
  scripts/generate_mkdocs.py, and scripts/check_docs_formatted.py lints the code blocks in
  the docs themselves.
- extras/ruff/CONTRIBUTING.md is 1,128 lines and unusually operational: project layout,
  example rule-addition walkthroughs, the full release checklist, MSRV policy, and a long
  "Benchmarking and Profiling" chapter.
- Doc quality is CI-enforced (`RUSTDOCFLAGS: "-D warnings"` on `cargo doc`, per section 6),
  so broken intra-doc links cannot land.
- Issue templates are structured YAML forms (extras/ruff/.github/ISSUE_TEMPLATE/
  1_bug_report.yaml, 2_rule_request.yaml, 3_question.yaml), and the PR template asks for a
  Summary and a Test Plan (extras/ruff/.github/PULL_REQUEST_TEMPLATE.md). CODEOWNERS routes
  crates to maintainers (`/crates/ruff_python_formatter/ @MichaReiser` and team-based
  `*_notified` groups for ty crates).
- extras/ruff/.git-blame-ignore-revs keeps mass reformatting commits out of blame.

### 11. Release and distribution

Versioning is documented in extras/ruff/docs/versioning.md: pre-1.0 semver where minor
means breaking and patch means fixes, with an explicit list of what counts as breaking for
the linter, formatter, and server. Internal crates are published as `0.0.x` with "no
stability guarantees" (visible in the workspace dependency table: `ruff_cache = { version
= "0.0.9", ... }` next to `ruff = { version = "0.16.3", ... }`).

Changelog discipline: a curated extras/ruff/CHANGELOG.md for the current series, archived
per-minor files in extras/ruff/changelogs (0.1.x.md through 0.15.x.md), and breaking
changes duplicated into extras/ruff/BREAKING_CHANGES.md. The release itself is
scripts/release.sh plus the `rooster` changelog generator, then human editorializing
(extras/ruff/CONTRIBUTING.md, "Release Process").

Distribution is cargo-dist, configured in extras/ruff/dist-workspace.toml: 18 prebuilt
targets (including musl, armv7, s390x, riscv64), shell and PowerShell installers,
`dispatch-releases = true` (releases are triggered by workflow dispatch, and the git tag is
created only after wheels are on PyPI, per the CONTRIBUTING checklist),
`github-attestations = true` for artifact provenance, and even the actions used inside the
generated workflow pinned by commit under `[dist.github-action-commits]`. The generated
extras/ruff/.github/workflows/release.yml adds a human gate: a `release-gate` job bound to
a GitHub environment that "requires a 2-factor approval, i.e., the workflow must be
approved by another team member". Local artifact jobs are delegated to
build-binaries.yml (maturin wheels for every platform, with a PGO pass on x86_64 via
scripts/build_ruff_pgo.py and `-Cprofile-use` flags), build-docker.yml, and
build-wasm.yml; publish jobs push to PyPI via `uv publish` under an environment with
`id-token: write` (trusted publishing, no long-lived secrets,
extras/ruff/.github/workflows/publish-pypi.yml), to crates.io, npm, and a release mirror.
As a CLI, ruff ships shell completions through a subcommand rather than packaged files:
`GenerateShellCompletion { shell: clap_complete_command::Shell }` in
extras/ruff/crates/ruff/src/args.rs.

### 12. Lessons for quinjet

quinjet already has the strict-clippy, rustfmt, cargo-deny, taplo, typos, coverage, miri,
and mutants story. What ruff adds on top, with mechanisms:

1. Snapshot-test the whole CLI contract. Add `insta` and `insta-cmd` as dev-dependencies
   and write `assert_cmd_snapshot!` tests that pin exit code, stdout, and stderr per
   subcommand, with a `CliTest`-style fixture (tempdir plus
   `settings.add_filter(tempdir_regex, "[TMP]/")`) exactly as in
   extras/ruff/crates/ruff/tests/cli/main.rs; for a Git TUI, the fixture would also
   `git init` and seed commits. Run `cargo insta test --unreferenced reject` in CI so stale
   snapshots fail.
2. Make the exit-code contract a type. quinjet's clap subcommands should return an
   `ExitStatus` enum with `impl From<ExitStatus> for ExitCode` (pattern from
   extras/ruff/crates/ruff/src/lib.rs), and `main` should adopt ruff's `report_error`:
   exit 0 on `ErrorKind::BrokenPipe`, print the `anyhow` chain to a locked stderr with
   `writeln!(...).ok()`.
3. Adopt zizmor and actionlint for the workflows. Both run as pre-commit hooks in
   extras/ruff/.pre-commit-config.yaml with pinned SHAs; quinjet can run them in the
   existing Makefile lint target (`zizmor .github/workflows`, `actionlint`) and set
   workflow-level `permissions: {}` plus `persist-credentials: false` on every checkout.
4. Pin every GitHub action to a full commit SHA with a `# vX.Y.Z` comment and let Renovate
   bump them (extras/ruff/.github/renovate.json5, `enabledManagers` including
   `github-actions`); this is strictly stronger than tag pinning.
5. Add the `required-checks-passed` aggregation job (`if: always()` plus a jq check over
   `toJSON(needs)`, extras/ruff/.github/workflows/ci.yaml lines 1368-1392) so branch
   protection needs exactly one context even as jobs are added or skipped.
6. Verify MSRV mechanically: keep `rust-version` in Cargo.toml, read it in CI with
   `SebRollen/toml-action` on `workspace.package.rust-version` (single-crate: `package.
   rust-version`), and run `cargo +$MSRV test --no-run` as ruff's `cargo-build-msrv` job
   does.
7. Use `Swatinem/rust-cache` with `save-if: github.ref == 'refs/heads/main'` and a
   `shared-key` per platform-profile pair, so PR runs never poison the cache.
8. Add `cargo-shear` (`cargo shear --deny-warnings`, exceptions under
   `[workspace.metadata.cargo-shear]`) to catch unused dependencies; it complements
   cargo-deny, which does not check usage.
9. Turn architectural rules into `disallowed-methods` entries in clippy.toml with `reason`
   strings, as ruff does for its `System` abstraction. For quinjet: ban
   `std::process::exit` outside main, or ban raw `crossterm::execute!` outside the terminal
   module, so the TUI/CLI layering is machine-enforced.
10. Keep docs from rotting with `cargo doc --no-deps` under `RUSTDOCFLAGS="-D warnings"`,
    plus a `--document-private-items` pass once the crate is clean (ci.yaml around line
    400); add both as Makefile targets and CI steps.
11. Adopt nextest with a `ci` profile in `.config/nextest.toml`: `fail-fast = false`,
    `failure-output = "immediate-final"`, `slow-timeout = { period = "1s",
    terminate-after = 60 }` to convert deadlocks into failures, and a `serial` test group
    for tests that touch a shared Git repo or the terminal.
12. Ship with cargo-dist: a `dist-workspace.toml` gives quinjet multi-target archives,
    shell/PowerShell installers, GitHub attestations, and a generated release.yml;
    `dispatch-releases = true` plus an approval-gated `release-gate` environment copies
    ruff's two-person release control.
13. Expose completions as a subcommand via the `clap_complete_command` crate
    (`GenerateShellCompletion { shell: clap_complete_command::Shell }` in
    extras/ruff/crates/ruff/src/args.rs), which quinjet's command-layer design can add as
    one more subcommand.
14. Consider a tiny differential-fuzz loop for the CLI surface: ruff's daily_fuzz.yaml
    pattern (scheduled workflow, random seeds, auto-file an issue on failure) applied to
    quinjet could run random command sequences against a scratch repo nightly and compare
    `--porcelain`-style output between the PR and main binaries, like the `fuzz-ty` job.
15. Add size regression guards for hot types with `static_assertions::assert_eq_size!` in
    a `#[cfg(test)] mod sizes`, as extras/ruff/crates/ruff_formatter/src/format_element.rs
    does; for a TUI, event and draw-command enums are the candidates.

---

## bevyengine/bevy (47648 stars)

### 1. What Bevy is and how big it is

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage: cited idioms

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

### 10. Documentation practices

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

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## helix-editor/helix (45833 stars)

### 1. What Helix is and why it matters

Helix is a modal terminal text editor in the Kakoune selection-first tradition: selections are
the primary editing primitive, every cursor is a one-range selection, and language intelligence
(tree-sitter syntax trees, LSP, DAP) is built in rather than bolted on through plugins. The
binary is `hx`, defined in `extras/helix/helix-term/Cargo.toml`:

```toml
[[bin]]
name = "hx"
path = "src/main.rs"
```

Industry treats Helix as a reference for two things: how to structure a large interactive
terminal application in Rust, and how to run a high-velocity open source project with a small
core team. The changelog for the 25.07 release records the scale of participation
(`extras/helix/CHANGELOG.md`): "This release saw changes from 195 contributors."

Scale indicators measured directly from the clone (revision `079a789`):

- 14 workspace members, 13 library or binary crates plus an `xtask` runner
  (`extras/helix/Cargo.toml`).
- 251 Rust source files totaling 107,479 lines of Rust outside `runtime/`.
- Per-crate line counts: helix-term 38,696; helix-core 20,086; helix-view 15,657;
  helix-lsp-types 9,906; helix-tui 7,623; helix-lsp 4,240; helix-stdx 2,652;
  helix-loader 2,087; helix-vcs 1,379; helix-dap 1,190; helix-dap-types 1,107;
  helix-event 1,094; xtask 850; helix-parsec 574.
- A 5,657 line `extras/helix/languages.toml` describing every supported language,
  220 bundled themes in `extras/helix/runtime/themes`, and 341 tree-sitter query
  directories in `extras/helix/runtime/queries`.

### 2. Repository layout

```text
extras/helix/
|-- Cargo.toml            workspace root, profiles, shared deps
|-- Cargo.lock            committed, releases build with --locked
|-- rust-toolchain.toml   pins channel 1.90.0 plus components
|-- rustfmt.toml          empty file: rustfmt defaults, on purpose
|-- languages.toml        language and grammar registry (5657 lines)
|-- theme.toml            default theme
|-- book/                 mdBook user manual (docs.helix-editor.com)
|-- docs/                 contributor docs: CONTRIBUTING, architecture, releases, vision
|-- contrib/              packaging assets: completions, .desktop, icons, appdata
|-- runtime/              grammars, queries, themes, tutor: shipped next to the binary
|-- tests/                cross-crate corpus data (indent/, query/)
|-- xtask/                ad-hoc task runner crate (docgen, query-check, ...)
|-- helix-core/           functional editing primitives
|-- helix-view/           editor state model (documents, views, registers)
|-- helix-term/           the hx binary: event loop, compositor, commands, UI
|-- helix-tui/            TUI widget/backend layer, forked from tui-rs
|-- helix-event/          hook/event system, debouncing, redraw control
|-- helix-lsp/ helix-lsp-types/  LSP client and protocol types
|-- helix-dap/ helix-dap-types/  DAP client and protocol types
|-- helix-loader/         config/grammar loading, build metadata
|-- helix-vcs/            diff providers (git via gix, feature-gated)
|-- helix-parsec/         zero-dependency parser combinators
|-- helix-stdx/           std extensions (paths, env, rope utils, faccess)
`-- .github/              workflows, composite action, templates, dependabot
```

The split is explained in `extras/helix/docs/architecture.md`, which states the design intent
crate by crate, for example:

```text
| helix-core      | Core editing primitives, functional.                             |
| helix-view      | UI abstractions for use in backends, imperative shell.           |
| helix-term      | Terminal UI                                                      |
```

Why this split works: `helix-core` is a pure functional layer (ropes, selections,
transactions) that can be tested without a terminal; `helix-view` is the imperative shell
holding editor state; `helix-term` is the only crate that knows about a real terminal. Protocol
types (`helix-lsp-types`, `helix-dap-types`) are separated from their clients so the huge,
mostly generated type definitions compile independently and can enforce stricter rules
(`extras/helix/helix-lsp-types/src/lib.rs` carries `#![forbid(unsafe_code)]`). Tiny leaf
crates like `helix-parsec` (574 lines, zero dependencies) keep compile graphs shallow.
The `helix-stdx` crate copies the rust-analyzer pattern of a project-local std extension,
which `extras/helix/docs/architecture.md` cites explicitly.

### 3. Cargo manifest practices

The root `extras/helix/Cargo.toml` uses resolver 2, a `default-members` entry so a bare
`cargo build` builds only the editor binary, and a `[workspace.package]` table that every
member inherits:

```toml
[workspace.package]
version = "25.7.1"
edition = "2021"
authors = ["Blaž Hrastnik <blaz@mxxn.io>"]
categories = ["editor"]
repository = "https://github.com/helix-editor/helix"
homepage = "https://helix-editor.com"
license = "MPL-2.0"
rust-version = "1.90"
```

Member manifests then contain only `version.workspace = true`, `edition.workspace = true`
and so on (see `extras/helix/helix-core/Cargo.toml`). Cross-crate dependency versions are
centralized in `[workspace.dependencies]` with a labeled dev-dependency section:

```toml
# dev dependencies
criterion = { version = "0.8", default-features = false, features = ["cargo_bench_support"] }
quickcheck = { version = "1", default-features = false }
```

Custom profiles are the standout practice. Release artifacts use an `opt` profile, and
integration tests get their own profile that optimizes only the hot crates so the suite runs
fast without paying full release compile times:

```toml
[profile.opt]
inherits = "release"
lto = "fat"
codegen-units = 1
strip = true
opt-level = 3

[profile.integration]
inherits = "test"
package.helix-core.opt-level = 2
package.helix-tui.opt-level = 2
package.helix-term.opt-level = 2
```

Feature flags gate real cost: `extras/helix/helix-term/Cargo.toml` has
`default = ["git"]`, `git = ["helix-vcs/git"]` (which turns on the heavy `gix` dependency in
`extras/helix/helix-vcs/Cargo.toml`: `git = ["gix"]` with `optional = true`), and
`integration = ["helix-event/integration_test"]` so test-only machinery never ships.
Platform-specific dependency tables replace runtime checks:

```toml
[target.'cfg(windows)'.dependencies]
crossterm = { version = "0.28", features = ["event-stream"] }

[target.'cfg(not(windows))'.dependencies]  # https://github.com/vorner/signal-hook/issues/100
signal-hook-tokio = { version = "0.4", features = ["futures-v0_3"] }
```

Unusual details worth copying: an exact-pin with a written justification in
`extras/helix/helix-core/Cargo.toml` (`unicode-width = "=0.1.12"` under a comment explaining
rendering breakage when installing without `--locked`); Debian packaging metadata inline as
`[package.metadata.deb]` in `extras/helix/helix-term/Cargo.toml`; `bench = false` on the
`helix-view` lib target so criterion owns benchmarking
(`extras/helix/helix-view/Cargo.toml`); and cargo aliases in
`extras/helix/.cargo/config.toml`:

```toml
[alias]
xtask = "run --package xtask --"
integration-test = "test --features integration --profile integration --workspace --test integration"
```

The same file passes `--cfg tokio_unstable` through `[target."cfg(all())"] rustflags` with a
nine-line comment explaining why `build.rustflags` would be silently overwritten by user
config; the flag unlocks `runtime::Handle::id` so parallel integration tests can keep
per-runtime globals separate. There are no `[lints]` tables anywhere in the workspace.

### 4. Formatting

`extras/helix/rustfmt.toml` is a zero-byte file. That is a deliberate statement: the presence
of the file pins the project to rustfmt defaults and prevents any parent directory or editor
override from changing them, while adding no unstable options that would require nightly.
`extras/helix/rust-toolchain.toml` guarantees every contributor formats with the same rustfmt:

```toml
[toolchain]
channel = "1.90.0"
components = ["rustfmt", "rust-src", "clippy"]
```

There is no `.editorconfig`. Non-Rust hygiene is handled by `extras/helix/.gitattributes`,
which normalizes line endings and assigns diff drivers per file type:

```text
*          text=auto
*.rs       text diff=rust
*.toml     text diff=toml
*.scm      text diff=scheme
```

The same file also curates GitHub language statistics
(`runtime/queries/**/*.scm linguist-language=Tree-sitter-Query`,
`tests/indent/** linguist-documentation`). TOML, markdown and query files have no dedicated
formatter; their consistency is maintained by review and by the generated-docs check
described in section 6.

### 5. Linting

Helix has no `clippy.toml` and no `[lints]` tables; the entire policy lives in one CI flag in
`extras/helix/.github/workflows/build.yml`:

```yaml
- name: Run cargo clippy
  run: cargo clippy --workspace --all-targets -- -D warnings
```

The philosophy is: default clippy level, zero tolerance. Everything clippy warns about by
default is a build failure across all targets (including tests and benches), and exceptions
are granted locally, never globally. The clone contains only 51 `clippy::` attribute
mentions across 107k lines, dominated by narrowly scoped allows:
13x `clippy::too_many_arguments`, 5x `clippy::type_complexity`,
4x `clippy::should_implement_trait`, each attached to a single item, for example
`#[allow(clippy::too_many_lines)]` directly on `Args::parse_args` in
`extras/helix/helix-term/src/args.rs`. Rustdoc is linted with the same severity in the same
job:

```yaml
- name: Run cargo doc
  run: cargo doc --no-deps --workspace --document-private-items
  env:
    RUSTDOCFLAGS: -D warnings
```

The custom check infrastructure is where Helix invests instead: the `xtask` crate
(`extras/helix/xtask/src/main.rs`) implements project-specific lints that no generic tool
could provide. `querycheck` compiles every tree-sitter query for every language against its
grammar, `indentcheck` runs the indent engine over a corpus in `extras/helix/tests/indent/`,
`theme-check` validates all 220 themes, and `docgen` regenerates command and language tables.
These run as first-class CI jobs (section 6), which is the real lesson: lint the artifacts
your project actually ships, not only the Rust.

### 6. CI/CD

Four workflows live in `extras/helix/.github/workflows`: `build.yml`, `release.yml`,
`gh-pages.yml`, `cachix.yml`, plus `dependabot.yml` and a composite action.

`build.yml` triggers on `pull_request`, pushes to `master`, `merge_group` (GitHub merge queue
is in active use), and a nightly cron (`00 01 * * *`). Concurrency cancels superseded PR runs
but never master runs:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.head_ref || github.run_id }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

Every workflow sets `permissions: contents: read` at the top and escalates per job only where
needed (`release.yml` grants `contents: write`, `id-token: write`, `attestations: write` only
to the publish job). Every job carries
`if: github.repository == 'helix-editor/helix' || github.event_name != 'schedule'` so forks
do not burn cron minutes. The test job has `timeout-minutes: 30` and a five-way OS matrix:

```yaml
matrix:
  os: [ubuntu-latest, macos-latest, windows-latest, ubuntu-24.04-arm, windows-11-arm]
```

Toolchain and caching are factored into a composite action,
`extras/helix/.github/actions/rust-setup/action.yml`, so all jobs share one definition. It
pins third-party actions by full commit SHA with a human-readable comment, uses
`Swatinem/rust-cache` with a `shared-key`, and adds a second cache layer for built
tree-sitter grammars keyed on the language registry, with a manual bust knob
(`GRAMMAR_CACHE_VERSION` env in `build.yml`):

```yaml
- uses: Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4 # v2.9.1
  with:
    shared-key: ${{ inputs.cache-key }}

- if: inputs.cache-grammars == 'true'
  uses: actions/cache@v5
  with:
    path: runtime/grammars
    key: ${{ runner.os }}-${{ runner.arch }}-stable-v${{ inputs.grammar-cache-version }}-tree-sitter-grammars-${{ hashFiles('languages.toml') }}
```

MSRV is enforced, not aspirational: `env: MSRV: "1.90"` at the top of `build.yml` and every
job installs exactly that toolchain. The `docs` job runs all xtask validators with
`if: always()` so one failure does not mask the others, then fails the build if generated
documentation is stale:

```yaml
- name: Check uncommitted documentation changes
  if: always()
  run: |
    git diff
    git diff-files --quiet \
      || (echo "Run 'cargo xtask docgen', commit the changes and push again" \
      && exit 1)
```

`gh-pages.yml` builds the mdBook and deploys versioned directories per tag plus a rolling
master build. `cachix.yml` publishes the Nix flake outputs on ubuntu and macos.
`extras/helix/.github/dependabot.yml` runs weekly for both `cargo` and `github-actions`
ecosystems, grouping minor and patch Rust bumps into one PR to cut review noise.

### 7. Testing

The layers, from smallest to largest:

- Unit tests inline in `#[cfg(test)] mod tests` throughout the crates, run by
  `cargo test --workspace` on all five CI OSes.
- Property tests via quickcheck where an invariant exists, for example the diff round-trip in
  `extras/helix/helix-core/src/diff.rs`:

```rust
quickcheck::quickcheck! {
    fn test_compare_ropes(a: String, b: String) -> bool {
        let mut old = Rope::from(a);
        let new = Rope::from(b);
        compare_ropes(&old, &new).apply(&mut old);
        old == new
    }
}
```

- Corpus tests: `extras/helix/helix-core/tests/indent.rs` replays real source files from
  `extras/helix/tests/indent/` through the indent engine; the same corpus feeds
  `cargo xtask indent-check`.
- Criterion benchmarks: `extras/helix/helix-view/benches/word_index.rs`, wired with
  `harness = false` and `required-features = ["bench"]`, where the `bench` feature exposes
  internals only for measurement (`extras/helix/helix-view/Cargo.toml`: "Exposes internals.
  Should not be enabled except for benchmarking.").

The crown jewel is end-to-end testing of the full TUI through the real event loop.
`extras/helix/helix-term/tests/integration.rs` gates everything behind
`#[cfg(feature = "integration")]`, and a smoke test reads like a spec:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn hello_world() -> anyhow::Result<()> {
    test(("#[\n|]#", "ihello world<esc>", "hello world#[|\n]#")).await?;
    Ok(())
}
```

Three pieces make that one-liner possible. First, a selection-annotation DSL in
`extras/helix/helix-core/src/test.rs`: `test::print` parses strings where `#[...|]#` marks
the primary selection, converting fixtures into `(String, Selection)` pairs; the editor's own
key-macro parser (`parse_macro`) turns `"ihello world<esc>"` into key events. Second, the
harness in `extras/helix/helix-term/tests/test/helpers.rs` constructs a real `Application`,
feeds events through a channel into `app.event_loop_until_idle(&mut rx_stream).await`, and
asserts on resulting document state; it includes a `LineFeedHandling` enum so the same
fixtures pass on Windows CRLF, and an `AppBuilder` builder for per-test config. Third,
test-only cheap infrastructure: the `integration_test` feature swaps the `runtime_local!`
macro in `extras/helix/helix-event/src/runtime.rs` from a plain static to a
per-tokio-runtime map keyed by `tokio::runtime::Id`, so parallel tests each get isolated
"globals". The suite runs under its own optimized profile via the
`cargo integration-test` alias, as a separate CI step on every OS in the matrix.

### 8. Error handling and API design

The pattern is textbook two-tier: `anyhow` at the application boundary, `thiserror` enums at
library boundaries. `extras/helix/helix-lsp/src/lib.rs` defines a crate `Result` alias and a
public error enum with `#[from]` conversions:

```rust
pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    Rpc(#[from] jsonrpc::Error),
    Parse(Box<dyn std::error::Error + Send + Sync>),
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    ...
    Other(#[from] anyhow::Error),
}
```

Similar enums exist in `extras/helix/helix-dap/src/lib.rs`,
`extras/helix/helix-view/src/document.rs` (`DocumentOpenError`), and
`extras/helix/helix-view/src/clipboard.rs`. The binary entry point
(`extras/helix/helix-term/src/main.rs`) shows disciplined exit-code handling: `main` is a
thin wrapper so the process can exit with a real code after destructors run:

```rust
fn main() -> Result<()> {
    let exit_code = main_impl()?;
    std::process::exit(exit_code);
}
```

Failures are graded, not uniform: unparsable CLI args are a hard error with
`.context("could not parse arguments")`; a missing config file silently falls back to
defaults via a targeted match arm
(`Err(ConfigLoadError::Error(err)) if err.kind() == std::io::ErrorKind::NotFound`); a
malformed config prints the error and waits for Enter before continuing with defaults; a
`BrokenPipe` from `--health` piped into `head` is deliberately swallowed. Panics are reserved
for unrecoverable states, and even then retried first:
`extras/helix/helix-term/src/application.rs` only panics on
"Failed to claim terminal" after 10 retries.

API-design details: newtypes with niche optimization in `extras/helix/helix-view/src/lib.rs`
(`pub struct DocumentId(NonZeroUsize);` under the comment "uses NonZeroUsize so
`Option<DocumentId>` use a byte rather than two"), with the extra constructor confined to
`#[cfg(test)]` and `pub(crate)`; slotmap `new_key_type!` for view keys; builder pattern in
the test harness (`AppBuilder` in `extras/helix/helix-term/tests/test/helpers.rs`); and
visibility used as documentation (`#[doc(hidden)] pub mod runtime` in
`extras/helix/helix-event/src/lib.rs` for macro plumbing that must be `pub` but is not API).

### 9. Deep Rust usage, ten cited examples

1. Trait plus blanket impl for composability: `extras/helix/helix-parsec/src/lib.rs` defines
   `trait Parser<'a> { type Output; fn parse(&self, input: &'a str) -> ParseResult<'a, Self::Output>; }`
   and a blanket `impl<'a, F, T> Parser<'a> for F where F: Fn(&'a str) -> ParseResult<'a, T>`,
   so every closure is a parser and combinators are ordinary higher-order functions.
2. Iterator-generic rendering API: the backend abstraction in
   `extras/helix/helix-tui/src/backend/mod.rs` takes cells as a generic iterator,
   `fn draw<'a, I>(&mut self, content: I) where I: Iterator<Item = (u16, u16, &'a Cell)>`,
   letting the terminal, crossterm, and test backends share one streaming protocol with no
   intermediate buffer allocation.
3. Precise lifetime bounds on zero-copy accessors:
   `extras/helix/helix-core/src/selection.rs` has
   `pub fn fragment<'a, 'b: 'a>(&'a self, text: RopeSlice<'b>) -> Cow<'b, str>`, tying the
   returned `Cow` to the text's lifetime rather than the selection's, and
   `fragments` returns `impl DoubleEndedIterator<Item = Cow<'a, str>> + ExactSizeIterator`.
4. Hand-rolled compressed Cow when profiling justifies it: `GraphemeStr<'a>` in
   `extras/helix/helix-core/src/graphemes.rs` packs a borrowed-or-owned string into
   `ptr: NonNull<u8>, len: u32` with the ownership bit stored as `const MASK_OWNED: u32 = 1 << 31`,
   with `unsafe` confined to `Deref` and `Drop`.
5. Lock-free config reloading: `extras/helix/helix-view/src/editor.rs` stores the syntax
   loader as `pub syn_loader: Arc<ArcSwap<syntax::Loader>>` and builds registers over
   `arc_swap::access::Map`, so readers on the render path never take a lock while `:config-reload`
   swaps the whole config atomically.
6. Declarative macros to express partial borrows: `extras/helix/helix-view/src/macros.rs`
   defines `current!`, `doc_mut!`, `view_mut!` because, per its module doc, functions taking
   `&mut self` would borrow the whole `Editor`; the macros expand to direct field accesses so
   the borrow checker sees disjoint borrows.
7. A typed event bus with an explicit soundness contract:
   `extras/helix/helix-event/src/registry.rs` keys hooks by `TypeId` and marks
   `register_hook` as `unsafe` with a written invariant ("`hook` must be totally generic over
   all lifetime parameters of `E`"), then wraps it in a safe `register_hook!` macro; the
   crate-level doc in `extras/helix/helix-event/src/lib.rs` explains the sync-hook versus
   `AsyncHook` split, debouncing, and frame locking.
8. Feature-swapped globals for test isolation: `runtime_local!` in
   `extras/helix/helix-event/src/runtime.rs` compiles to a plain static normally and, under
   `integration_test`, to a `parking_lot::RwLock<HashMap<tokio::runtime::Id, &'static T, ...>>`
   so concurrent integration tests cannot share state.
9. Platform handling through cfg-swapped imports rather than scattered branches:
   `extras/helix/helix-term/tests/test/helpers.rs` selects the whole event source per OS,
   `#[cfg(windows)] use crossterm::event::{Event, KeyEvent};` versus
   `#[cfg(not(windows))] use termina::event::{...}`, mirroring the per-target dependency
   tables in `extras/helix/helix-term/Cargo.toml`; `extras/helix/helix-stdx/src/faccess.rs`
   confines all Windows ACL `unsafe` behind a `mod imp` per platform.
10. Extension traits over foreign types: `RopeSliceExt<'a>` in
    `extras/helix/helix-stdx/src/rope.rs` adds `regex_input`, `starts_with`,
    grapheme-boundary helpers to ropey's `RopeSlice`, marking non-consuming builders with
    `#[must_use]`, so downstream code reads as if ropey natively supported cursor-based regex.
11. Persistent, invertible edits as an OT-style algebra:
    `extras/helix/helix-core/src/transaction.rs` models all edits as
    `enum Operation { Retain(usize), Delete(usize), Insert(Tendril) }` inside a `ChangeSet`
    that can be composed, inverted for undo, and mapped over positions with a six-variant
    `Assoc` policy; this single abstraction powers undo, LSP edits, and multi-cursor changes.
12. Compile-time command registry: `static_commands!` in
    `extras/helix/helix-term/src/commands.rs` declares each editor command once
    (`$name, $doc,`) and generates both the `const` items and
    `pub const STATIC_COMMAND_LIST: &'static [Self]`, which keymaps, the fuzzy palette, and
    `xtask docgen` all consume, so a command can never exist without a name and doc string.

### 10. Documentation practices

Documentation is split by audience. Users get an mdBook in `extras/helix/book/` deployed to
docs.helix-editor.com by `gh-pages.yml`, with `edit-url-template` in
`extras/helix/book/book.toml` linking every page to its source. Three pages under
`extras/helix/book/src/generated/` (`typable-cmd.md`, `static-cmd.md`, `lang-support.md`)
are machine-written by `cargo xtask docgen` from the command registry itself
(`extras/helix/xtask/src/docgen.rs` imports `helix_term::commands::TYPABLE_COMMAND_LIST`),
and CI fails if they drift from the code.

Contributors get `extras/helix/docs/`: `CONTRIBUTING.md` covers log-based debugging,
integration-test workflow, and a written MSRV policy ("We follow Firefox's MSRV policy"
listing the three places to update); `architecture.md` is a genuine orientation document
mapping crates to responsibilities and naming the core abstractions (Rope, Selection,
Transaction, Syntax); `releases.md` is a step-by-step release runbook; `vision.md` states
scope. Rustdoc is held to `-D warnings` including private items (section 6), and module-level
docs carry design rationale rather than restating signatures; the eleven-line doc comment on
`test::print` in `extras/helix/helix-core/src/test.rs` documents the selection DSL grammar
with examples and panics sections. Issue intake is structured:
`extras/helix/.github/ISSUE_TEMPLATE/bug_report.yaml` is a GitHub form with required
reproduction fields and a pre-formatted log section, labeled `C-bug` automatically, while
`enhancement.md` redirects feature ideas to Discussions.

### 11. Release and distribution

Versioning is CalVer, `YY.0M(.MICRO)`, encoded as SemVer for Cargo (25.07.1 becomes
`version = "25.7.1"` in `extras/helix/Cargo.toml`); `extras/helix/helix-loader/build.rs`
converts it back for display and embeds the git hash
(`cargo:rustc-env=VERSION_AND_GIT_HASH=...`), with a Nix fallback via
`HELIX_NIX_BUILD_REV`. The changelog is hand-curated per
`extras/helix/docs/releases.md`, and `extras/helix/CHANGELOG.md` opens with a comment
template listing the standing section order (Breaking changes, Features, Commands, ...).

`extras/helix/.github/workflows/release.yml` triggers on version tags, on
`patch/ci-release-*` branches, and on PRs touching itself; a `preview` env publishes
artifacts instead of a release when not on a real tag, so the pipeline is testable without
tagging. The `fetch-grammars` job downloads all tree-sitter grammars once and shares the
tarball with a six-target matrix (x86_64 and aarch64 for Linux, macOS, Windows) that builds
with `cargo build --profile opt --locked`. Linux intentionally builds on ubuntu-22.04 with a
comment warning that a newer image would raise the GLIBC floor. Outputs: tar.xz and zip
archives bundling the runtime and completions, an AppImage with zsync update metadata, and a
.deb produced by `cargo-deb` from the metadata in `extras/helix/helix-term/Cargo.toml`. The
publish job signs provenance with `actions/attest-build-provenance` before uploading.
Shell completions for bash, elvish, fish, nushell, and zsh are hand-maintained in
`extras/helix/contrib/completion/` (the CLI parser is also hand-rolled in
`extras/helix/helix-term/src/args.rs`, a plain `while let Some(arg) = argv.next()` loop over
a `#[derive(Default)] pub struct Args`), and the .deb installs them into the distribution's
completion directories. Tags are GPG-signed (`git tag -s` in the runbook), and Nix users get
cached builds through `cachix.yml`.

### 12. Lessons for quinjet

quinjet already exceeds Helix on lint strictness, deny/typos/taplo, coverage, miri, and
mutants. What Helix still teaches:

1. Adopt the integration profile trick: add
   `[profile.integration]` with `inherits = "test"` and per-package
   `package.quinjet.opt-level = 2` in Cargo.toml, plus an alias
   `integration-test = "test --features integration --profile integration --test integration"`
   in `.cargo/config.toml`, mirroring `extras/helix/Cargo.toml` and
   `extras/helix/.cargo/config.toml`; TUI-driving tests are slow at opt-level 0.
2. Build a key-sequence harness for the TUI: an `AppBuilder`, a channel of synthetic
   crossterm events, an `event_loop_until_idle` seam in the app, and fixture syntax for
   before/after state, modeled on `extras/helix/helix-term/tests/test/helpers.rs`; drive it
   from a single `tests/integration.rs` behind an `integration` cargo feature so release
   binaries carry no test hooks.
3. Generate reference docs from the command table and gate drift in CI: a small xtask (or a
   `docs` subcommand) that renders subcommand tables to markdown, then a workflow step that
   runs it and fails on `git diff-files --quiet`, exactly as in the `docs` job of
   `extras/helix/.github/workflows/build.yml`.
4. Split the OS matrix wide and cheap: `ubuntu-latest, macos-latest, windows-latest,
   ubuntu-24.04-arm, windows-11-arm` with `timeout-minutes` on the job, per
   `extras/helix/.github/workflows/build.yml`; a Git TUI has exactly the same
   terminal-and-filesystem portability risks as an editor.
5. Factor CI setup into a repo-local composite action
   (`.github/actions/rust-setup/action.yml`) with `Swatinem/rust-cache` `shared-key` inputs,
   and pin third-party actions by commit SHA with a version comment, as
   `extras/helix/.github/actions/rust-setup/action.yml` does.
6. Add `merge_group:` to the CI trigger list and turn on the GitHub merge queue; Helix pairs
   it with a `concurrency` group that cancels only PR runs
   (`extras/helix/.github/workflows/build.yml`).
7. Set default `permissions: contents: read` in every workflow and escalate per job; sign
   release artifacts with `actions/attest-build-provenance@v4` in the publish job, per
   `extras/helix/.github/workflows/release.yml`.
8. Test the release pipeline without tagging: a `preview` env expression
   (`!startsWith(github.ref, 'refs/tags/')`) that uploads artifacts instead of creating a
   release, plus a `pull_request: paths: ['.github/workflows/release.yml']` trigger, from
   `extras/helix/.github/workflows/release.yml`.
9. Encode grades of failure at startup: match on `io::ErrorKind::NotFound` for a missing
   config versus a hard error for a bad one, tolerate `BrokenPipe` on informational output,
   and keep `fn main` a thin `std::process::exit(main_impl()?)` wrapper, per
   `extras/helix/helix-term/src/main.rs`.
10. Use quickcheck (already a dev-dependency pattern worth copying from
    `extras/helix/Cargo.toml`) for round-trip invariants quinjet has in abundance: diff
    apply/revert, hunk splitting, ref name parsing, following
    `extras/helix/helix-core/src/diff.rs`.
11. Ship packaging from the manifest: `[package.metadata.deb]` plus `cargo-deb --no-build` in
    the release workflow, installing completions into
    `/usr/share/bash-completion/completions` and friends, per
    `extras/helix/helix-term/Cargo.toml`.
12. Add `.gitattributes` with `* text=auto` and per-extension diff drivers
    (`*.rs diff=rust`, `*.toml diff=toml`) as in `extras/helix/.gitattributes`; it is the
    cheapest cross-platform CRLF insurance a TUI repo can buy.
13. Group dependabot minor/patch cargo updates
    (`groups: rust-dependencies: update-types: [minor, patch]`) and watch
    `github-actions` weekly, per `extras/helix/.github/dependabot.yml`.
14. When quinjet grows internal IDs, copy the `NonZeroUsize` newtype with the one-line
    rationale comment from `extras/helix/helix-view/src/lib.rs`, and keep test-only
    constructors `#[cfg(test)] pub(crate)`.

---

## sharkdp/fd (44095 stars)

### 1. What the project is and why it matters

fd is a user-friendly, parallel replacement for the Unix `find` command. Its manifest describes it plainly (extras/fd/Cargo.toml):

```toml
[package]
name = "fd-find"
description = "fd is a simple, fast and user-friendly alternative to find."
version = "10.4.2"
edition= "2024"
rust-version = "1.90.0"
```

Industry uses fd because it is dramatically faster than `find` (the README at extras/fd/README.md documents a hyperfine benchmark where fd is roughly 23 times faster than `find -iregex` on a 4-million-file home directory), because it respects `.gitignore` by default, and because it composes well with `xargs`, fzf, and editor tooling. It is packaged in essentially every Linux distribution, Homebrew, Scoop, and winget.

Measured scale indicators from the clone:

- Single crate, single binary. There is no workspace; `[[bin]] name = "fd"` points at `src/main.rs` (extras/fd/Cargo.toml).
- 5,059 lines of Rust in `src/` across 22 files, plus 3,222 lines of test code (extras/fd/tests/tests.rs is 2,878 lines, extras/fd/tests/testenv/mod.rs is 344 lines). Total: 8,281 lines of Rust.
- 130 locked packages in extras/fd/Cargo.lock, from only about 20 direct dependencies.
- 101 `#[test]` functions in the integration suite alone, plus dozens of inline unit tests and macro-generated cases.
- One CI workflow file (extras/fd/.github/workflows/CICD.yml) covering lint, format, MSRV, a 14-target build matrix, and release publishing.

The headline lesson of this chapter: a tool used by millions can be a small, single-crate codebase if the code is disciplined, the test harness exercises the real binary, and the release pipeline is fully automated.

### 2. Repository layout

The real top-level tree (from `ls` of extras/fd):

```text
fd/
|-- .cargo/
|   `-- config.toml          target-specific rustflags (static CRT on MSVC)
|-- .github/
|   |-- ISSUE_TEMPLATE/      bug_report.yaml, feature_request.md, question.md, config.yml
|   |-- workflows/
|   |   `-- CICD.yml         the single CI/CD pipeline
|   |-- dependabot.yml
|   `-- FUNDING.yml
|-- contrib/
|   `-- completion/          hand-written zsh completion (_fd) and fdfind aliases
|-- doc/
|   |-- fd.1                 hand-maintained man page (587 lines of roff)
|   |-- release-checklist.md
|   |-- screencast.sh        script that regenerates the README demo SVG
|   `-- sponsors.md
|-- scripts/
|   |-- create-deb.sh        builds Debian packages in CI
|   |-- update-help.awk      syncs `fd -h` output into README.md
|   `-- version-bump.sh      automates the release version bump
|-- src/
|   |-- main.rs              entry point, config construction, module declarations
|   |-- cli.rs               clap derive definitions (971 lines)
|   |-- walk.rs              parallel traversal engine (744 lines)
|   |-- exec/                --exec / --exec-batch subsystem (mod, command, job)
|   |-- filter/              size, time, owner filters (mod, size, time, owner)
|   |-- fmt/                 --format template engine (mod, input)
|   |-- config.rs, dir_entry.rs, output.rs, sanitize.rs, hyperlink.rs,
|   |-- filesystem.rs, filetypes.rs, exit_codes.rs, error.rs, regex_helper.rs
|-- tests/
|   |-- testenv/mod.rs       reusable end-to-end harness
|   `-- tests.rs             2,878 lines of black-box CLI tests
|-- Cargo.toml, Cargo.lock, Cross.toml, Makefile, rustfmt.toml
|-- CHANGELOG.md, CONTRIBUTING.md, SECURITY.md, README.md
`-- LICENSE-APACHE, LICENSE-MIT
```

Why this split works:

- `src/` is flat where the domain is flat (one file per concern: output, sanitize, hyperlink) and nested only where a subsystem has real internal structure (`exec/`, `filter/`, `fmt/`). No file except `cli.rs` and `walk.rs` exceeds 300 lines.
- Everything that supports distribution but is not code lives in named top-level directories: `doc/` for the man page and release docs, `contrib/` for shell-specific completion files that cannot be generated, `scripts/` for release mechanics. CI can `cp doc/fd.1` and `bash scripts/create-deb.sh` without guessing.
- The test harness lives in `tests/testenv/mod.rs` next to the single integration test binary, so the entire black-box surface is compiled once. A note in fd's history: keeping one integration test crate instead of many files avoids relinking the binary per test file.

### 3. Cargo manifest practices

extras/fd/Cargo.toml is a masterclass in single-crate manifest hygiene.

Dependency organization. Simple version requirements are one-liners; anything needing features gets its own table:

```toml
[dependencies.clap]
version = "4.6.1"
features = ["suggestions", "color", "wrap_help", "cargo", "derive"]

[dependencies.lscolors]
version = "0.21"
default-features = false
features = ["nu-ansi-term"]
```

Platform-conditional dependencies keep Unix-only crates off Windows builds entirely:

```toml
[target.'cfg(unix)'.dependencies]
nix = { version = "0.31.1", default-features = false, features = ["signal", "user", "hostname"] }
```

The jemalloc dependency has the most elaborate cfg expression in the file, and its comment explains why and cross-references the code that must stay in sync:

```toml
# FIXME: Re-enable jemalloc on macOS
# jemalloc is currently disabled on macOS due to a bug in jemalloc in combination with macOS
# Catalina. See https://github.com/sharkdp/fd/issues/498 for details.
# This has to be kept in sync with src/main.rs where the allocator for
# the program is set.
[target.'cfg(all(not(windows), not(target_os = "android"), not(target_os = "macos"), ...))'.dependencies]
tikv-jemallocator = {version = "0.7.0", optional = true}
```

Feature flags. Features are additive capability switches, not configuration:

```toml
[features]
use-jemalloc = ["tikv-jemallocator"]
completions = ["clap_complete"]
base = ["use-jemalloc"]
default = ["completions"]
```

`clap_complete` is optional and only pulled in by the `completions` feature; `src/main.rs` guards the whole completion path with `#[cfg(feature = "completions")]`.

Profiles. The dev profile is tuned for compile speed without losing backtraces, and dependencies skip debug info entirely:

```toml
[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.debugging]
inherits = "dev"
debug = true

[profile.release]
lto = true
strip = true
codegen-units = 1
```

The custom `debugging` profile is the escape hatch: full debug info on demand (`cargo build --profile debugging`) without slowing everyday builds.

MSRV and edition. `rust-version = "1.90.0"` and `edition= "2024"` sit in `[package]`, and section 6 shows how CI reads the MSRV out of the manifest so it is declared exactly once.

Unusual extras. The manifest carries `[package.metadata.binstall]` so `cargo binstall fd-find` fetches the prebuilt release archive instead of compiling:

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }.{ archive-format }"
bin-dir = "{ bin }-v{ version }-{ target }/{ bin }{ binary-ext }"
pkg-fmt = "tgz"
```

with per-target overrides switching Windows targets to `zip`. There is also `exclude = ["/benchmarks/*"]` to keep benchmark fixtures out of the published crate, and extras/fd/.cargo/config.toml statically links the MSVC C runtime so the Windows EXE has no DLL dependency:

```toml
# On Windows MSVC, statically link the C runtime so that the resulting EXE does
# not depend on the vcruntime DLL.
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

Cross-compilation quirks live in extras/fd/Cross.toml, passing `JEMALLOC_SYS_WITH_LG_PAGE=16` through to aarch64 containers to fix a page-size bug referenced by issue number.

### 4. Formatting

extras/fd/rustfmt.toml is a single line:

```toml
# Defaults are used
```

That one line is a deliberate practice, not an omission. The file exists so that editors and CI agree there is a rustfmt configuration, and its content documents the policy: stock rustfmt, no overrides, no style debates. CI enforces it with `cargo fmt -- --check` (the `ensure_cargo_fmt` job in extras/fd/.github/workflows/CICD.yml).

There is no `.editorconfig` and no formatter for YAML or Markdown; the non-Rust surface is small enough that review covers it. The only formatting-adjacent config for non-Rust files is extras/fd/doc/.gitattributes, which marks the generated screencast as vendored so it does not pollute language statistics:

```text
* linguist-vendored
```

### 5. Linting

fd's linting setup is minimal and centralized in CI rather than in the manifest. There is no `clippy.toml`, no `[lints]` table, and no crate-level `#![deny(...)]` attributes. The wall is a single CI invocation (extras/fd/.github/workflows/CICD.yml):

```yaml
  lint_check:
    name: Ensure 'cargo clippy' has no warnings
    steps:
    - run: cargo clippy --all-targets --all-features -- -Dwarnings
```

and a second clippy run on the MSRV toolchain (see section 6), whose step name states the reason: "Run clippy (on minimum supported rust version to prevent warnings we can't fix)". Running clippy on both stable and the MSRV catches lints that only exist on one of them.

The philosophy is default-lint-set, zero-warnings, with allows applied surgically at the smallest scope and always justified. The entire `src/` tree contains exactly one clippy allow (extras/fd/src/walk.rs):

```rust
/// The Worker threads can result in a valid entry having PathBuf or an error.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum WorkerResult {
    // Errors should be rare, so it's probably better to allow large_enum_variant than
    // to box the Entry variant
    Entry(DirEntry),
    Error(ignore::Error),
}
```

The comment explains the performance reasoning behind overriding the lint. The tests have one more (`#[allow(clippy::let_and_return)]` in extras/fd/tests/tests.rs, where a cfg(windows) block mutates the binding in between). Conditional-compilation warts are handled with targeted `cfg_attr`, for example in extras/fd/src/main.rs:

```rust
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut should_warn = pattern.contains('/');
```

and in extras/fd/src/config.rs a field used only on some platforms carries `#[cfg_attr(not(unix), allow(unused))]`. There is no custom lint infrastructure; the check surface is clippy plus `cargo fmt --check` plus the compiler under `-Dwarnings`.

### 6. CI/CD

There is exactly one workflow, extras/fd/.github/workflows/CICD.yml, which is both CI and CD. Triggers:

```yaml
on:
  workflow_dispatch:
  pull_request:
  push:
    branches:
      - master
    tags:
      - '*'
```

Security hardening at the top level:

```yaml
permissions:
  contents: read
```

Every `actions/checkout` invocation adds `persist-credentials: false`, so the checked-out tree never retains a token. Only the `build` job escalates, and only for what it needs:

```yaml
    permissions:
      id-token: write
      contents: write
      attestations: write
```

Jobs:

1. `crate_metadata` extracts name, version, maintainer, homepage, and MSRV from `cargo metadata --no-deps --format-version 1 | jq ...` and publishes them as job outputs. This makes Cargo.toml the single source of truth: the MSRV job and the packaging steps all read these outputs instead of duplicating constants.
2. `ensure_cargo_fmt` runs `cargo fmt -- --check` on stable.
3. `lint_check` runs `cargo clippy --all-targets --all-features -- -Dwarnings`.
4. `min_version` installs the exact MSRV toolchain via `dtolnay/rust-toolchain@master` with `toolchain: ${{ needs.crate_metadata.outputs.msrv }}` and runs both clippy and `cargo test --locked` on it.
5. `build` is a 14-entry matrix with `fail-fast: false` covering aarch64/arm/i686/x86_64 crossed with gnu/musl Linux (via cross), both macOS architectures, and three Windows toolchains including `windows-11-arm`:

   ```yaml
          - { target: aarch64-unknown-linux-gnu   , os: ubuntu-24.04, use-cross: true }
          - { target: x86_64-apple-darwin         , os: macos-26-intel                }
          - { target: aarch64-pc-windows-msvc     , os: windows-11-arm                }
   ```

   The build command is selected with an expression, `BUILD_CMD: "${{ matrix.job.use-cross && 'cross' || 'cargo' }}"`, and every cargo invocation passes `--locked`. Tests run on every target; for emulated ARM targets a step narrows the scope to `--bin=fd` because full integration tests are impractical under qemu. The job then runs `make completions`, assembles a tarball containing the binary, README, licenses, changelog, man page, and completions, and calls `bash scripts/create-deb.sh` on Ubuntu runners to build Debian packages (which also install `fdfind` symlinks for Debian's binary rename).
6. `winget` publishes to the Windows package manager on version tags using a token-scoped community action.

Action pinning is tiered: first-party and toolchain actions are pinned by tag (`actions/checkout@v7.0.1`, `dtolnay/rust-toolchain@stable`), while every third-party action that touches artifacts or credentials is pinned by full commit SHA with a version comment:

```yaml
      uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7
      uses: actions/attest@508db95dd578ae2727ebd6217d5ba78e4fbda05d # v4
      uses: softprops/action-gh-release@3d0d9888cb7fd7b750713d6e236d1fcb99157228 # v3.0.2
```

Even the `cross` binary is pinned (`cross_version: "v0.2.5"`) and downloaded with `gh release download` rather than installed from a floating source. Release artifacts are attested with `actions/attest` (build provenance) before upload, gated on a regex check of the ref:

```yaml
        unset IS_RELEASE ; if [[ $GITHUB_REF =~ ^refs/tags/v[0-9].* ]]; then IS_RELEASE='true' ; fi
```

Notable absences, both deliberate: there is no cargo/sccache caching action anywhere in the workflow (the crate is small enough that clean builds are cheap and cache poisoning is not a risk worth taking on a release pipeline), and there is no merge queue configuration (`merge_group` trigger absent). Dependency freshness is handled by extras/fd/.github/dependabot.yml, which adds the newer `cooldown` setting so that just-released versions age for a week before being proposed:

```yaml
  - package-ecosystem: "cargo"
    schedule:
      interval: "monthly"
    cooldown:
      default-days: 7
  - package-ecosystem: "github-actions"
    schedule:
      interval: "daily"
    cooldown:
      default-days: 7
```

### 7. Testing

fd's testing story has two clean layers.

Unit tests live inline, in `#[cfg(test)] mod tests` blocks inside the file they test: parsing in extras/fd/src/filter/size.rs and extras/fd/src/filter/owner.rs, template parsing in extras/fd/src/fmt/mod.rs, path helpers in extras/fd/src/filesystem.rs, sanitization in extras/fd/src/sanitize.rs, exit-code merging in extras/fd/src/exit_codes.rs. Table-driven cases are generated with local `macro_rules!` so each row is an individually named, individually reportable test:

```rust
    gen_size_filter_parse_test! {
        byte_plus:                ("+1b",     SizeFilter::Min(1)),
        kilo_plus:                ("+1k",     SizeFilter::Min(1000)),
        kibi_plus:                ("+1ki",    SizeFilter::Min(1024)),
        ...
    }
```

(extras/fd/src/filter/size.rs; the same pattern appears as `owner_tests!` and `func_tests!` elsewhere). Time-dependent logic is made deterministic with a cfg(test)-only clock in extras/fd/src/filter/time.rs:

```rust
#[cfg(test)]
thread_local! {
    static TESTTIME: std::cell::RefCell<Option<Zoned>> = None.into();
}

/// This allows us to set a specific time when running tests
#[cfg(test)]
fn now() -> Zoned {
    TESTTIME.with_borrow(|reftime| reftime.as_ref().cloned().unwrap_or_else(Zoned::now))
}
```

Integration tests are pure black-box: they run the compiled `fd` binary as a subprocess. The harness (extras/fd/tests/testenv/mod.rs) builds a `TestEnv` that creates a tempdir fixture with a fake `.git` directory (so gitignore semantics activate), `.fdignore` and `.gitignore` files, and platform-appropriate symlinks, then locates the binary through Cargo's own mechanism:

```rust
fn find_fd_exe() -> PathBuf {
    // Read the location of the fd executable from the environment
    PathBuf::from(env::var("CARGO_BIN_EXE_fd").unwrap_or(env!("CARGO_BIN_EXE_fd").to_string()))
}
```

The harness isolates environment state per test (`cmd.env("LS_COLORS", "")`, and a temp `XDG_CONFIG_HOME` when a global ignore file is under test), normalizes output (sorting lines, mapping `/` to the platform separator, rendering `\0` visibly as `NULL`), and produces readable failures by diffing expected against actual with the `diff` crate:

```rust
    let diff_text = diff::lines(expected, actual)
        .into_iter()
        .map(|diff| match diff {
            diff::Result::Left(l) => format!("-{l}"),
            diff::Result::Both(l, _) => format!(" {l}"),
            diff::Result::Right(r) => format!("+{r}"),
        })
```

On top of the harness, extras/fd/tests/tests.rs asserts stdout content, stderr content, and exit status for essentially every flag. Two patterns deserve special mention. First, `test-case` parameterization for the flag-override matrix, proving that each negating flag exactly cancels its counterpart:

```rust
#[test_case("--hidden", &["--no-hidden"] ; "hidden")]
#[test_case("--no-ignore", &["--ignore"] ; "no-ignore")]
#[test_case("-uu", &["--ignore", "--no-hidden"] ; "uu")]
fn test_opposing(flag: &str, opposing_flags: &[&str]) {
```

Second, hostile-input coverage: `test_invalid_utf8` creates a file with a raw `\xFE` byte in its name and asserts the lossy rendering, and `test_hyperlink` asserts the exact OSC 8 escape sequence including the hostname. Tests that depend on OS capabilities are guarded (`#[cfg(unix)]`, `#[cfg(target_os = "linux")]`) and some even probe the environment at runtime, like `test_file_system_boundaries` skipping itself when `/dev/null` shares a device with `/`.

What fd does not have in-repo: no snapshot-testing crate (the normalize-and-diff harness fills that role), no fuzzing targets, no property testing, and no criterion benchmarks. Performance benchmarking lives in a separate repository (`fd-benchmarks`, linked from extras/fd/README.md) driven by hyperfine, and the manifest excludes `/benchmarks/*` from publication.

### 8. Error handling and API design

fd is a binary crate, so it standardizes on `anyhow` rather than structured error enums. `run()` returns `anyhow::Result<ExitCode>` and errors are built at the point of failure with actionable, user-facing text (extras/fd/src/main.rs):

```rust
        env::set_current_dir(base_directory).with_context(|| {
            format!(
                "Could not set '{}' as the current working directory",
                base_directory.to_string_lossy()
            )
        })?;
```

Error messages teach: the regex build failure appends a note about `--fixed-strings`, `--exact`, and `--glob`; the path-separator diagnostic prints two copy-pastable alternative commands. Domain parsing failures happen at the clap boundary via `value_parser = SizeFilter::from_string` and `value_parser = OwnerFilter::from_string` (extras/fd/src/cli.rs), so invalid input never reaches program logic.

Exit codes are a first-class type (extras/fd/src/exit_codes.rs), not scattered integers:

```rust
pub enum ExitCode {
    Success,
    HasResults(bool),
    GeneralError,
    KilledBySigint,
}

impl From<ExitCode> for i32 {
    fn from(code: ExitCode) -> Self {
        match code {
            ExitCode::Success => 0,
            ExitCode::HasResults(has_results) => !has_results as i32,
            ExitCode::GeneralError => 1,
            ExitCode::KilledBySigint => 130,
        }
    }
}
```

`ExitCode::exit(self) -> !` also re-raises SIGINT after restoring the default handler so callers observe a genuine signal death, and `merge_exitcodes(impl IntoIterator<Item = ExitCode>)` folds the results of parallel `--exec` jobs. `main()` is a thin adapter: run, print `{err:#}` through the sanitizing `print_error`, exit with `GeneralError`.

Panic policy: `unwrap()` is confined to invariants (mutex poisoning, joins on scoped threads) and `unreachable!` carries an explanation of why the branch is impossible (extras/fd/src/walk.rs). `debug_assert!` documents parser postconditions in extras/fd/src/fmt/mod.rs.

API design within the crate is deliberate even without external consumers. `Config` (extras/fd/src/config.rs) is a plain struct with a doc comment on every field, constructed once in `construct_config` and passed by reference everywhere. Visibility is minimal: `PathUrl` is `pub(crate)` (extras/fd/src/hyperlink.rs), the `Check<T>` enum inside the owner filter is private, and CLI struct fields that exist only as override targets are private unit types. `TestEnv` uses consuming builder methods (`normalize_line`, `global_ignore_file`) with struct-update syntax. `OwnerFilter::filter_ignore` turns a no-op filter into `None` so downstream code can use plain `Option` combinators.

### 9. Deep Rust usage

Ten-plus concrete idioms, each cited:

1. Lazy per-entry memoization with `OnceCell`. `DirEntry` caches metadata and color style so a syscall and a style lookup happen at most once per entry, without `mut` methods (extras/fd/src/dir_entry.rs):

   ```rust
   pub struct DirEntry {
    inner: DirEntryInner,
    metadata: OnceCell<Option<Metadata>>,
    style: OnceCell<Option<Style>>,
   }
   ...
    pub fn metadata(&self) -> Option<&Metadata> {
        self.metadata
            .get_or_init(|| match &self.inner { ... })
            .as_ref()
    }
   ```

2. `OnceLock` statics for compile-once machinery: the size-filter regex (`static SIZE_CAPTURES: OnceLock<Regex>` in extras/fd/src/filter/size.rs), the aho-corasick placeholder automaton (`static PLACEHOLDERS: OnceLock<AhoCorasick>` in extras/fd/src/fmt/mod.rs), and the cached hostname in extras/fd/src/hyperlink.rs.

3. Zero-copy with `Cow` on the hot path. The per-entry match string borrows the filename and only allocates when `--full-path` forces a join (extras/fd/src/walk.rs):

   ```rust
   fn search_str_for_entry<'a>(
    entry_path: &'a std::path::Path,
    full_path_base: Option<&std::path::Path>,
   ) -> Cow<'a, OsStr> {
   ```

   The same pattern appears in `osstr_to_bytes` (extras/fd/src/filesystem.rs), `replace_separator` (extras/fd/src/fmt/mod.rs), and `sanitize_for_terminal` (extras/fd/src/sanitize.rs), which returns `Cow::Borrowed` when nothing needs escaping.

4. Structured concurrency with `thread::scope`. Both the sender/receiver pair and the `--exec` job pool borrow `&self` and `&Config` without `Arc`-wrapping the world (extras/fd/src/walk.rs):

   ```rust
        let exit_code = thread::scope(|scope| {
            // Spawn the receiver thread(s)
            let receiver = scope.spawn(|| self.receive(rx));
            self.spawn_senders(walker, tx);
            receiver.join().unwrap()
        });
   ```

5. Backpressure-aware batched channels. Instead of one channel send per file, workers accumulate results in a `Batch` (an `Arc<Mutex<Option<Vec<WorkerResult>>>>`) and send the handle once per batch; the receiver drains it by `take()`-ing the Vec through `IntoIterator`. The channel itself is `bounded(2 * config.threads)`, and the batch limit drops to 1 when results feed parallel `--exec` receivers, "to evenly distribute work" (extras/fd/src/walk.rs, `BatchSender::send` and `spawn_senders`).

6. Two-mode output buffering as a tiny state machine. `ReceiverBuffer<'a, W: Write>` starts in `Buffering` (so fast searches print sorted) and flips to `Streaming` on a deadline via `rx.recv_deadline(self.deadline)`; the mode enum plus `stream()`/`stop()` transitions make the policy explicit (extras/fd/src/walk.rs). Being generic over `W: Write` keeps it unit-testable and lets production pass `BufWriter<StdoutLock>`.

7. Cooperative cancellation with atomics and a double Ctrl-C escape hatch (extras/fd/src/walk.rs):

   ```rust
            ctrlc::set_handler(move || {
                quit_flag.store(true, Ordering::Relaxed);

                if interrupt_flag.fetch_or(true, Ordering::Relaxed) {
                    // Ctrl-C has been pressed twice, exit NOW
                    ExitCode::KilledBySigint.exit();
                }
            })
   ```

   Relaxed ordering is correct here because the flags are pure signals with no dependent data, and the code does not pretend otherwise.

8. The clap negating-flags pattern. Every boolean flag gets a hidden opposite whose field type is the unit type, so it occupies no state but participates in `overrides_with` resolution; combined with `args_override_self = true`, "last flag wins" works for scripts and aliases (extras/fd/src/cli.rs):

   ```rust
    /// Overrides --hidden
    #[arg(long, overrides_with = "hidden", hide = true, action = ArgAction::SetTrue)]
    no_hidden: (),
   ```

   The same file drops down from derive to the imperative API exactly where derive cannot express the requirement, implementing `clap::FromArgMatches` and `clap::Args` by hand for the `--exec` group because "there isn't a derive api for getting grouped values yet".

9. Semantic analysis of user regexes via `regex-syntax` HIR. Smart-case does not naively scan the pattern string for uppercase; it parses the pattern and recursively walks the HIR so `\Acargo` and `carg\x6F` are correctly judged lowercase (extras/fd/src/regex_helper.rs):

   ```rust
        HirKind::Capture(Capture { sub, .. }) | HirKind::Repetition(Repetition { sub, .. }) => {
            hir_has_uppercase_char(sub)
        }
        HirKind::Concat(hirs) | HirKind::Alternation(hirs) => {
            hirs.iter().any(hir_has_uppercase_char)
        }
   ```

10. A one-unsafe-block policy. The only `unsafe` in `src/` is the POSIX-mandated dance of restoring the default SIGINT handler and re-raising, in extras/fd/src/exit_codes.rs; everything else, including all path and byte handling, is safe code.

11. Platform handling as paired total functions rather than scattered cfg blocks: `is_socket`, `is_pipe`, `is_block_device`, and `osstr_to_bytes` each have a Unix and a Windows definition with identical signatures (extras/fd/src/filesystem.rs), so call sites contain zero conditional compilation. Where cfg must appear inline, it is expression-level and justified, like the jemalloc `#[global_allocator]` gate in extras/fd/src/main.rs that mirrors Cargo.toml.

12. Edition 2024 let-chains used for flat control flow throughout, for example (extras/fd/src/walk.rs):

    ```rust
                            if let Some(max_results) = self.config.max_results
                                && self.num_results >= max_results
                            {
                                return self.stop();
                            }
    ```

13. Micro-attention where it matters: `#[cold]` on the completions printer (extras/fd/src/main.rs), `#[inline]` on comparison impls and tiny accessors (extras/fd/src/dir_entry.rs), `NonZeroUsize` for the thread count with `available_parallelism().min(64)` capping startup overhead (extras/fd/src/cli.rs), and byte-regexes (`regex::bytes`) end to end so non-UTF-8 filenames are first-class.

14. Security-minded output: extras/fd/src/sanitize.rs escapes C0/C1 controls, bidi overrides, and zero-width characters only when stdout is a TTY, with unit tests named after the attacks they block (`strips_osc52_clipboard_payload`, `strips_bidi_overrides_and_zero_width`), while extras/fd/src/output.rs still writes raw bytes to pipes so downstream tools receive filenames intact.

### 10. Documentation practices

- Rustdoc is used for maintainers, not for docs.rs (a binary crate has no API consumers): every `Config` field has a `///` line (extras/fd/src/config.rs), non-obvious functions carry doc comments that explain rationale and link issues (the 20-line comment on `ensure_search_pattern_is_not_a_path` in extras/fd/src/main.rs reads like a design note), and extras/fd/src/sanitize.rs opens with a `//!` module doc: "TTY-output sanitization to prevent terminal escape injection via filenames."
- The user manual is the hand-maintained man page extras/fd/doc/fd.1 (587 lines of roff) plus a 800-line README with a troubleshooting section that the bug-report template points at. The README's help output is kept honest mechanically: extras/fd/scripts/update-help.awk re-runs `cargo run --release --quiet -- -h` and splices the result into the README's fenced block.
- extras/fd/CONTRIBUTING.md sets pull-request expectations, requires an entry in the "Upcoming release" section of the changelog with the exact format `- Short description of what has been changed, see #123 (@user)`, and asks contributors to open an issue before a PR.
- extras/fd/SECURITY.md defines a private vulnerability-reporting path via GitHub advisories with explicit confidentiality expectations.
- Issue templates: extras/fd/.github/ISSUE_TEMPLATE/bug_report.yaml is a structured GitHub form with a required checkbox ("I have read the troubleshooting section and still think this is a bug"), a required version input, and a required OS textarea rendered as shell; feature requests and questions get lighter Markdown templates, and config.yml keeps blank issues enabled. There is no PR template; CONTRIBUTING carries that weight.
- There is no ARCHITECTURE.md; at 5k lines the module names and doc comments are the architecture document.

### 11. Release and distribution

Versioning is semver on the crate (`10.4.2` at extras/fd/Cargo.toml), tags are `vX.Y.Z`, and the changelog is the source of release notes. extras/fd/CHANGELOG.md keeps a permanent `# Unreleased` section with `## Features`, `## Bugfixes`, `## Changes`, `## Other` subsections; every entry credits the contributor and cites the issue or PR number. MSRV bumps are announced as changelog entries ("Minimum required rust version has been increased to 1.90.0").

The release process is a documented checklist plus scripts:

- extras/fd/doc/release-checklist.md is a copy-pasteable checklist covering version bump, README/MSRV sync, `cargo publish --dry-run`, tagging, verifying binary deployment, and post-release changelog scaffolding.
- extras/fd/scripts/version-bump.sh automates the mechanical part: creates a `release-$version` branch, seds the version into Cargo.toml, updates the MSRV note in the README, and renames the changelog heading.
- Pushing the tag triggers the CD half of extras/fd/.github/workflows/CICD.yml: 14 target archives (tar.gz/zip with binary, man page, completions, licenses, changelog inside), Debian packages from extras/fd/scripts/create-deb.sh (including musl variants with correct `Conflicts:` metadata and `fdfind` alias symlinks for Debian), provenance attestation via actions/attest, upload to the GitHub release, and winget publication.

Completions and man page distribution is handled by the extras/fd/Makefile: generated completions come from the binary itself (`$(EXE) --gen-completions bash > $@`), the zsh completion is a hand-written file copied from extras/fd/contrib/completion/_fd, and `make install` places binary, completions for bash/fish/zsh, and the man page into FHS paths. Runtime generation is also a user feature: `fd --gen-completions <shell>` works on any installed binary because clap_complete ships in the default `completions` feature. Finally, the binstall metadata in Cargo.toml (section 3) makes `cargo binstall` a first-class install path.

### 12. Lessons for quinjet

quinjet already exceeds fd on lint strictness, cargo-deny, typos, coverage, miri, and mutants. What fd still teaches, with exact mechanisms:

1. Declare and enforce an MSRV from one source of truth. Add `rust-version = "..."` to Cargo.toml, then add a CI job pair modeled on fd's: a `crate_metadata` job that runs `cargo metadata --no-deps --format-version 1 | jq -r '"msrv=" + .packages[0].rust_version'` into `$GITHUB_OUTPUT`, and a `min_version` job using `dtolnay/rust-toolchain@master` with `toolchain: ${{ needs.crate_metadata.outputs.msrv }}` running `cargo clippy --locked --all-targets` and `cargo test --locked` (extras/fd/.github/workflows/CICD.yml).
2. Harden workflows the fd way: top-level `permissions: contents: read`, `persist-credentials: false` on every `actions/checkout`, per-job permission escalation only where needed, and every third-party action pinned to a full commit SHA with a `# vX.Y.Z` comment (extras/fd/.github/workflows/CICD.yml).
3. Add `cooldown: default-days: 7` to dependabot for both the `cargo` and `github-actions` ecosystems so freshly published releases age before being proposed (extras/fd/.github/dependabot.yml).
4. Build a `TestEnv`-style black-box harness for the CLI surface: locate the binary with `env!("CARGO_BIN_EXE_quinjet")`, construct a real temporary Git repository fixture with `tempfile::Builder::new().prefix(...)`, isolate environment variables per invocation, normalize output before comparing, and render failures as unified diffs with the `diff` crate (extras/fd/tests/testenv/mod.rs). Since every quinjet operation is a subcommand, every operation can be asserted end to end on stdout, stderr, and exit status exactly as extras/fd/tests/tests.rs does.
5. Adopt the `test-case` crate for flag and alias matrices, especially an equivalent of fd's `test_opposing` proving that each overriding option exactly cancels its counterpart (extras/fd/tests/tests.rs lines 2674 onward).
6. Model process exit as an enum with `impl From<ExitCode> for i32`, a `merge_exitcodes` fold, and 130 for SIGINT death, instead of scattering integer literals (extras/fd/src/exit_codes.rs). For a Git tool, distinct documented codes for "conflict", "nothing to do", and "user abort" pay off in scripts.
7. Sanitize terminal-bound output. Git data (branch names, commit subjects, remote URLs) is attacker-influenced text; port the `needs_escape`/`maybe_sanitize` approach that escapes controls, bidi overrides, and zero-width characters only when the stream is a TTY, with attack-named unit tests (extras/fd/src/sanitize.rs). This matters for quinjet's plain CLI output path even more than for the ratatui path.
8. Tune profiles for iteration speed: `[profile.dev] debug = "line-tables-only"`, `[profile.dev.package."*"] debug = false`, plus a `[profile.debugging]` that inherits dev with full debug info, and a release profile with `lto = true`, `strip = true`, `codegen-units = 1` (extras/fd/Cargo.toml).
9. Ship completions and a man page from the binary itself: put `clap_complete` behind a default `completions` feature with a hidden `--gen-completions` flag (extras/fd/src/cli.rs, extras/fd/src/main.rs), and add Makefile targets that generate and install them (extras/fd/Makefile). Consider `clap_mangen` for the man page since quinjet has no hand-written roff to preserve.
10. Add `[package.metadata.binstall]` with the pkg-url template matching the release artifact naming so `cargo binstall quinjet` works from day one (extras/fd/Cargo.toml).
11. Automate releases off tags: a matrix build job that packages binary plus completions plus docs per target, attests artifacts with `actions/attest` under `id-token: write` and `attestations: write`, and uploads with a SHA-pinned `softprops/action-gh-release`, all gated on `refs/tags/v[0-9]` (extras/fd/.github/workflows/CICD.yml). Keep a `doc/release-checklist.md` and a `scripts/version-bump.sh` for the human steps.
12. Keep the changelog contributor-facing: a permanent Unreleased section with fixed subsections, entries of the form `- description, see #123 (@user)`, and a CONTRIBUTING.md that makes the entry part of the definition of done (extras/fd/CHANGELOG.md, extras/fd/CONTRIBUTING.md).
13. Convert bug reports into structured YAML issue forms with a required version field and a required "I read the troubleshooting docs" checkbox (extras/fd/.github/ISSUE_TEMPLATE/bug_report.yaml).
14. Mechanically sync `--help` output into the README with a small script run at release time, as extras/fd/scripts/update-help.awk does, so documentation of the command surface can never drift from clap.
15. When an override of a strict lint is unavoidable, follow fd's one-allow discipline: smallest possible scope, always paired with a comment explaining the measured or reasoned tradeoff (extras/fd/src/walk.rs `WorkerResult`).

---

## nushell/nushell (40272 stars)

### 1. What the project is and how big it is

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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
`extras/nushell/ast-grep/tests/__snapshots__`. This is lint infrastructure as reviewable,
tested code.

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage

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

### 10. Documentation practices

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

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## tokio-rs/tokio (32930 stars)

### 1. What the project is and how big it is

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

### 2. Repository layout

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

### 3. Cargo manifest practices

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

### 4. Formatting

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

### 5. Linting

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

### 6. CI/CD

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

### 7. Testing

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

### 8. Error handling and API design

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

### 9. Deep Rust usage

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

### 10. Documentation practices

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

### 11. Release and distribution

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

### 12. Lessons for quinjet

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

---

## gitui-org/gitui (22396 stars)

### 1. What the project is and how big it is

gitui is a keyboard-driven terminal user interface for Git, self-described in extras/gitui/Cargo.toml as a "blazing fast terminal-ui for git". It is one of the most widely installed Rust TUIs in industry: the README installation section at extras/gitui/README.md lists packaged builds for Fedora (`dnf`), openSUSE (`zypper`), Homebrew, MacPorts, winget, scoop, chocolatey, FreeBSD `pkg`, and conda-forge, which is a good proxy for real-world adoption. Engineers reach for it because it wraps libgit2 and gitoxide behind an async job system so that even huge repositories (the Makefile keeps commented run targets against the Linux and Kubernetes trees in extras/gitui/Makefile) stay responsive.

Measured from the clone at commit `2fa693c`:

- 162 Rust source files, 50,372 lines of Rust across the repository.
- 7 crates: the root binary plus 6 library crates. extras/gitui/Cargo.toml declares five workspace members explicitly, and `invalidstring` joins the workspace as a path dependency of asyncgit:

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

- Line counts per crate: `src` (the binary) 29,418; `asyncgit` 17,145; `filetreelist` 1,953; `git2-hooks` 1,631; `git2-testing` 97; `scopetime` 68; `invalidstring` 11.
- 318 `#[test]` functions across the workspace (177 in asyncgit, 79 in the binary crate, 62 in filetreelist and git2-hooks combined).

### 2. Repository layout

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

### 3. Cargo manifest practices

The root manifest at extras/gitui/Cargo.toml is both the binary package and the workspace root. Notable practices:

- MSRV and edition are pinned in the package table: `edition = "2021"` and `rust-version = "1.88"`. The same MSRV appears in `.clippy.toml` and as an explicit row in the CI matrix, so the claim is enforced three ways.
- There is no `[workspace.package]` inheritance and no `[workspace.dependencies]`: every member repeats `authors`, `edition`, `license`, `homepage`, `repository` (see extras/gitui/asyncgit/Cargo.toml and extras/gitui/filetreelist/Cargo.toml). Each member is independently published to crates.io, which is why each carries full metadata, `categories`, and `keywords`. Path dependencies always pair a `path` with a `version` so publishing works: `asyncgit = { path = "./asyncgit", version = "0.28.1", default-features = false }`. One unusual bound: `filetreelist = { path = "./filetreelist", version = ">=0.6" }`.
- Crates-io hygiene: `exclude = [".github/*", ".vscode/*", "assets/*"]` keeps the published tarball small, and filetreelist excludes its demo gif (`exclude = ["/demo.gif"]`).
- Feature flags document their constraints inline:

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

- Profiles are tuned for the product. Debug builds keep the UI fast by optimizing only the hot dependency, and release optimizes for binary size:

```toml
[profile.dev.package."ratatui"]
opt-level = 3

[profile.release]
opt-level = "z"  # Optimize for size.
strip = "debuginfo"
lto = true
codegen-units = 1
```

- Dependencies are alphabetized, and `default-features = false` is applied aggressively (`chrono`, `ratatui`, `syntect`, `simplelog`, `bytesize`, `two-face`) to keep the dependency tree and binary small.
- There is no `[lints]` table anywhere; lint policy lives in crate-level attributes (section 5).
- extras/gitui/rust-toolchain.toml pins only `channel = "stable"` with `profile = "default"`, so contributors build on current stable while CI separately guards the MSRV.
- extras/gitui/.cargo/config.toml maps cross linkers per target, e.g. `[target.aarch64-unknown-linux-gnu] linker = "aarch64-linux-gnu-gcc"`.

### 4. Formatting

extras/gitui/rustfmt.toml is three lines, all stable options:

```toml
max_width = 70
hard_tabs = true
newline_style = "Unix"
```

- `max_width = 70`: far below the default 100. The codebase is designed to be read in narrow terminal splits next to the running TUI; it also forces short expressions and early extraction of locals.
- `hard_tabs = true`: indentation is tab characters, so each reader chooses their own indent width. This is paired with the editor layer: extras/gitui/.editorconfig declares `root = true` and, for `[*.rs]`, `indent_style = tab`, so non-rustfmt editors agree with rustfmt.
- `newline_style = "Unix"`: normalizes line endings across the Windows contributors the project demonstrably has (there is a full Windows CI leg and MSI packaging).

Non-Rust formatting is also enforced. TOML files are formatted with tombi, configured at extras/gitui/tombi.toml with an MSRV-driven constraint, explained in place:

```toml
# Keep dependency inline tables on a single line. Multi-line inline tables are
# TOML 1.1 syntax that Cargo on our MSRV (rust 1.88) rejects with
# "invalid inline table", so tombi must not expand them.
[format.rules]
line-width = 220
```

CI runs `tombi format --check` in the linting job (extras/gitui/.github/workflows/ci.yml) and the Makefile aliases it as `make sort`. Spelling is checked by typos with extras/gitui/typos.toml, which whitelists project words (`ratatui = "ratatui"`) and excludes the changelog via `extend-exclude = ["CHANGELOG.md"]`. Editor auto-format is switched on for contributors in extras/gitui/.vscode/settings.json (`"editor.formatOnSave": true`).

### 5. Linting

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

### 6. CI/CD

Four workflows live in extras/gitui/.github/workflows.

`ci.yml` triggers on a nightly cron (`"0 2 * * *"`), on push to every branch (`branches: ["*"]`), and on pull requests to master. Its jobs:

- `build`: a 3x3 matrix, `os: [ubuntu-latest, macos-latest, windows-latest]` by `rust: [nightly, stable, "1.88"]`, with `fail-fast: false` and `continue-on-error: ${{ matrix.rust == 'nightly' }}`. Pinning the literal MSRV as a matrix row means an accidental use of a newer API fails CI, while the nightly row is an early-warning canary that cannot block merges. Steps: `Swatinem/rust-cache@v2` with a `shared-key` composed of os, cache name, and toolchain; `dtolnay/rust-toolchain@master` with the matrix toolchain; nextest installed via `taiki-e/install-action@nextest`; debug build, `make test`, `make clippy`, `make build-release`; then `cargo install --path "." --force --locked` as a packaging smoke test; binary size listing per OS; `otool -L` on macOS to audit dynamic library linkage; and `cargo wix` on Windows to prove the MSI still builds. It even installs signing tools (`gpgsm`, gnupg) because the test suite includes real end-to-end commit-signing tests.
- `build-linux-musl`: same three toolchains against `x86_64-unknown-linux-musl`, running the full suite with `make test-linux-musl` and checking `--version` output of both debug and release binaries.
- `build-linux-arm`: cross-compiles aarch64, armv7, and arm targets with vendor GCC toolchains, then actually executes the aarch64 test binaries under emulation by exporting `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER: qemu-aarch64-static -L /usr/aarch64-linux-gnu`. Very few projects run their tests on a foreign architecture in CI.
- `build-apple-x86`: cross-builds the Intel macOS binary from Apple Silicon runners.
- `linting`: `cargo fmt -- --check`, `tombi format --check`, and `cargo deny check`.
- `udeps`: nightly toolchain plus `cargo +nightly udeps --all-targets`.
- `log-test` ("Changelog Test"): runs `ffurrer2/extract-release-notes@v2` on every PR and uploads the result, guaranteeing the changelog stays machine-extractable before release day.
- `test-homebrew`: `brew install --build-from-source gitui` on macOS, verifying the downstream formula still builds from source.

Actions are pinned by major tag (`actions/checkout@v4`, `Swatinem/rust-cache@v2`, `softprops/action-gh-release@v2`), not by SHA. `cd.yml` triggers on tag push and `workflow_dispatch`, and is the only workflow that requests write permission, minimally scoped:

```yaml
permissions:
  contents: write
```

The release job re-runs tests and clippy per OS, builds all release artifacts via Makefile targets (`release-mac`, `release-mac-x86`, `release-linux-musl`, `release-win`, `release-linux-arm`), computes a SHA256 for the mac tarball, extracts the release body from CHANGELOG.md with the same extract-release-notes action CI validated, publishes with `softprops/action-gh-release@v2` using `prerelease: ${{ contains(github.ref, '-') }}`, and finally bumps the homebrew-core formula through `mislav/bump-homebrew-formula-action@v3`, skipping prereleases. `nightly.yml` rebuilds all artifacts on a 3 a.m. cron and uploads them to an S3 bucket (`AWS_BUCKET_NAME: s3://gitui/nightly/`), documented for users in extras/gitui/NIGHTLIES.md. `brew.yml` is a manual re-run of the formula bump with a `tag-name` input for when the automatic bump fails.

Repo automation beyond workflows: extras/gitui/.github/dependabot.yml runs cargo updates daily and groups them (`cargo-minor` and `cargo-patch` groups with `patterns: ["*"]`), collapsing dependency noise into two rolling PRs; extras/gitui/.github/stale.yml marks issues `dormant` after 180 days with `pinned`, `security`, and `nostale` exemptions. No merge-queue configuration is present in the repository; branch protection is configured server-side and not visible from the clone.

### 7. Testing

There are no `tests/` directories anywhere in the workspace; all 318 tests are colocated `#[cfg(test)] mod tests` blocks inside the modules they cover (49 files contain a `mod tests`). The split by crate mirrors the architecture: the Git domain logic in asyncgit carries the bulk (177 tests, e.g. staging, rebase, hooks, signing under extras/gitui/asyncgit/src/sync/), pure data structures in filetreelist and hook logic in git2-hooks carry 62, and the binary crate has 79 including full-application tests.

The harness infrastructure is layered:

- `git2-testing` (extras/gitui/git2-testing/src/lib.rs) provides `repo_init_empty`, `repo_init`, `repo_init_bare`, and `repo_init_suffix`, each returning `(TempDir, Repository)` with committer identity preconfigured. Crucially it also sandboxes global Git state so developer machines cannot influence tests:

```rust
    // Adapted from https://github.com/rust-lang/cargo/pull/9035
    INIT.call_once(|| unsafe {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        set_search_path(ConfigLevel::System, path).unwrap();
        set_search_path(ConfigLevel::Global, path).unwrap();
```

- `invalidstring` (extras/gitui/invalidstring/src/lib.rs) manufactures invalid UTF-8 strings so path and message handling is tested against hostile data.
- Snapshot testing: the whole application is driven headlessly in extras/gitui/src/gitui.rs using ratatui's `TestBackend`, and terminal frames are asserted with insta. A local macro normalizes nondeterminism before comparison:

```rust
    macro_rules! apply_common_filters {
        {} => {
            let mut settings = insta::Settings::clone_current();
```

  with filters that rewrite temp directories to `[TEMP_FILE]` and 7-char commit ids after a box-drawing bar to `[AAAAA]`. The `gitui_starts` test boots the app on a fresh repo, snapshots the loading frame, injects an `AsyncGitNotification::Status`, snapshots again, then synthesizes key events to switch tabs and snapshots the log view. Snapshots live at extras/gitui/src/snapshots/*.snap. The dev-dependency enables filters explicitly: `insta = { version = "1.41.0", features = ["filters"] }`.

- End-to-end external-tool tests: extras/gitui/asyncgit/src/sync/sign.rs contains `test_x509_sign_and_verify_e2e`, which shells out to real `openssl` and `gpgsm` to build a throwaway X.509 identity, signs an actual commit, and verifies it. It is `#[cfg(unix)]` and `#[serial]` (via `serial_test`) because it mutates the process-wide `GNUPGHOME`. CI installs `gpgsm` on Linux and gnupg on macOS specifically for these tests.
- Runner: `cargo nextest run --workspace` is the canonical test command (extras/gitui/Makefile `test:` target), with per-target variants `test-linux-musl` and `test-linux-arm`; the ARM variant demonstrates nextest's filter expressions by excluding one kernel-behavior-dependent test: `-E 'not test(test_hook_with_missing_shebang)'`.
- `pretty_assertions` is a dev-dependency of every crate with tests, and `env_logger` is initialized per test via `git2-testing::init_log` with `is_test(true)`.

There is no fuzzing, property testing, or `benches/` directory in the repository; performance work is done ad hoc through `make profile` (`cargo flamegraph --features timing`) and the scopetime instrumentation instead. The public surface (a flag-only CLI) is covered indirectly: CI executes the built binary (`./target/.../gitui --version`) on musl and validates `cargo install` on every OS.

Two smaller habits round out the harness. First, the application object is factored so tests can drive it: `Gitui` in extras/gitui/src/gitui.rs exposes `input_event`, `update_async`, and `wait_for_async_git_notification` methods that the snapshot test calls directly, meaning the production event loop and the test share the same code path rather than a parallel test-only harness. Second, snapshot temp directories are made recognizable on purpose: `repo_init_suffix(Some("-insta"))` (extras/gitui/git2-testing/src/lib.rs) creates temp dirs ending in `-insta` precisely so the insta filter regexes can find and redact them on all three operating systems.

### 8. Error handling and API design

The repository uses the classic two-tier scheme: `thiserror` in libraries, `anyhow` in the binary.

- extras/gitui/asyncgit/src/error.rs defines a single crate-wide `Error` enum with `#[from]` conversions for io, git2, UTF-8, integer conversion, threadpool, hooks, and signing errors, plus domain variants with user-readable messages such as `#[error("git: no head found")] NoHead` and `#[error("git: uncommitted changes")] UncommittedChanges`. A nested `GixError` enum isolates the highly granular gitoxide error types, and large variants are boxed to keep the enum small: `Discover(#[from] Box<gix::discover::Error>)`.
- The signing module keeps two purpose-specific error enums (`SignBuilderError`, `SignError`) next to the `Sign` trait in extras/gitui/asyncgit/src/sync/sign.rs, so construction failures and runtime failures are distinct types.
- The binary crate uses `anyhow::Result` end to end; `main` itself returns `Result<()>` (extras/gitui/src/main.rs) and context is attached at the boundary, e.g. in extras/gitui/src/args.rs: `fs::create_dir_all(&confpath).with_context(|| format!("failed to create config directory: {}", confpath.display()))?`.
- Panic policy is enforced by lints (`clippy::unwrap_used` and `clippy::panic` denied in both main crates) and mitigated at runtime: a custom hook restores the terminal before printing, so a crash never leaves the user's shell in raw mode (extras/gitui/src/main.rs):

```rust
    panic::set_hook(Box::new(|e| {
        let backtrace = Backtrace::new();
        shutdown_terminal();
```

  Normal shutdown uses `scopeguard::defer! { shutdown_terminal(); }` immediately after terminal setup. The only explicit exit code is `std::process::exit(0)` after `--bugreport` output in extras/gitui/src/args.rs; all other paths exit through `Result`.

- Builder pattern: `SignBuilder::from_gitconfig` returns a `Box<dyn Sign>` selected from git config, with `impl Sign for GPGSign` and `impl Sign for SSHSign` as concrete strategies.
- Newtypes: `CommitId(Oid)` in extras/gitui/asyncgit/src/sync/commits_info.rs wraps the raw git2 object id and centralizes every conversion (`From<Oid>`, `From<gix::ObjectId>`, `From<gix::Commit<'_>>`, `Display`), letting the UI crate stay ignorant of both Git backends. `RepoPath` in extras/gitui/asyncgit/src/sync/repository.rs is an enum newtype distinguishing a plain path from a separated gitdir/workdir pair, with `From<PathBuf>` and `From<&str>` for ergonomic construction.
- Visibility discipline: asyncgit's `lib.rs` re-exports a curated `sync` API while keeping job modules (`fetch_job`, `filter_commits`) private; `#![forbid(missing_docs)]` forces every public item to be documented or hidden.

### 9. Deep Rust usage: ten-plus cited idioms

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

### 10. Documentation practices

- Crate docs double as architecture docs. extras/gitui/src/main.rs opens with a `//!` map of the module groups (tabs, components, popups, ui, asyncgit) and of the included crates with their dependency relationships. extras/gitui/src/components/mod.rs documents the composition philosophy explicitly, including its limits ("composition is driven by code", plus an honest note that the two traits should probably merge someday).
- asyncgit enforces docs with `#![forbid(missing_docs)]` (extras/gitui/asyncgit/src/lib.rs). The team consciously trades prose for coverage: many items carry an empty `///` doc, and `clippy::empty_docs` is allowed, meaning the forbid acts as a checklist that makes undocumented surface impossible while letting trivial items stay terse.
- User docs are versioned markdown at the repo root: extras/gitui/KEY_CONFIG.md (custom keybindings), extras/gitui/THEMES.md (theme RON patching), extras/gitui/FAQ.md, extras/gitui/NIGHTLIES.md (nightly artifact URLs), and a 301-line README with a linked table of contents and a per-package-manager install matrix.
- extras/gitui/CONTRIBUTING.md is short and welcoming: build instructions by reference, a Discord link for help, and a pointer to `good-first-issue` labels.
- extras/gitui/.github/PULL_REQUEST_TEMPLATE.md encodes the quality gate as a checklist:

```markdown
I followed the checklist:
- [ ] I added unittests
- [ ] I ran `make check` without errors
- [ ] I tested the overall application
- [ ] I added an appropriate item to the changelog
```

- Issue templates exist for bug reports and feature requests (extras/gitui/.github/ISSUE_TEMPLATE/bug_report.md, feature_request.md), and the in-app `--bugreport` flag (extras/gitui/src/bug_report.rs, built on the `bugreport` crate) prints version, OS, compile-time info, and relevant environment variables as Markdown ready to paste into an issue.

### 11. Release and distribution

- Versioning is SemVer with the binary and asyncgit released in lockstep at 0.28.1 (extras/gitui/Cargo.toml, extras/gitui/asyncgit/Cargo.toml); utility crates version independently (filetreelist 0.6.0, git2-hooks 0.7.0).
- Changelog discipline is strict Keep a Changelog: extras/gitui/CHANGELOG.md (1,025 lines) keeps an `## Unreleased` section that every PR must append to (enforced socially by the PR template and mechanically by the `log-test` CI job that extracts release notes from it on every run). Entries credit contributors by handle and link issues, and release sections embed screenshots of headline features.
- The release pipeline is tag-driven (extras/gitui/.github/workflows/cd.yml): artifacts are mac arm64 and x86 tarballs, a musl-static Linux x86_64 tarball, aarch64/armv7/arm tarballs, a Windows tarball, and a WiX MSI (sources in extras/gitui/wix/main.wxs). Makefile release targets strip binaries and print `otool -L` so accidental dynamic linkage is visible in logs. Release bodies come from the changelog via `ffurrer2/extract-release-notes@v2`; hyphenated tags publish as prereleases; a successful stable release auto-bumps homebrew-core.
- Reproducibility and provenance: extras/gitui/build.rs honors `SOURCE_DATE_EPOCH` for the build date, accepts `BUILD_GIT_COMMIT_ID` for `git archive` tarballs, and stamps `GITUI_BUILD_NAME` as either the bare version (when `GITUI_RELEASE=1`) or `<version>-nightly <date> (<hash>)`, which `clap` then surfaces via `.version(env!("GITUI_BUILD_NAME"))` in extras/gitui/src/args.rs.
- A parallel nightly channel (extras/gitui/.github/workflows/nightly.yml) rebuilds all platforms daily and pushes to S3, giving users a low-friction way to verify fixes before a release.
- gitui is a flag-only CLI (no subcommands), and the repository ships no shell completions or man pages; discoverability is delegated to the in-app help and `--help` template defined in extras/gitui/src/args.rs.
- License compliance for distributors is one command away: the Makefile's `licenses` target runs `cargo bundle-licenses --format toml --output THIRDPARTY.toml` (extras/gitui/Makefile), producing a machine-readable third-party license inventory that packagers can regenerate at any tag.
- Local packaging parity: the same Makefile targets CI uses (`release-mac`, `release-win`, `release-linux-musl`) are runnable on a developer machine, so a maintainer can reproduce any release artifact without GitHub Actions, and `install` / `install-timing` targets exercise the exact `cargo install --path "." --offline --locked` path users hit.

### 12. Lessons for quinjet

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

---

## clap-rs/clap (16634 stars)

### 1. What the project is and why industry uses it

clap is the de facto standard command-line argument parser for Rust. The root package describes itself in `extras/clap/Cargo.toml` as "A simple to use, efficient, and full-featured Command Line Argument Parser". Its crate-level documentation in `extras/clap/src/lib.rs` states the project's engineering values directly, and they read like a maintenance policy rather than marketing:

```text
//! - Resilient maintainership, including
//!   - Willing to break compatibility rather than batching up breaking changes in large releases
//!   - Leverage feature flags to keep to one active branch
//!   - Being under [WG-CLI](https://github.com/rust-cli/team/) to increase the bus factor
//! - We follow semver and will wait about 6-9 months between major breaking changes
//! - We will support the last two minor Rust releases (MSRV, currently 1.74)
```

Industry adopts it because it delivers a polished end-user CLI experience (help text, suggestions, colored output, completions) out of the box, and because it exposes both a runtime builder API and a `#[derive]` API over the same core.

Measured scale from the clone:

- 8 packages: the root `clap` facade plus 7 workspace members listed in `extras/clap/Cargo.toml` (`clap_bench`, `clap_builder`, `clap_derive`, `clap_lex`, `clap_complete`, `clap_complete_nushell`, `clap_mangen`).
- 330 `.rs` files totaling 84,223 lines of Rust.
- Per-area line counts: `clap_builder` 28,998; `tests` 30,959; `clap_complete` 9,470; `clap_derive` 4,517; `examples` 2,648; `clap_mangen` 1,842; `clap_bench` 1,601; `src` 1,676; `clap_lex` 1,269; `clap_complete_nushell` 1,243. The integration test tree is larger than the core implementation crate, which says a lot about the testing culture.
- Version at the clone: `clap 4.6.6`, MSRV `1.85`, edition `2024`.

### 2. Repository layout

```text
extras/clap/
|-- Cargo.toml              root "clap" facade crate + [workspace] tables
|-- Cargo.lock              committed, freshness-checked in CI
|-- CHANGELOG.md            Keep a Changelog format, machine-updated
|-- CONTRIBUTING.md         goals, compat policy, commit hygiene
|-- CITATION.cff            citation metadata, validated in CI
|-- Makefile                feature-matrix commands shared by devs and CI
|-- deny.toml               cargo-deny bans/licenses/sources config
|-- .clippy.toml            clippy knobs: test allowances, disallowed-methods
|-- committed.toml          conventional-commit lint config
|-- typos.toml              spell-check exceptions
|-- release.toml            cargo-release workspace config
|-- .pre-commit-config.yaml pre-commit hooks (yaml/json/toml checks, typos, committed)
|-- .cargo/config.toml      resolver behavior for incompatible rust versions
|-- .github/
|   |-- workflows/          ci, audit, bench-baseline, committed, post-release,
|   |                       pre-commit, rust-next, spelling, template + release-notes.py
|   |-- ISSUE_TEMPLATE/     bug_report.yml, feature_request.yml, config.yml
|   |-- PULL_REQUEST_TEMPLATE.md
|   |-- renovate.json5      dependency-update policy incl. custom regex managers
|   `-- settings.yml        repo settings as code (probot settings app)
|-- src/                    thin facade: lib.rs re-exports + doc-only modules
|   |-- bin/stdio-fixture.rs   fixture binary for output snapshot tests
|   |-- _tutorial.rs, _faq.rs, _features.rs, _concepts.rs
|   |-- _cookbook/ and _derive/   rustdoc-only documentation modules
|-- clap_builder/           the actual implementation (builder API, parser, output)
|-- clap_derive/            proc-macro crate (Parser/Args/Subcommand/ValueEnum)
|-- clap_lex/               minimal OsStr-level lexer, reusable standalone
|-- clap_complete/          shell completion generation (static + dynamic engine)
|-- clap_complete_nushell/  nushell completion backend
|-- clap_mangen/            man page (roff) generation
|-- clap_bench/             divan benchmarks, publish = false
|-- examples/               paired .rs + .md trycmd transcripts, tutorials
`-- tests/                  integration tests for the public surface
```

Why this split works: the root `clap` crate is a facade over `clap_builder` and `clap_derive` (see `[dependencies]` in `extras/clap/Cargo.toml`), so the proc-macro crate can be compiled in parallel with the builder and users who skip `derive` never pay for `syn`. `clap_lex` isolates the genuinely tricky, `unsafe`-bearing OsStr handling into a tiny auditable crate. Completion and man-page generation live in separate crates with their own versions so they can release independently.

### 3. Cargo manifest practices

`extras/clap/Cargo.toml` uses `[workspace.package]` inheritance for everything that must stay uniform:

```toml
[workspace.package]
repository = "https://github.com/clap-rs/clap"
license = "MIT OR Apache-2.0"
edition = "2024"
rust-version = "1.85"  # MSRV
include = [
  "build.rs",
  "src/**/*",
  "Cargo.toml",
  "LICENSE*",
  "README.md",
  "examples/**/*"
]
```

Every member manifest then carries `repository.workspace = true`, `license.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`, `include.workspace = true` (for example `extras/clap/clap_builder/Cargo.toml`). The `include` list keeps published tarballs lean. The `# MSRV` comment is not decoration: it is a grep anchor. `extras/clap/CONTRIBUTING.md` documents "Updating MSRV: Search for `MSRV`" and Renovate keys off similar comment tags.

Other notable manifest practices:

- Lockstep internal versions are pinned exactly: `clap_builder = { path = "./clap_builder", version = "=4.6.6", default-features = false }` in `extras/clap/Cargo.toml`, so facade and implementation can never drift.
- Feature flags are organized into labeled tiers in `extras/clap/Cargo.toml`: `default` (std, color, help, usage, error-context, suggestions), "Optional" (deprecated, derive, cargo, wrap_help, env, unicode, string), and "In-work features" all prefixed `unstable-` (`unstable-v5`, `unstable-ext`, `unstable-markdown`). Facade features forward with the `?` syntax: `deprecated = ["clap_builder/deprecated", "clap_derive?/deprecated"]`.
- Optional dependencies are namespaced via `dep:`: `color = ["dep:anstream"]` in `extras/clap/clap_builder/Cargo.toml`, so features never leak implicit dependency features.
- `[lints] workspace = true` appears in every member (for example `extras/clap/clap_lex/Cargo.toml`), pulling from the shared `[workspace.lints.*]` tables.
- Profiles in `extras/clap/Cargo.toml`: `panic = "abort"` in both dev and release (a parser library needs no unwinding), `codegen-units = 1` plus `lto = true` in release and bench, and `[profile.test] opt-level = 1` to keep the huge test suite fast.
- `[lib] bench = false` in every crate so `cargo bench` only runs the real `[[bench]]` targets in `extras/clap/clap_bench/Cargo.toml`, each with `harness = false` for divan.
- Dozens of `[[example]]` blocks in `extras/clap/Cargo.toml` each declare `required-features` and `doc-scrape-examples = true`, so examples build only with the features they need and get scraped into docs.rs.
- docs.rs config: `[package.metadata.docs.rs] features = ["unstable-doc"]` and `rustdoc-args = ["--generate-link-to-definition"]`.
- `extras/clap/clap_bench/Cargo.toml` sets `publish = false`, `version = "0.0.0"` and `[package.metadata.release] release = false`: the benchmark crate can never leak to crates.io.
- `extras/clap/.cargo/config.toml` sets `[resolver] incompatible-rust-versions = "fallback"` so dependency resolution respects the MSRV.

### 4. Formatting

There is no `rustfmt.toml` or `.rustfmt.toml` anywhere in the repository, and no `.editorconfig`. That absence is itself the policy: default rustfmt, zero configuration to argue about. Enforcement happens in CI, in the `rustfmt` job of `extras/clap/.github/workflows/ci.yml`:

```yaml
  rustfmt:
    name: rustfmt
    ...
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        toolchain: "1.97"  # STABLE
        components: rustfmt
    ...
    - name: Check formatting
      run: cargo fmt --check
```

The toolchain is pinned to a specific stable (`1.97`) rather than floating `stable`, so a rustfmt release cannot break every open PR overnight; the `# STABLE` comment is a Renovate anchor that gets bumped automatically (see section 6).

Non-Rust files are handled by pre-commit hooks in `extras/clap/.pre-commit-config.yaml`: `check-yaml`, `check-json`, `check-toml`, `check-merge-conflict`, `check-case-conflict`, `detect-private-key`, plus `typos` and `committed`. The config excludes generated content:

```yaml
exclude: |
  (?x)^(
    tests/.*|
    CHANGELOG.md
  )$
```

These same hooks run in CI via `extras/clap/.github/workflows/pre-commit.yml` using the `j178/prek-action` runner with a pinned `prek-version: '0.2.27'`.

### 5. Linting

Clippy policy lives in three places, each doing a distinct job.

First, `[workspace.lints.rust]` and `[workspace.lints.clippy]` in `extras/clap/Cargo.toml` define the lint wall once for all 8 crates. The philosophy is a curated warn-list, not a blanket `pedantic`: about 50 individually chosen clippy lints set to `warn` (`str_to_string`, `dbg_macro`, `todo`, `mem_forget`, `uninlined_format_args`, `verbose_file_reads`, ...), with explicit allows that carry their reasoning inline:

```toml
let_and_return = "allow"  # sometimes good to name what you are returning
...
# Fix later:
multiple_bound_locations = "allow"
assigning_clones = "allow"
blocks_in_conditions = "allow"
```

The `# Fix later:` block is a visible debt ledger inside the lint table itself. On the rustc side, `rust_2018_idioms` is enabled as a group at `priority = -1` with targeted members (`unreachable_pub = "warn"`, `unused_qualifications = "warn"`) layered on top.

Second, `extras/clap/.clippy.toml` configures lint behavior, including a project-specific style ruleset via `disallowed-methods`, each with a human reason:

```toml
allow-print-in-tests = true
allow-expect-in-tests = true
allow-unwrap-in-tests = true
allow-dbg-in-tests = true
disallowed-methods = [
    { path = "std::option::Option::map_or", reason = "prefer `map(..).unwrap_or(..)` for legibility" },
    ...
    { path = "std::iter::Iterator::for_each", reason = "prefer `for` for side-effects" },
]
```

This is how you get custom "house style" lints without writing a compiler plugin: `disallowed-methods` turns taste into machine-checked policy, and the `allow-*-in-tests` keys stop test code from fighting production-strictness.

Third, crate-level attributes set non-negotiables per crate. Every crate opens with the same wall, for example `extras/clap/clap_builder/src/lib.rs`:

```rust
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]
```

`print_stdout`/`print_stderr` matter for a library that must never write to the user's terminal except through its own `Colorizer`.

Enforcement escalates warnings to errors only in CI: the `clippy-%` target in `extras/clap/Makefile` runs `cargo clippy ... --all-targets -- -D warnings -A deprecated`, and the `clippy` CI job runs it four times across feature configurations (ultra-minimal, minimal, full, release). Spelling is a separate lint layer (`extras/clap/typos.toml` plus `extras/clap/.github/workflows/spelling.yml`), and commit messages are linted with `committed` against `style="conventional"` in `extras/clap/committed.toml`.

### 6. CI/CD

All CI logic is intentionally split between thin YAML and a `Makefile` that both humans and CI call. `extras/clap/Makefile` opens with the rationale:

```make
# CI Steps
#
# Considerations
# - Easy to debug: show the command being run
# - Leverage CI features: Only run individual steps so we can use features like reporting elapsed time per step
```

It defines named feature bundles (`minimal`, `default`, `wasm`, `full`, `next`, `debug`, `release`) and pattern rules (`check-%`, `build-%`, `test-%`, `clippy-%`) that expand them, so `make test-full` means the same thing on a laptop and in a runner.

`extras/clap/.github/workflows/ci.yml` is the main pipeline:

- Top-level `permissions: contents: read`, per-job overrides (`permissions: contents: none` on the gate job), `concurrency` with `cancel-in-progress: true`, and env `RUST_BACKTRACE: 1`, `CARGO_TERM_COLOR: always`.
- A single aggregation gate job named `ci` that `needs:` every other job and fails if any dependency failed, was cancelled, or skipped:

```yaml
  ci:
    permissions:
      contents: none
    name: CI
    needs: [test, shell-integration, shell-integration-nu, check, ui, minimal-versions, lockfile, docs, rustfmt, clippy, cffconvert]
    runs-on: ubuntu-latest
    if: "always()"
    steps:
    - name: Failed
      run: exit 1
      if: "contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') || contains(needs.*.result, 'skipped')"
```

  Branch protection then only needs to require one check ("CI"), as documented in the commented ruleset at the bottom of `extras/clap/.github/settings.yml`.

- `test` matrix: 6 builds covering `linux`/`windows`/`mac` at `full` features plus `minimal`, `default` and `next` feature sets on Linux; every leg also runs benches in test mode and dynamic-completion tests.
- `check` matrix covers the MSRV toolchain (`1.85`), `wasm32-unknown-unknown`, `wasm32-wasip2`, a `debug`-feature build, and a `release` build.
- `minimal-versions` job downgrades the lockfile with `cargo +nightly generate-lockfile -Z minimal-versions` and then compiles on stable `--locked`, proving the declared version floors are honest.
- `lockfile` job runs `cargo update --workspace --locked` to fail if `Cargo.lock` is stale.
- `docs` builds with `RUSTDOCFLAGS: -D warnings` through `make doc`, which passes `--all-features --no-deps --document-private-items`.
- Caching is `Swatinem/rust-cache@v2` everywhere, with cache size deliberately reduced via `env: CARGO_PROFILE_DEV_DEBUG: line-tables-only` and a comment saying so.
- Actions are pinned to major tags (`actions/checkout@v7`, `dtolnay/rust-toolchain@stable`), not SHAs; toolchains are pinned to explicit versions with `# MSRV` and `# STABLE` comments.

Supporting workflows:

- `extras/clap/.github/workflows/audit.yml`: `actions-rs/audit-check` plus `EmbarkStudios/cargo-deny-action@v2` (checking `bans licenses sources`), triggered only on `Cargo.toml`/`Cargo.lock` paths, with `continue-on-error: true` on the audit job and the comment "Prevent sudden announcement of a new advisory from failing ci".
- `extras/clap/.github/workflows/rust-next.yml`: a monthly cron (`'3 3 3 * *'`) that runs the whole matrix on beta and nightly, plus a "Check latest dependencies" job that runs `cargo update` first. Toolchain and dependency breakage is detected on a schedule instead of blocking PRs.
- `extras/clap/.github/workflows/bench-baseline.yml`: on every push to master, builds the `git-derive` example with `CARGO_PROFILE_RELEASE_STRIP: true` and reports its file size to Bencher (`bencherdev/bencher@main`, `--file-size target/release/examples/git-derive`). Binary size is a tracked benchmark, not folklore.
- `extras/clap/.github/workflows/committed.yml` and `spelling.yml`: conventional-commit and typos gates on every PR.
- `extras/clap/.github/workflows/template.yml`: a monthly cron that merges from a shared template repository (`TEMPLATE_URL: "https://github.com/epage/_rust.git"`), pushes a branch, opens a PR with `gh pr create`, and enables automerge. Repository boilerplate stays converged across the maintainer's projects automatically.
- `extras/clap/.github/settings.yml` keeps repository settings in code (probot settings app): `allow_rebase_merge: false`, `allow_auto_merge: true`, `delete_branch_on_merge: true`, `squash_merge_commit_title: "PR_TITLE"`.
- `extras/clap/.github/renovate.json5` encodes a deliberate dependency policy: compatible updates for normal dependencies are disabled ("Keep version reqs low"), dev-dependency patches automerge, and custom regex managers keep the `STABLE` Rust pin synchronized across `Makefile`, `ci.yml`, `rust-next.yml`, `.clippy.toml`, and `tests/derive_ui.rs` with automerge enabled.

### 7. Testing

Tests live almost entirely as integration tests against the public API. `extras/clap/tests/builder/` has 46 files exercising the builder API and `extras/clap/tests/derive/` covers the derive API; both are wired up with a single line thanks to automod, as in `extras/clap/tests/builder/main.rs`:

```rust
#![allow(clippy::self_named_module_files)] // false positive
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

automod::dir!("tests/builder");
```

Adding a test file requires no registration, and the whole directory compiles as one test binary (fast linking) while staying organized by topic (`env.rs`, `groups.rs`, `subcommands.rs`, ...).

Distinct testing layers:

- End-to-end CLI transcripts with trycmd: `extras/clap/tests/ui.rs` compiles the examples and replays TOML cases from `extras/clap/tests/ui/*.toml`. A case like `extras/clap/tests/ui/help_flag_stdout.toml` pins `bin.name`, `args`, `status.code`, full `stdout` and `stderr` against the fixture binary `extras/clap/src/bin/stdio-fixture.rs`. Additionally, every example has a paired markdown transcript (`extras/clap/examples/git.md` shows `$ git ...` sessions with expected output) that trycmd verifies, so the documentation is executable.
- Compile-fail UI tests with trybuild: `extras/clap/tests/derive_ui.rs` runs `t.compile_fail("tests/derive_ui/*.rs")` against checked-in `.stderr` files, pinned to one toolchain with `#[rustversion::attr(not(stable(1.97)), ignore)] // STABLE` so rustc diagnostic changes cannot break unrelated PRs, and gated behind the `unstable-derive-ui-tests` feature.
- Snapshot testing with snapbox, including rendered-terminal SVG snapshots: the root dev-dependencies in `extras/clap/Cargo.toml` include `snapbox = { version = "1.2.0", features = ["term-svg"] }`, and `extras/clap/tests/derive/snapshots/` holds files like `headers.term.svg`, capturing styled help output including ANSI colors as reviewable SVG images.
- Real-shell completion tests in PTYs: `extras/clap/clap_complete/tests/testsuite/bash.rs` uses `completest_pty::BashRuntimeBuilder` to type into an actual bash and assert the completions; the `shell-integration` CI job installs `elvish fish zsh` via apt before running them, and `extras/clap/clap_complete/tests/snapshots/` pins generated scripts per shell (`basic.bash`, `basic.zsh`, `basic.fish`, `basic.ps1`, `basic.elvish`).
- Benchmarks: `extras/clap/clap_bench/benches/` contains divan benchmarks modeled on real CLIs (`ripgrep.rs`, `rustup.rs`), all `harness = false`, and CI compiles them on every PR via `make test-... ARGS='--workspace --benches'`.
- Feature-matrix testing: the Makefile bundles ensure the crate is tested with no default features, default, full, and next (v5-preview) feature sets, catching feature-gate compile breakage that single-configuration CI misses.

### 8. Error handling and API design

There is no `thiserror` or `anyhow` anywhere in the dependency tree; error handling is fully hand-rolled and user-facing. The central type in `extras/clap/clap_builder/src/error/mod.rs` is generic over a formatting strategy and keeps its payload boxed so `Result<T, Error>` stays one pointer wide:

```rust
pub struct Error<F: ErrorFormatter = DefaultFormatter> {
    inner: Box<ErrorInner>,
    phantom: std::marker::PhantomData<F>,
}
```

`DefaultFormatter` is itself a conditional alias: `RichFormatter` when the `error-context` feature is on, `KindFormatter` otherwise (same file). `ErrorKind` in `extras/clap/clap_builder/src/error/kind.rs` is a `#[non_exhaustive]` enum where every variant carries a runnable doctest demonstrating how to trigger it. Exit codes are explicit constants in `extras/clap/clap_builder/src/util/mod.rs` (`SUCCESS_CODE: i32 = 0`, `USAGE_CODE: i32 = 2`, matching Unix convention for usage errors), and `Error::exit()` returns `!`.

The panic policy is written down in `extras/clap/CONTRIBUTING.md`: "`panic!` on *developer* error, exit gracefully on *end-user* error". It is implemented, not just stated: `extras/clap/clap_builder/src/builder/debug_asserts.rs` is a 63-assertion validation pass over the built `Command` (duplicate flags, version settings, index collisions) that runs only in debug builds, and panicking accessors in `extras/clap/clap_builder/src/parser/matches/arg_matches.rs` are annotated `#[cfg_attr(debug_assertions, track_caller)]` so the panic points at the caller's line, with panic messages that teach ("arg `{id}`'s `ArgAction` should be `Count` which should provide a default"). Internal invariant failures route through a single `INTERNAL_ERROR_MSG` in `extras/clap/clap_builder/src/lib.rs` that asks the user to file a bug.

API design discipline visible in the code:

- Builder setters take `impl IntoResettable<T>` (for example `pub fn long(mut self, l: impl IntoResettable<Str>) -> Self` in `extras/clap/clap_builder/src/builder/arg.rs`), giving ergonomic conversions plus the ability to pass `None` to reset.
- 141 `#[must_use]` annotations in `clap_builder/src` alone, so dropping a builder result warns.
- Newtypes carry semantics: `Str`, `OsStr`, `StyledStr`, `Id`, `ValueRange` in `extras/clap/clap_builder/src/builder/`.
- Visibility is tight: internals live in `pub(crate)` modules (`mkeymap`, `output`, `util` in `extras/clap/clap_builder/src/lib.rs`), `unreachable_pub = "warn"` is on workspace-wide, and macro plumbing that must be `pub` is `#[doc(hidden)]` (32 occurrences in `clap_builder/src`).

### 9. Deep Rust usage

1. Autoref specialization on stable. The `value_parser!` macro in `extras/clap/clap_builder/src/builder/value_parser.rs` picks the best available parser for a type at compile time without specialization, by exploiting method resolution over reference depth:

   ```rust
   macro_rules! value_parser {
    ($name:ty) => {{
        use $crate::builder::impl_prelude::*;
        let auto = $crate::builder::_infer_ValueParser_for::<$name>::new();
        (&&&&&&auto).value_parser()
    }};
   }
   ```

   Six traits are implemented for `&&&&&&_infer_ValueParser_for<P>` down to `_infer_ValueParser_for<P>` (same file, `impl_prelude`), ranking `ValueParserFactory` above `ValueEnum` above `From<OsString>` and so on. Deref coercion selects the highest-priority impl that applies.

2. Sealed traits. Both the specialization traits (`_impls_ValueParserFactorySealed` and friends in `extras/clap/clap_builder/src/builder/value_parser.rs`) and the `OsStr` extension trait in `extras/clap/clap_lex/src/ext.rs` are sealed:

   ```rust
   mod private {
    pub trait Sealed {}

    impl Sealed for std::ffi::OsStr {}
   }
   ```

   Public traits stay extensible only where extension is intended (`TypedValueParser` is open; `OsStrExt` is closed).

3. Quarantined unsafe with audited boundaries. Five of six crates declare `#![forbid(unsafe_code)]` (`extras/clap/src/lib.rs`, `clap_builder`, `clap_derive`, `clap_mangen`, `clap_complete_nushell`, `clap_bench`). The only `unsafe` lives in `extras/clap/clap_lex/src/ext.rs` for zero-copy `OsStr` splitting, each site carrying a SAFETY argument:

   ```rust
        bytes.strip_prefix(prefix.as_bytes()).map(|s| {
            // SAFETY:
            // - This came from `as_encoded_bytes`
            // - Since `prefix` is `&str`, any split will be along UTF-8 boundary
            unsafe { OsStr::from_encoded_bytes_unchecked(s) }
        })
   ```

   The workspace also sets `unsafe_op_in_unsafe_fn = "warn"` in `extras/clap/Cargo.toml`.

4. Binary-size-aware string newtype. `Str` in `extras/clap/clap_builder/src/builder/str.rs` stores `&'static str` by default and only gains a `String`-backed variant when the `string` feature is on (`#[cfg(feature = "string")] impl From<String> for Str`). Users who define their CLI with literals never link allocation paths for names.

5. Type erasure with debug-only diagnostics. `AnyValue` in `extras/clap/clap_builder/src/util/any_value.rs` wraps `Arc<dyn Any + Send + Sync>` for parsed values of arbitrary type, and its `AnyValueId` keeps `type_name: &'static str` only under `#[cfg(debug_assertions)]` so release builds pay nothing for readable type mismatch errors. `downcast_into` uses `Arc::try_unwrap(...).unwrap_or_else(|arc| (*arc).clone())` to avoid cloning when it holds the last reference.

6. Data-structure choice by workload. `extras/clap/clap_builder/src/util/flat_map.rs` implements a `Vec`-backed map ("This preserves insertion order") with `Borrow`-based lookup generics mirroring the std API. For the small maps a CLI definition produces, linear scan beats hashing, and insertion order is exactly help-display order.

7. `Resettable<T>` to fix a real type-inference gap. `extras/clap/clap_builder/src/builder/resettable.rs` documents precisely why it exists: "you can't have a function argument that is `impl Into<Option<T>>` where `T` is `impl Into<S>` accept `None` as its type is ambiguous". The workaround is a two-variant enum plus `From<T>` and `From<Option<T>>` impls, keeping `arg.short(None)` compiling.

8. Zero-cost debug tracing. `extras/clap/clap_builder/src/macros.rs` defines `debug!` twice: a styled-stderr writer when the `debug` feature is on, and `macro_rules! debug { ($($arg:tt)*) => {}; }` otherwise. Hundreds of trace points cost nothing in normal builds, and `extras/clap/CONTRIBUTING.md` documents `cargo test --features debug` as the debugging workflow.

9. Early-return macros instead of `?`. `extras/clap/clap_builder/src/macros.rs` defines `ok!` and `some!` that `match` and `return` directly. Unlike `?`, `ok!` performs no `From::from` conversion on the error, keeping error paths monomorphic and cheap inside the parser hot loop (used pervasively, for example in `extras/clap/clap_builder/src/util/flat_map.rs`).

10. Iterator-driven algorithms with measured exceptions. `did_you_mean` in `extras/clap/clap_builder/src/parser/features/suggestions.rs` maintains a sorted candidate list via `binary_search_by(...).unwrap_or_else(|e| e)` insertion and ends with an `into_iter().map(...).collect()` pipeline; it also documents an upstream bug decision inline: "GH #4660: using `jaro` because `jaro_winkler` implementation in `strsim-rs` is wrong".

11. Feature-gated graceful degradation without cfg soup. `extras/clap/clap_builder/src/output/textwrap/mod.rs` is a deliberate micro-fork ("Pull in only what we need rather than relying on the compiler to remove what we don't need") that exposes one `wrap()` signature with two bodies: real wrapping under `wrap_help`, identity otherwise. Callers never see the feature flag. The same file-header pattern in `extras/clap/clap_builder/src/error/mod.rs` uses module-level `#![cfg_attr(not(feature = "error-context"), allow(dead_code))]` instead of sprinkling per-item cfgs.

12. Docs as compiled code. `extras/clap/src/lib.rs` embeds runnable material with `#![doc = include_str!("../examples/demo.rs")]` and even doctests the README:

```rust
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
```

A guard for unsupported configurations uses the language rather than build scripts: `#[cfg(not(feature = "std"))] compile_error!("`std` feature is currently required to build `clap`");` in the same file.

### 10. Documentation practices

The most distinctive practice: the entire book lives inside the crate as rustdoc modules. `extras/clap/src/lib.rs` declares `pub mod _tutorial;`, `_cookbook`, `_derive`, `_faq`, `_features`, `_concepts`, all behind `#[cfg(feature = "unstable-doc")]`, and docs.rs builds with that feature via `[package.metadata.docs.rs]` in `extras/clap/Cargo.toml`. Tutorials interleave prose with `#![doc = include_str!("../examples/tutorial_builder/01_quick.rs")]` (see `extras/clap/src/_tutorial.rs`), so every tutorial snippet is a compiled example, and every example's console output is a trycmd-verified `.md` transcript. `extras/clap/examples/README.md` states the framework explicitly: the docs are organized by the four documentation types (tutorials, how-to guides, reference, explanation).

`extras/clap/CONTRIBUTING.md` is unusually operational: it defines compatibility expectations per release type (major "6-9 months", minor "2 months", patch "one for every user-facing, user-contributed PR (i.e. release early, release often)"), a version support table (v4 active, v3 maintenance, v2 deprecated), deprecation mechanics (`#[cfg_attr(feature = "deprecated", deprecated(...))]` behind an opt-in feature flag), and commit-history guidance including "Add tests in a commit before their feature or fix, showing the current behavior". It even documents code layout philosophy: "the `pub` items serve as a table-of-contents".

Issue intake is structured: `extras/clap/.github/ISSUE_TEMPLATE/bug_report.yml` is a form requiring exact versions ("PLEASE DO NOT PUT \"latest\" HERE"), a minimal reproduction, and pre-search checkboxes; `config.yml` routes questions to Discussions. `extras/clap/.github/PULL_REQUEST_TEMPLATE.md` asks only two things: what issue this closes ("a maintainer-approved Issue is required for non-trivial changes") and notes to reviewers. `missing_docs` is `warn` at crate level, and the docs CI job turns rustdoc warnings into failures including for private items.

### 11. Release and distribution

Releases are driven by cargo-release with the entire mechanical burden encoded in manifests:

- `extras/clap/release.toml` sets `dependent-version = "fix"`, `allow-branch = ["master", "v*-master"]` (so patch releases can happen from old major branches), and `owners` for crates.io team access.
- `[package.metadata.release]` in `extras/clap/Cargo.toml` sets `shared-version = true`, `tag-name = "v{{version}}"`, and `pre-release-replacements` that rewrite `CHANGELOG.md` (stamping `Unreleased` and `ReleaseDate`, regenerating the compare links from `<!-- next-header -->` and `<!-- next-url -->` markers), update `CITATION.cff`, and even fix the changelog link inside `src/lib.rs`. Each subcrate carries its own replacement set (see `extras/clap/clap_lex/Cargo.toml`), and `dependent-version = "upgrade"` in `extras/clap/clap_builder/Cargo.toml` keeps the facade's pinned dependency in lockstep.
- `extras/clap/CHANGELOG.md` follows Keep a Changelog with semver, contains a pre-written "5.0.0 - TBD" section flagged "*available through `unstable-v5` feature flag*", and shows patch cadence in action (4.6.4, 4.6.5, 4.6.6 within weeks).
- On tag push, `extras/clap/.github/workflows/post-release.yml` extracts the matching changelog section with `extras/clap/.github/workflows/release-notes.py` and creates the GitHub release from it. One changelog, two outputs, no drift.

clap is a library, so "distribution" means crates.io plus enabling downstream binaries to distribute well: `extras/clap/clap_complete` generates completions for bash, zsh, fish, PowerShell and elvish (snapshots in `extras/clap/clap_complete/tests/snapshots/`), `extras/clap/clap_complete_nushell` covers nushell, and `extras/clap/clap_mangen` renders man pages via the `roff` crate. Versioning strategy is notable: breaking changes for v5 are developed on master behind `unstable-v5` (see the feature graph in `extras/clap/Cargo.toml`), keeping one active branch instead of a long-lived diverging v5 branch.

### 12. Lessons for quinjet

quinjet already has a stricter clippy wall than clap, plus rustfmt, cargo-deny, taplo, typos, coverage, miri and mutants. What clap still adds:

1. Adopt trycmd markdown transcripts for every subcommand. Add dev-dependency `trycmd` (clap uses `trycmd = { version = "1.2.0", default-features = false, features = ["color-auto", "diff", "examples"] }` in `extras/clap/Cargo.toml`), create `tests/ui/*.toml` cases pinning `args`, `status.code`, `stdout`, `stderr` as in `extras/clap/tests/ui/help_flag_stdout.toml`, and write `.md` transcripts per subcommand so docs and end-to-end tests are the same artifact. Since every quinjet operation is a CLI subcommand, this covers the whole command surface.
2. Snapshot styled output as terminal SVGs. Add `snapbox` with `features = ["term-svg"]` (root `extras/clap/Cargo.toml`) and commit `.term.svg` snapshots like `extras/clap/tests/derive/snapshots/headers.term.svg`; for a ratatui app this pins help screens and error rendering including color, reviewable in a browser.
3. Use `automod::dir!` for integration test trees. `extras/clap/tests/builder/main.rs` shows one test binary spanning 46 topic files with zero mod declarations; quinjet gets fast link times and per-topic files for free with the `automod` crate.
4. Add a minimal-versions CI job: `cargo +nightly generate-lockfile -Z minimal-versions` then `cargo +stable check --workspace --all-features --locked`, exactly as in the `minimal-versions` job of `extras/clap/.github/workflows/ci.yml`, to prove declared dependency floors are real.
5. Add a lockfile-freshness job: `cargo update --workspace --locked` (the `lockfile` job in `extras/clap/.github/workflows/ci.yml`).
6. Use the aggregation-gate pattern: one job named `ci` with `needs: [...]`, `if: always()`, failing on `contains(needs.*.result, 'failure') || ... 'skipped'` (top of `extras/clap/.github/workflows/ci.yml`), so branch protection requires exactly one check and matrix changes never desync required-check names.
7. Pin the lint/format toolchain to a named stable with a `# STABLE` comment and let Renovate bump it via a custom regex manager (`extras/clap/.github/renovate.json5` matches `STABLE.*?(?<currentValue>\d+...)` across `Makefile` and workflow files, with `automerge: true`).
8. Move a scheduled `rust-next.yml` off the PR path: monthly cron testing beta and nightly plus a `cargo update` "latest dependencies" leg (`extras/clap/.github/workflows/rust-next.yml`), so toolchain and ecosystem breakage is discovered without blocking merges. quinjet's miri/mutants Makefile targets belong on such a cron too.
9. Track binary size as a benchmark: build with `CARGO_PROFILE_RELEASE_STRIP: true` and report `--file-size` of the release binary via `bencherdev/bencher` on every push to main (`extras/clap/.github/workflows/bench-baseline.yml`). For a TUI that people install, size regressions become visible per commit.
10. Encode house style as `disallowed-methods` in `.clippy.toml` with a `reason` per entry, and use `allow-unwrap-in-tests = true` / `allow-expect-in-tests = true` (`extras/clap/.clippy.toml`) instead of blanket test attributes; quinjet's restriction wall covers categories, this covers specific APIs the project has decided against.
11. Enforce conventional commits mechanically: `committed.toml` with `style="conventional"` plus the `crate-ci/committed` action on `pull_request` with `fetch-depth: 0` (`extras/clap/committed.yml` workflow), feeding straight into changelog discipline.
12. Automate the changelog-to-release pipeline: adopt `cargo-release` with `pre-release-replacements` rewriting `CHANGELOG.md` `<!-- next-header -->` / `<!-- next-url -->` markers (`[package.metadata.release]` in `extras/clap/Cargo.toml`), and a tag-triggered workflow that extracts the section into the GitHub release body (`extras/clap/.github/workflows/post-release.yml` + `release-notes.py`).
13. Harden workflows: top-level `permissions: contents: read`, per-job elevation only where needed, and `concurrency` groups with `cancel-in-progress: true` on every workflow (all files under `extras/clap/.github/workflows/`).
14. Set `panic = "abort"` in dev and release profiles plus `codegen-units = 1`, `lto = true` in release (`extras/clap/Cargo.toml` `[profile.*]`), and `opt-level = 1` for the test profile if the suite grows; also `[lib] bench = false` if benches are added, so `cargo bench` targets stay explicit.
15. Add a `debug` cargo feature with a no-op `debug!` macro twin (`extras/clap/clap_builder/src/macros.rs`): free-when-off tracing is more useful in a TUI, where stderr printing breaks the screen, than ad hoc logging.
16. Ship completions and man pages from the existing clap definition: `clap_complete` (and `clap_mangen` with the `roff` backend, `extras/clap/clap_mangen/Cargo.toml`) can generate them in a build script or a hidden subcommand; clap's own PTY test approach with `completest-pty` (`extras/clap/clap_complete/tests/testsuite/bash.rs`) shows how to verify them against real shells.
17. Keep exit codes as named constants with usage errors distinct from failures: `SUCCESS_CODE = 0`, `USAGE_CODE = 2` in `extras/clap/clap_builder/src/util/mod.rs`; quinjet subcommands should distinguish "bad invocation" (2) from "operation failed" (1) the same way.
18. Turn rustdoc into a gate: a docs CI job with `RUSTDOCFLAGS: -D warnings` running `cargo doc --all-features --no-deps --document-private-items` (`docs` job in `extras/clap/.github/workflows/ci.yml` plus the `doc` target in `extras/clap/Makefile`).

---

## Patterns

Nine cross-cutting syntheses. Each one names the consensus the eighteen
repositories share, the camps where they split and the reasoning on each side,
a comparison across the whole corpus, and a closing checklist a new Rust
project can apply directly.

### Chapters

- [Formatting and Style](./patterns/formatting-and-style.md): rustfmt camps, nightly
  options, editorconfig, and the formatters around Rust.

- [Lints and Static Analysis](./patterns/lints-and-static-analysis.md): lint tables,
  deny versus warn philosophy, clippy.toml knobs, and supply-chain scanners.

- [CI CD Patterns](./patterns/ci-cd-patterns.md): workflow architecture, matrices,
  caching, pinning, hardening, and release pipelines.

- [Project Structure](./patterns/project-structure.md): single crate versus workspace,
  module conventions, and where large files get split.

- [Testing Strategies](./patterns/testing-strategies.md): real-binary harnesses,
  snapshots, property tests, fuzzing, and coverage.

- [Error Handling and API Design](./patterns/error-handling-and-api-design.md): error
  types, exit codes, panic policy, and API discipline.

- [Rust Language Idioms](./patterns/rust-language-idioms.md): zero-copy, newtypes,
  concurrency selection, macros, and unsafe policy.

- [Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md):
  dependency hygiene, MSRV, changelogs, and shipping binaries.

- [Documentation Practices](./patterns/documentation-practices.md): rustdoc gates,
  manuals, templates, and generated references.

---

## Formatting and Style Across the Rust Ecosystem

Formatting is the least glamorous dimension of engineering practice and the one with the highest
consensus. Across the eighteen repositories studied here (rustdesk, tauri, deno, uv, zed, ripgrep,
alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap), every
single project machine-formats its Rust code and gates it in CI. The interesting variation is not
whether to format but how much configuration to allow, whether nightly-only rustfmt options are
worth the toolchain cost, and how far mechanical style enforcement extends beyond `.rs` files into
TOML, Markdown, YAML, and prose.

### Consensus practices

Four practices appear in effectively all eighteen projects.

**1. rustfmt is the only Rust formatter, and CI checks it.** No project uses an alternative
formatter or leaves formatting to convention. The enforcement command is nearly always
`cargo fmt --all -- --check` or a close variant. ripgrep's job is representative
(extras/ripgrep/.github/workflows/ci.yml):

```yaml
  rustfmt:
    runs-on: ubuntu-latest
    steps:
    ...
        components: rustfmt
    - name: Check formatting
      run: cargo fmt --all --check
```

tokio is the one project that avoids `cargo fmt` itself, for a documented reason
(extras/tokio/.github/workflows/ci.yml):

```yaml
      - name: "rustfmt --check"
        # Workaround for rust-lang/cargo#7732
        run: |
          if ! rustfmt --check --edition 2021 $(git ls-files '*.rs'); then
            printf "Please run \`rustfmt --edition 2021 \$(git ls-files '*.rs')\` to fix rustfmt errors.\nSee CONTRIBUTING.md for more details.\n" >&2
            exit 1
          fi
```

**2. The formatting config, when present, is tiny and version-pinned.** Most configs are one to
three lines, and the dominant content is not a style preference at all: it is an edition or
style_edition pin so that a toolchain upgrade cannot silently reformat the tree. uv, zed, and ruff
all ship the identical two-line file (extras/uv/rustfmt.toml, extras/zed/rustfmt.toml,
extras/ruff/rustfmt.toml):

```toml
edition = "2024"
style_edition = "2024"
```

bevy pins in the other direction, holding style_edition back to avoid churn
(extras/bevy/rustfmt.toml):

```toml
use_field_init_shorthand = true
newline_style = "Unix"
style_edition = "2021"
```

**3. Formatting-only commits are erased from blame.** zed and ruff both commit a
`.git-blame-ignore-revs` file listing bulk-reformat commits. zed documents the intent in the file
itself (extras/zed/.git-blame-ignore-revs):

```text
# This file consists of a list of commits that should be ignored for
# `git blame` purposes. This is useful for ignoring commits that only
# changed whitespace / indentation / formatting, but did not change
# the underlying syntax tree.
```

**4. Spelling of identifiers and docs is machine-checked.** Eight of the eighteen repositories
carry a `typos` configuration (extras/uv/_typos.toml, extras/zed/typos.toml,
extras/starship/typos.toml, extras/ruff/_typos.toml, extras/bevy/typos.toml,
extras/nushell/typos.toml, extras/gitui/typos.toml, extras/clap/typos.toml). tokio goes further
with cargo-spellcheck and a 328-line custom dictionary (extras/tokio/spellcheck.dic, configured in
extras/tokio/spellcheck.toml with `extra_dictionaries = ["spellcheck.dic"]`).

### The two rustfmt camps

#### The defaults camp (12 of 18)

The majority position is explicit: rustfmt defaults, zero style overrides, and often a config file
whose only job is to say so. Three projects ship a file that is nothing but a comment or literally
empty:

- extras/bat/rustfmt.toml and extras/fd/rustfmt.toml each contain exactly `# Defaults are used`.
- extras/helix/rustfmt.toml is a zero-byte file.
- extras/starship/.rustfmt.toml spells out why an empty file is better than no file:

```toml
# This file intentionally left almost blank
#
# The empty `rustfmt.toml` makes rustfmt use the default configuration,
# overriding any which may be found in the contributor's home or parent
# folders.
```

That comment names the real function of an empty config: it is a firewall against a contributor's
`~/.rustfmt.toml`. helix pairs its empty file with a toolchain pin so the rustfmt binary itself is
also fixed (extras/helix/rust-toolchain.toml):

```toml
[toolchain]
channel = "1.90.0"
components = ["rustfmt", "rust-src", "clippy"]
```

tokio, clap, and rustdesk have no rustfmt config file at the repository root at all; nushell's
config is the single line `edition = "2024"` (extras/nushell/rustfmt.toml); uv, zed, and ruff pin
editions only; bevy is defaults plus two stable one-liners. The reasoning on this side is
consistent across CONTRIBUTING files and config comments: defaults mean zero onboarding cost, no
debate surface, no nightly requirement, and no reformat churn when contributors' local versions
differ. rustdesk shows how the camp handles a genuine local need without a global rule: only one
library crate opts into one option (extras/rustdesk/libs/enigo/rustfmt.toml is exactly
`wrap_comments = true`).

#### The custom camp (6 of 18)

Six projects deliberately diverge, each for a different articulated reason.

**Density**: ripgrep compresses aggressively (extras/ripgrep/rustfmt.toml):

```toml
max_width = 79
use_small_heuristics = "max"
edition = "2024"
```

Both options are stable, so ripgrep gets a distinctive dense style with a plain stable toolchain.
gitui goes even narrower, and is the only project in the set that uses hard tabs
(extras/gitui/rustfmt.toml):

```toml
max_width = 70
hard_tabs = true
newline_style = "Unix"
```

gitui backs the tab decision with an editorconfig so non-rustfmt editors agree
(extras/gitui/.editorconfig):

```ini
root = true
[*.rs]
indent_style = tab
```

**Ecosystem alignment**: tauri and deno both come from mixed Rust and TypeScript codebases and pull
Rust toward web conventions: 2-space indentation and a narrower or explicit width. tauri writes out
every choice, including redundant defaults, so nothing is implicit
(extras/tauri/rustfmt.toml):

```toml
max_width = 100
hard_tabs = false
tab_spaces = 2
newline_style = "Unix"
...
force_explicit_abi = true
```

deno keeps its Rust config to three lines (extras/deno/.rustfmt.toml: `max_width = 80`,
`tab_spaces = 2`, `edition = "2024"`) and layers the rest through dprint, described below.

**Maximal opinion**: alacritty is the far end of the spectrum, with fifteen options including many
unstable ones such as `format_strings`, `normalize_comments`, `wrap_comments`,
`reorder_impl_items`, and `imports_granularity = "Module"` (extras/alacritty/rustfmt.toml).
meilisearch sits nearby with `unstable_features = true`, `use_small_heuristics = "max"`, and the
two import options (extras/meilisearch/.rustfmt.toml).

### Nightly-only formatting options

The custom camp splits again on how it pays for unstable options.

- **Pay openly with a nightly CI job.** alacritty's sourcehut build installs nightly rustfmt just
  to format (extras/alacritty/.builds/linux.yml):

```yaml
  - rustfmt: |
      cd alacritty
      rustup toolchain install nightly -c rustfmt
      cargo +nightly fmt -- --check
```

- **Format with nightly locally, check with stable in CI.** meilisearch's fmt job runs
  `cargo fmt --all -- --check` on a pinned stable 1.91.1 toolchain
  (extras/meilisearch/.github/workflows/test-suite.yml). Stable rustfmt warns and ignores the
  unstable keys, and because the default import behavior is Preserve, already-grouped imports pass
  the stable check untouched. The unstable style is therefore maintained by convention plus
  occasional nightly runs, not enforced per PR.

- **Refuse to pay, and document the deferral.** bevy keeps its wishlist in comments
  (extras/bevy/rustfmt.toml):

```toml
# The following lines may be uncommented on nightly Rust.
# Once these features have stabilized, they should be added to the always-enabled options above.
# unstable_features = true
# imports_granularity = "Crate"
# normalize_comments = true

# these options seem poorly implemented and cause churn, so, try to avoid them
# wrap_comments = true
# comment_width = 100
```

### Import grouping and ordering

Import layout is the single most wanted unstable feature. Four projects configure it; the rest
accept rustfmt's default alphabetical reordering within whatever groups the author wrote
(`reorder_imports` is on by default, and tauri restates it explicitly).

- deno wants one item per `use` and three groups (std, external, crate), and injects the options
  per invocation through dprint's exec plugin rather than the config file
  (extras/deno/.dprint.json):

```json
    "commands": [{
      "command": "rustfmt --config imports_granularity=item --config group_imports=StdExternalCrate",
      "exts": ["rs"],
      "cacheKeyFiles": [
        "rust-toolchain.toml",
        ".rustfmt.toml"
      ]
    }]
```

- meilisearch chooses `imports_granularity = "Module"` with the same `StdExternalCrate` grouping
  (extras/meilisearch/.rustfmt.toml).
- alacritty chooses `imports_granularity = "Module"` without grouping
  (extras/alacritty/rustfmt.toml).
- bevy would choose `imports_granularity = "Crate"` if it were stable (commented block above).

The lesson: item-level granularity (deno) optimizes for conflict-free diffs, module-level
(alacritty, meilisearch) optimizes for compact headers, and `StdExternalCrate` grouping is the
uncontested choice whenever grouping is configured at all.

### editorconfig

Six of eighteen repositories commit a `.editorconfig`: tauri, deno, uv, alacritty, ruff, and gitui.
Its role is to govern the files rustfmt never touches. uv's is the most instructive
(extras/uv/.editorconfig):

```ini
[*]
charset = utf-8
trim_trailing_whitespace = true
end_of_line = lf
indent_style = space
insert_final_newline = true
indent_size = 2

[*.{rs,py,pyi}]
indent_size = 4

[*.snap]
trim_trailing_whitespace = false

[*.md]
max_line_length = 100
```

Two details recur across these files. First, the base indent is 2 spaces for config and web files
with a 4-space override for Rust, matching rustfmt so editors and formatter never disagree
(extras/ruff/.editorconfig does the same). Second, snapshot and golden files are exempted from
whitespace fixing, because trailing whitespace in a captured output is data: uv exempts `*.snap`
and even one specific test file (`crates/uv/tests/help.rs`), and deno unsets the rules for `*.out`
expectation files and vendored Node tests (extras/deno/.editorconfig).

### TOML formatting

TOML manifests are the second most formatted file type, with three tools in play.

- **taplo** is the mainstream choice. tauri runs `taplo fmt --check --diff` as a dedicated CI job
  (extras/tauri/.github/workflows/fmt.yml), and bevy installs a pinned taplo 0.10.0 binary and runs
  the same command with a fix-it hint on failure (extras/bevy/.github/workflows/ci.yml). starship
  uses taplo as a validator rather than a formatter, linting preset files against a schema:
  `taplo lint --schema "file://${GITHUB_WORKSPACE}/.github/config-schema.json" docs/public/presets/toml/*.toml`
  (extras/starship/.github/workflows/format-workflow.yml).
- **tombi** is gitui's alternative, and its config is a model of a justified override
  (extras/gitui/tombi.toml):

```toml
# Keep dependency inline tables on a single line. Multi-line inline tables are
# TOML 1.1 syntax that Cargo on our MSRV (rust 1.88) rejects with
# "invalid inline table", so tombi must not expand them.
[format.rules]
line-width = 220
```

- **dprint's TOML plugin** covers deno and starship as part of their umbrella formatter
  (extras/deno/.dprint.json plugin list includes `toml-0.7.0.wasm`; extras/starship/.dprint.json
  declares a `"toml": {}` section).

### Markdown and YAML linting

No repository writes YAML style rules by hand; instead, three patterns cover prose and config
files.

- **One umbrella formatter.** deno's `.dprint.json` formats TypeScript, JSON, Markdown, TOML, and
  YAML (via the `pretty_yaml` plugin with `"quotes": "preferSingle"`) and shells out to rustfmt for
  `.rs`, so `deno run tools/format.js --check` is the entire formatting gate
  (extras/deno/tools/format.js). starship's dprint config formats Markdown at
  `"lineWidth": 100` (extras/starship/.dprint.json).
- **Prettier for the web-adjacent files.** tauri runs Prettier over JS/TS/MD with a commented
  `.prettierignore` (extras/tauri/.prettierignore explains, for example, that change files are
  hand-written and an IIFE script must not be formatted). zed pins the exact version in a script,
  `PRETTIER_VERSION=3.5.0` (extras/zed/script/prettier), with a one-key config
  `{ "printWidth": 120 }` (extras/zed/.prettierrc). ruff runs Prettier over YAML only, through
  pre-commit (extras/ruff/.pre-commit-config.yaml: `- id: prettier` with `types: [yaml]`).
- **Dedicated Markdown linters.** ruff layers mdformat (with mkdocs and footnote plugins) and
  markdownlint-cli in a priority-ordered pre-commit pipeline
  (extras/ruff/.pre-commit-config.yaml). bevy runs markdownlint through super-linter with a small
  policy file that disables the line-length rule and allowlists `<details>` and `<summary>`
  (extras/bevy/.github/linters/.markdown-lint.yml).

Workflow YAML gets correctness linting rather than style linting: actionlint (ruff via pre-commit
with a shellcheck integration, zed with extras/zed/.github/actionlint.yml for custom runner
labels), check-jsonschema's `check-github-workflows` hook (ruff), and zizmor for security auditing
(ruff, zed, uv). No project in the set uses yamllint.

### Naming conventions

None of the eighteen projects restates Rust's RFC 430 naming rules, because rustc's built-in
`non_snake_case`, `non_camel_case_types`, and `non_upper_case_globals` lints already enforce them.
What projects do write down is vocabulary:

- uv's STYLE.md legislates terminology down to identifier casing (extras/uv/STYLE.md):

```text
2. Use "pre-release", not "prerelease" (except in code, in which case: use `Prerelease`, not
   `PreRelease`; and `prerelease`, not `pre_release`).
```

- uv and ruff teach clippy the correct casing of domain words through `doc-valid-idents`
  (extras/uv/clippy.toml begins `doc-valid-idents = ["PyPI", "PubGrub", "PyPy", "CPython", ...]`),
  so doc comments cannot silently miscase product names.
- Workspace-wide crate prefixes act as a naming convention at the package level: `uv-*` (70
  crates), `bevy_*` (extras/bevy/crates), `helix-*` (extras/helix), `nu-*` (extras/nushell/crates),
  `tauri-*` (extras/tauri/crates). The prefix makes ownership and layering visible in every `use`
  statement.
- typos and cargo-spellcheck close the loop by rejecting misspelled identifiers and doc words, with
  every exception justified: zed's typos.toml carries a 118-line exclusion list and nushell's uses
  regex ignores for box-drawing characters in TUI fixtures.

### File and module size norms

No project enforces a maximum file length with tooling. The observed norm is that production
modules stay in the low thousands of lines and the outliers are either tests or deliberate
single-source-of-truth registries:

- The largest files are overwhelmingly tests: extras/zed/crates/editor/src/editor_tests.rs (43,169
  lines), extras/uv/crates/uv/tests/lock/lock.rs (38,196), extras/deno/tests/integration/lsp_tests.rs
  (22,700), extras/meilisearch/crates/meilisearch/tests/search/multi/proxy.rs (9,712).
- The largest intentional production file is extras/ripgrep/crates/core/flags/defs.rs at 8,161
  lines: every CLI flag as a unit struct in one file, because help text, man page, and completions
  all generate from that single registry and splitting it would scatter the source of truth.
- Projects that keep files small do it by crate granularity, not by file-length rules: zed's
  largest non-test production files sit inside a 250-crate workspace, tokio's biggest source file
  is 2,699 lines, gitui's is 1,959, and starship's is 2,332. fd shows the single-crate version of
  the same discipline: a flat 5k-line src/ with subdirectories only for real subsystems.

The practical norm to extract: keep a production module under roughly 2,000 to 3,000 lines, allow
generated-style registries and test files to grow without limit, and reach for a new module or
crate rather than a file-length lint.

### Comparison table: rustfmt posture

| Repository  | Config file                         | Stance   | Notable settings                                       | Nightly rustfmt needed |
|-------------|-------------------------------------|----------|--------------------------------------------------------|------------------------|
| rustdesk    | none at root (one lib crate only)   | defaults | enigo crate sets `wrap_comments = true`                | no                     |
| tauri       | rustfmt.toml                        | custom   | 2-space, width 100, `force_explicit_abi`               | no                     |
| deno        | .rustfmt.toml + dprint exec flags   | custom   | width 80, 2-space; imports via `--config` flags        | for import options     |
| uv          | rustfmt.toml                        | defaults | edition + style_edition 2024 only                      | no                     |
| zed         | rustfmt.toml                        | defaults | edition + style_edition 2024 only                      | no                     |
| ripgrep     | rustfmt.toml                        | custom   | width 79, `use_small_heuristics = "max"`               | no                     |
| alacritty   | rustfmt.toml                        | custom   | 15 options incl. wrap/normalize comments, Module imports | yes (CI installs it) |
| bat         | rustfmt.toml                        | defaults | `# Defaults are used`                                  | no                     |
| starship    | .rustfmt.toml                       | defaults | intentionally blank, blocks home-dir configs           | no                     |
| meilisearch | .rustfmt.toml                       | custom   | `unstable_features`, Module + StdExternalCrate imports | locally; stable CI     |
| ruff        | rustfmt.toml                        | defaults | edition + style_edition 2024 only                      | no                     |
| bevy        | rustfmt.toml                        | defaults | style_edition 2021 pin; nightly wishlist in comments   | no                     |
| helix       | rustfmt.toml                        | defaults | empty file; rustfmt pinned via rust-toolchain.toml     | no                     |
| fd          | rustfmt.toml                        | defaults | `# Defaults are used`                                  | no                     |
| nushell     | rustfmt.toml                        | defaults | `edition = "2024"` only; fmt enforced by git hook too  | no                     |
| tokio       | none                                | defaults | `rustfmt --check --edition 2021` over git ls-files     | no                     |
| gitui       | rustfmt.toml                        | custom   | width 70, `hard_tabs = true`                           | no                     |
| clap        | none                                | defaults | `cargo fmt --check` on pinned stable                   | no                     |

### Comparison table: non-Rust style enforcement

| Repository  | .editorconfig | TOML formatting            | Markdown / YAML tooling                                  |
|-------------|---------------|----------------------------|----------------------------------------------------------|
| rustdesk    | no            | none                       | Dart analyzer only; none for md/yaml                     |
| tauri       | yes           | taplo fmt --check in CI    | Prettier for JS/TS/MD with commented ignore file         |
| deno        | yes           | dprint TOML plugin         | dprint markdown + pretty_yaml plugins                    |
| uv          | yes           | none                       | Prettier in checks; md width 100 via editorconfig; zizmor |
| zed         | no            | none                       | Prettier 3.5.0 pinned in script; actionlint + zizmor     |
| ripgrep     | no            | none                       | none                                                     |
| alacritty   | yes           | none (editorconfig covers) | none; scdoc man pages compiled as a docs gate            |
| bat         | no            | none                       | none                                                     |
| starship    | no            | dprint TOML + taplo lint   | dprint markdown at lineWidth 100                         |
| meilisearch | no            | none                       | none                                                     |
| ruff        | yes           | none                       | mdformat + markdownlint; Prettier for YAML; actionlint   |
| bevy        | no            | taplo fmt --check in CI    | super-linter markdownlint with policy file               |
| helix       | no            | none                       | none; generated docs drift-checked instead               |
| fd          | no            | none                       | none                                                     |
| nushell     | no            | none                       | typos with regex ignores for TUI artifacts               |
| tokio       | no            | none                       | cargo-spellcheck with 328-line dictionary                |
| gitui       | yes           | tombi with justified width | none                                                     |
| clap        | no            | pre-commit toml/yaml checks | committed (commit lint) + typos                         |

### What a new Rust project should do

- [ ] Commit a rustfmt.toml even if you want defaults. Make it explicit like bat's
      `# Defaults are used` (extras/bat/rustfmt.toml) or starship's commented blank file, so a
      contributor's home config can never leak in.
- [ ] Pin `edition` and `style_edition` in that file (extras/uv/rustfmt.toml pattern) so a
      toolchain bump cannot reformat the tree; when it must, record the bulk commit in
      `.git-blame-ignore-revs` (extras/zed/.git-blame-ignore-revs pattern).
- [ ] Enforce with `cargo fmt --all -- --check` in CI on a pinned toolchain that installs the
      `rustfmt` component; optionally mirror it in a versioned git hook like
      extras/nushell/.githooks/pre-commit.
- [ ] Skip nightly-only options at first. If you want import grouping later, choose
      `group_imports = "StdExternalCrate"` (the unanimous choice where configured) and decide
      openly how to pay: a nightly fmt CI job (alacritty), per-invocation `--config` flags (deno),
      or convention plus a stable check (meilisearch).
- [ ] Add a `.editorconfig` for the files rustfmt does not own: LF, final newline, trimmed
      whitespace, 2-space base indent with a 4-space `[*.rs]` override, and explicit exemptions for
      snapshot or golden files (extras/uv/.editorconfig pattern).
- [ ] Format TOML with taplo (`taplo fmt --check --diff` as in extras/tauri/.github/workflows/fmt.yml);
      if a formatter choice interacts with your MSRV, write the reason into the config the way
      extras/gitui/tombi.toml does.
- [ ] Pick one tool for Markdown and pin its version: dprint if you want an umbrella formatter,
      Prettier via a pinned script (extras/zed/script/prettier), or mdformat plus markdownlint in
      pre-commit (extras/ruff/.pre-commit-config.yaml). Disable the Markdown line-length rule or
      set it deliberately; do not leave it to defaults.
- [ ] Lint workflow YAML for correctness, not style: actionlint with a config for custom runner
      labels (extras/zed/.github/actionlint.yml) plus zizmor for security.
- [ ] Add `typos` with a curated exception file from day one, and `doc-valid-idents` entries in
      clippy.toml for every product name your docs will use (extras/uv/clippy.toml pattern).
- [ ] Write naming and terminology rules only where the compiler cannot: a short STYLE.md for
      user-facing wording and identifier vocabulary (extras/uv/STYLE.md), and consistent crate
      prefixes (`project-*`) once you split into a workspace.
- [ ] Do not add a file-length lint. Keep production modules roughly under 2,000 to 3,000 lines by
      splitting modules and crates, and accept large files only when they are tests or a deliberate
      single source of truth like extras/ripgrep/crates/core/flags/defs.rs.

---

## Lints and Static Analysis

This chapter surveys how eighteen production Rust repositories (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap) configure clippy, rustc lints, `clippy.toml`, `cargo-deny` and friends, spell checkers, and their own custom check scripts. The single most important finding: the best projects treat lint configuration as executable architecture documentation, not as style policing. A `disallowed-methods` entry with a `reason` string is a design decision the compiler enforces forever.

### 1. Consensus practices

#### 1.1 Zero warnings, enforced at CI rather than in source

Sixteen of eighteen projects run clippy in CI and fail on any warning. The dominant pattern keeps lint levels at `warn` in configuration and escalates to deny only at the CI boundary, either with `-D warnings` on the command line or `RUSTFLAGS=-Dwarnings` in the workflow environment:

- extras/helix/.github/workflows/build.yml line 87: `cargo clippy --workspace --all-targets -- -D warnings`
- extras/bat/.github/workflows/CICD.yml line 63: `cargo clippy --locked --all-targets --all-features -- -D warnings`
- extras/uv/.github/workflows/check-lint.yml line 130: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- extras/starship/.github/workflows/workflow.yml line 53: `cargo clippy --workspace --locked -- -D warnings`
- extras/tauri/.github/workflows/lint-rust.yml line 82 runs `clippy --target ... --all-targets --all-features -- -D warnings` across a five-target matrix including iOS and Android
- extras/tokio/.github/workflows/ci.yml line 14 sets `RUSTFLAGS: -Dwarnings` at workflow level, so every job, not just clippy, fails on warnings
- extras/meilisearch/.github/workflows/test-suite.yml line 239: `cargo clippy --all-targets ${{ matrix.features }} -- --deny warnings -D clippy::todo`

The reasoning is consistent everywhere it is written down: `deny` in source breaks local iterative builds on unrelated toolchain updates, while `warn` locally plus `-D` in CI gives contributors a quiet inner loop and the project an absolute wall. Zed states the cost side explicitly in its workspace table (extras/zed/Cargo.toml):

```toml
# We currently do not restrict any style rules
# as it slows down shipping code to Zed.
#
# Running ./script/clippy can take several minutes, and so it's
# common to skip that step and let CI do it.
style = { level = "allow", priority = -1 }
```

The two abstainers are instructive. rustdesk's clippy job is commented out in extras/rustdesk/.github/workflows/ci.yml (lines 56 to 61). ripgrep has no clippy invocation anywhere in the repository; its quality gates are `#![deny(missing_docs)]` in all eight library crate roots (extras/ripgrep/crates/globset/src/lib.rs line 112, extras/ripgrep/crates/searcher/src/lib.rs line 84, and six siblings) plus a CI rustdoc gate with `RUSTDOCFLAGS -D warnings`. Opting out of clippy is viable, but only when replaced by other mechanical gates, not by taste.

#### 1.2 `disallowed-methods` and `disallowed-types` as architecture enforcement

Ten projects (deno, uv, zed, ruff, bevy, starship, meilisearch, nushell, clap, and deno's per-crate variants) use `clippy.toml` bans to turn architectural rules into compile errors. This is the signature practice of the corpus. The bans are never stylistic; each encodes an invariant that a code review would otherwise have to catch by hand:

uv forbids raw filesystem APIs so every I/O call flows through its `uv-fs` wrapper crate (extras/uv/clippy.toml):

```toml
disallowed-types = [
  "std::fs::DirEntry",
  "std::fs::File",
  "std::fs::OpenOptions",
  "std::fs::ReadDir",
  ...
]

disallowed-methods = [
  ...
  "std::fs::canonicalize",
  "std::fs::copy",
  "std::fs::create_dir",
  ...
]
```

zed bans blocking process spawns on the UI thread and non-deterministic timers in tests, with a `reason` and a `replacement` for every entry (extras/zed/clippy.toml):

```toml
disallowed-methods = [
    { path = "std::process::Command::spawn", reason = "Spawning `std::process::Command` can block the current thread for an unknown duration", replacement = "smol::process::Command::spawn" },
    { path = "smol::Timer::after", reason = "smol::Timer introduces non-determinism in tests", replacement = "gpui::BackgroundExecutor::timer" },
]
```

bevy bans every `f32` transcendental to force calls through deterministic wrappers (extras/bevy/clippy.toml):

```toml
disallowed-methods = [
  { path = "f32::powi", reason = "use bevy_math::ops::FloatPow::squared, bevy_math::ops::FloatPow::cubed, or bevy_math::ops::powf instead for libm determinism" },
  { path = "f32::sin", reason = "use bevy_math::ops::sin instead for libm determinism" },
]
```

starship bans three specific hazards with a comment per ban (extras/starship/clippy.toml):

```toml
disallowed-methods = [
  # std::process::Command::new may inadvertly run executables from the current working directory
  "std::process::Command::new",
  # Setting environment variables can cause issues with non-rust code
  "std::env::set_var",
  # use `dunce` to avoid UNC/verbatim paths, where possible
  "std::fs::canonicalize",
]
```

meilisearch's single entry encodes a security invariant (extras/meilisearch/clippy.toml): `tar::Archive::unpack` is banned in favor of a path-traversal-safe wrapper. nushell bans a type rather than a method, with a replacement (extras/nushell/clippy.toml):

```toml
[[disallowed-types]]
path = "std::time::Instant"
reason = "WASM panics if used, use instead"
replacement = "nu_utils::time::Instant"
```

deno takes this the furthest: `clippy.toml` is per crate (about thirty of them under extras/deno/cli, extras/deno/ext, extras/deno/runtime, extras/deno/libs), and a custom lint (`ensureDisallowedMethodsEnforced` in extras/deno/tools/lint.js) fails CI when any ext or libs crate is missing a `clippy.toml` or missing a required ban such as `std::env::current_dir` or `std::time::SystemTime::now`. The lint config itself is linted. The cli crate's file shows the layering intent (extras/deno/cli/clippy.toml):

```toml
disallowed-methods = [
  { path = "reqwest::Client::new", reason = "create an HttpClient via an HttpClientProvider instead" },
  { path = "std::process::exit", reason = "use deno_runtime::exit instead" },
]
```

Banning `std::process::exit` funnels the entire program through one exit path, which is what makes exit-code discipline testable.

#### 1.3 Every exception carries a written reason

Across all projects that suppress anything, the suppression is annotated: gitui's `deny.toml` links an upstream issue for every `skip-tree` entry, bevy's advisory ignores each cite the RUSTSEC page and the blocking dependency, tauri's `.cargo/audit.toml` explains each ignored advisory, and ruff enables `clippy::disallowed_methods` per crate with a machine-checked reason (extras/ruff/crates/ty_python_semantic/src/lib.rs):

```rust
#![warn(
    clippy::disallowed_methods,
    reason = "Prefer System trait methods over std methods in ty crates"
)]
```

bevy goes further and makes undocumented suppression itself a lint (extras/bevy/Cargo.toml): `allow_attributes = "warn"` and `allow_attributes_without_reason = "warn"`, and deno denies `clippy::allow_attributes_without_reason` on the CI command line (extras/deno/tools/lint.js, `clippyDenyFlags`).

#### 1.4 Pinned lint toolchains

Because clippy adds lints every release, projects that gate merges on it pin the toolchain: tokio sets `rust_clippy: '1.88'` in extras/tokio/.github/workflows/ci.yml (line 22), meilisearch pins 1.91.1 with the clippy component in extras/meilisearch/rust-toolchain.toml, deno pins 1.95.0 with clippy in extras/deno/rust-toolchain.toml, gitui mirrors its MSRV into extras/gitui/.clippy.toml (`msrv = "1.88.0"`), and fd runs a second clippy pass on the exact MSRV toolchain (extras/fd/.github/workflows/CICD.yml line 81: "Run clippy (on minimum supported rust version to prevent warnings we can't fix)").

### 2. Divergent camps

#### 2.1 Where the lint policy lives

Three camps exist, and the choice tracks repository shape:

1. Workspace `[lints]` tables, inherited by every crate via `[lints] workspace = true`: uv, ruff, clap, nushell, tokio, zed, bevy. This is the modern default for multi-crate workspaces. zed enforces adoption mechanically: its xtask conformity check fails if any crate opts out (extras/zed/tooling/xtask/src/tasks/package_conformity.rs, lines 28 to 31: `let is_using_workspace_lints = cargo_toml.lints.is_some_and(|lints| lints.workspace);`). bevy documents the one real limitation in extras/bevy/Cargo.toml: cargo cannot override workspace lints per crate (rust-lang/cargo#13157), so bevy duplicates the table with overrides applied for the root package.
2. Crate-root attributes: gitui, alacritty, bat, tauri, ripgrep. gitui's wall is the most aggressive in the corpus (extras/gitui/asyncgit/src/lib.rs):

   ```rust
   #![forbid(missing_docs)]
   #![deny(clippy::all, clippy::perf, clippy::nursery, clippy::pedantic)]
   #![deny(
    clippy::filetype_is_file,
    clippy::cargo,
    clippy::unwrap_used,
    clippy::panic,
    ...
   )]
   ```

   alacritty uses three lines plus a clippy-only escalation (extras/alacritty/alacritty_terminal/src/lib.rs):

   ```rust
   #![warn(rust_2018_idioms, future_incompatible)]
   #![deny(clippy::all, clippy::if_not_else, clippy::enum_glob_use)]
   #![cfg_attr(clippy, deny(warnings))]
   ```

3. External injection: deno keeps its hard denies out of both source and manifests, in rustflags that apply to every build (extras/deno/.cargo/config.toml):

```toml
[target.'cfg(all())']
rustflags = [
  "-D", "clippy::all",
  "-D", "clippy::await_holding_refcell_ref",
  "-D", "clippy::missing_safety_doc",
  "-D", "clippy::undocumented_unsafe_blocks",
  "--cfg", "tokio_unstable",
]
```

The workspace-table camp values one source of truth and `priority` control; the crate-attribute camp values visibility (the policy is in the file you are editing) and per-crate variation; deno's rustflags approach guarantees the denies apply to plain `cargo build`, not only when someone remembers to run clippy.

#### 2.2 Pedantic wall versus curated list versus defaults only

- Pedantic-on-by-default: uv and ruff set `pedantic = { level = "warn", priority = -2 }` then allow a documented list of exceptions (extras/uv/Cargo.toml, extras/ruff/Cargo.toml). ruff annotates individual allows, for example `needless_continue = "allow" # An explicit continue can be more readable`. gitui reaches the same end state via crate attributes, denying `all + perf + nursery + pedantic`.
- Curated warn list: clap hand-picks roughly sixty lints in `[workspace.lints.clippy]` (extras/clap/Cargo.toml), from `str_to_string` to `lossy_float_literal`, with inline notes such as `let_and_return = "allow"  # sometimes good to name what you are returning`.
- Defaults only: helix, fd, bat, meilisearch, alacritty run stock `clippy::all` and rely on `-D warnings`. helix holds 107k lines to only about 41 scoped `#[allow]` attributes with default lints; fd has exactly one clippy allow in all of src/ (extras/fd/src/walk.rs line 38, `#[allow(clippy::large_enum_variant)]`).

The pedantic camp argues the allow list is cheaper to maintain than reviewing for the same issues by hand. The defaults camp argues pedantic noise trains contributors to reach for `#[allow]`, and that a small clean codebase does not need it. Both camps produce near-zero suppression counts; what matters is that the choice is enforced, not which choice is made.

#### 2.3 True `deny` levels: reserved for correctness classes

Where projects do put `deny` in configuration, it marks classes of bug, not style: nushell denies `unwrap_used` and `unchecked_time_subtraction` workspace-wide while merely warning on everything else (extras/nushell/Cargo.toml); zed denies `dbg_macro`, `todo`, `declare_interior_mutable_const`, `redundant_clone`, and `disallowed_methods` (extras/zed/Cargo.toml); bevy denies `unsafe_code` across the workspace with per-crate `expect(reason)` opt-ins (extras/bevy/Cargo.toml, `unsafe_code = "deny"`); bat denies `unsafe_code` at both crate roots (extras/bat/src/lib.rs line 22, extras/bat/src/bin/bat/main.rs line 1); gitui uses `forbid(unsafe_code)` so not even an allow can reopen the door.

#### 2.4 Restriction lints for output hygiene

The CLI-heavy projects converge on the print-macro restriction lints because `println!` panics on a closed pipe. uv and ruff warn on `print_stdout`, `print_stderr`, `dbg_macro`, `exit`, `get_unwrap`, `rc_buffer`, `rc_mutex`, and `rest_pat_in_fully_bound_structs` under a literal `# Disallowed restriction lints` heading (extras/uv/Cargo.toml). deno denies the same pair and even patches the diagnostic (extras/deno/tools/lint.js):

```js
    // the std print macros panic on broken pipes (ex. `deno test | head`);
    // prefer the `log` crate for diagnostics and deno_print's drop_println!
    // macros for stdout output, or ignore these print_* rules if necessary
    "--deny",
    "clippy::print_stderr",
    "--deny",
    "clippy::print_stdout",
```

The ban only works because a sanctioned wrapper exists; every project pairs the restriction with a replacement (deno's `drop_println!`, bevy's `bevy_math::ops`, uv's `uv-fs`).

#### 2.5 rustc lints: `unexpected_cfgs` as a custom-cfg registry

Projects with custom `--cfg` flags register them through the `unexpected_cfgs` lint so typos in cfg names fail the build: tokio lists nine (extras/tokio/Cargo.toml, `check-cfg = ['cfg(fuzzing)', 'cfg(loom)', ... 'cfg(tokio_unstable)']`), ruff registers `cfg(fuzzing)` and `cfg(codspeed)` (extras/ruff/Cargo.toml), and nushell registers `cfg(ci)` (extras/nushell/Cargo.toml). The other consensus rustc lints are `unreachable_pub = "warn"` (uv, ruff, clap), `unsafe_op_in_unsafe_fn` (clap, bevy), and `missing_docs` (bevy warns workspace-wide, ripgrep and gitui deny or forbid per crate, tauri warns via `#![warn(missing_docs, rust_2018_idioms)]` at extras/tauri/crates/tauri/src/lib.rs line 55).

#### 2.6 `clippy.toml` tuning knobs beyond the bans

The corpus uses a wide set of secondary knobs: `msrv` and `cognitive-complexity-threshold = 18` (extras/gitui/.clippy.toml); `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-print-in-tests`, `allow-dbg-in-tests` so production restrictions do not poison test code (extras/clap/.clippy.toml, extras/nushell/clippy.toml); `doc-valid-idents` to stop `doc_markdown` mangling product names such as `"PyPI"` and `"PowerShell"` (extras/uv/clippy.toml, extras/ruff/clippy.toml, extras/bevy/clippy.toml, extras/clap/.clippy.toml); `ignore-interior-mutability` for false-positive `mutable_key_type` hits, each with a comment (extras/zed/clippy.toml, extras/ruff/clippy.toml); `avoid-breaking-exported-api = false` for pre-1.0 freedom (extras/zed/clippy.toml); `check-private-items = true` so doc lints reach private code (extras/bevy/clippy.toml); and `standard-macro-braces` to standardize `children![...]` (extras/bevy/clippy.toml).

#### 2.7 Supply chain: four tiers of paranoia

1. Full cargo-deny: tokio, bevy, gitui, starship, clap. tokio's is the strictest license posture in the corpus (extras/tokio/deny.toml): `allow = ["MIT", "Apache-2.0"]` with a single `Unicode-3.0` exception for `unicode-ident`, plus `wildcards = "deny"` and `unknown-registry = "deny"` / `unknown-git = "deny"` under `[sources]`. gitui uniquely sets `multiple-versions = "deny"` and documents every `skip-tree` escape with the upstream issue that forces it (extras/gitui/deny.toml, for example `# currently needed due to: * dirs-sys v0.4.1 (https://github.com/dirs-dev/dirs-sys-rs/issues/29)`).
2. Advisory-only scanning: bat and tauri run cargo-audit with a versioned ignore file (extras/bat/.cargo/audit.toml, extras/tauri/.cargo/audit.toml, where every RUSTSEC id carries a comment such as `# proc-macro-error is unmaintained`); nushell runs rustsec via a dedicated workflow (extras/nushell/.github/workflows/audit.yml). starship splits its cargo-deny legs so a brand-new advisory cannot redden unrelated PRs (advisories run `continue-on-error`), invoked through a SHA-pinned action (extras/starship/.github/workflows/security-audit.yml line 27).
3. Human audit trails: tauri is the only project running cargo-vet, importing third-party audit sets so most dependencies arrive pre-audited (extras/tauri/supply-chain/config.toml: imports from bytecode-alliance, embark-studios, google, isrg, mozilla, zcash).
4. Nothing beyond a lockfile: ripgrep, alacritty, helix, fd, rustdesk, deno, meilisearch, uv, zed. These lean on exact pins, `--locked` builds, and update bots (Renovate or dependabot) instead of scanners. Several compensate with unused-dependency scanners: `cargo shear --deny-warnings` (extras/uv/.github/workflows/check-lint.yml line 196, extras/ruff/.github/workflows/ci.yaml line 830), cargo-machete with a metadata ignore list (zed), cargo-udeps on nightly (tauri, gitui).

#### 2.8 Spell checkers

Nine projects check spelling mechanically. Eight use `typos` with a curated exception file: extras/uv/_typos.toml, extras/zed/typos.toml (a 118-line exclusion list where every entry says why, for example `# Contributor names aren't typos.`), extras/ruff/_typos.toml, extras/bevy/typos.toml, extras/nushell/typos.toml, extras/starship/typos.toml, extras/gitui/typos.toml, extras/clap/typos.toml. tokio alone uses cargo-spellcheck with a committed dictionary whose first line is a word count, validated for sortedness and uniqueness by a CI shell step (extras/tokio/spellcheck.dic, extras/tokio/.github/workflows/ci.yml around line 1263). The `typos` camp wins on setup cost; the tokio approach wins on documentation-heavy crates where API names dominate prose.

#### 2.9 Custom static analysis where clippy cannot reach

The mature projects all grow at least one bespoke checker:

- zed writes real compiler plugins with dylint when a rule needs type information, pinned to their own nightly (extras/zed/tooling/lints/rust-toolchain.toml, `channel = "nightly-2026-03-21"`). Its `BLOCKING_IO_ON_FOREGROUND` lint flags `std::fs` calls inside functions holding a synchronous UI context (extras/zed/tooling/lints/src/blocking_io_on_foreground.rs).
- nushell uses ast-grep structural rules with autofixes, wired through extras/nushell/sgconfig.yml and snapshot-tested; a rule is ten lines of YAML (extras/nushell/ast-grep/rules/empty_if_branch.yml: `message: "An empty block if(-else) expression is confusing or a potential bug."`).
- uv runs hawk, its own public-API dead-code linter, as `cargo +1.97.1 hawk check --target-dir target/hawk -D warnings` with reasoned overrides in extras/uv/hawk.toml (`[[override]] lint = "hawk::unnecessary_public"`).
- helix's xtask implements domain lints no general tool could express: `query-check`, `indent-check`, `highlight-check`, `theme-check`, and `docgen` drift detection (extras/helix/xtask/src/main.rs).
- deno's tools/lint.js layers repo-structure checks (the clippy.toml completeness audit, copyright headers, unreferenced expectation files) on top of clippy.
- uv, ruff, bevy, and zed also lint their CI itself with zizmor and actionlint (extras/uv/.github/workflows/check-zizmor.yml, extras/bevy CodeQL actions coverage).

### 3. Comparison table

| Repository | Workspace `[lints]` | `clippy.toml` | CI enforcement | Dependency scanning | Spelling | Custom checks |
|---|---|---|---|---|---|---|
| rustdesk | no | no | clippy job commented out | dependabot only | no | build/packaging scripts only |
| tauri | no, crate attrs (`warn(missing_docs)`) | no | `-D warnings`, 5-target matrix | cargo-audit + cargo-vet + udeps | no | license-header and change-tag scripts |
| deno | no, denies via `.cargo/config.toml` rustflags | per crate, ~30 files | `-D warnings` + deny flags in tools/lint.js | exact pins, no scanner | no | tools/lint.js repo audits |
| uv | yes, pedantic warn + restriction warns | yes, fs/env bans | `-D warnings` on Linux and Windows | cargo-shear; Renovate | typos | hawk, zizmor |
| zed | yes, targeted denies, style allowed | yes, reason + replacement | script/clippy `--deny warnings` | cargo-machete; Renovate | typos | dylint, xtask conformity |
| ripgrep | no | no | no clippy at all; rustdoc `-D warnings` | none | no | ci/test-complete, invariant tests |
| alacritty | no, crate attrs | no | `cfg_attr(clippy, deny(warnings))`; sourcehut `RUSTFLAGS=-D warnings` per-feature tests | none | no | completion drift test |
| bat | no, `deny(unsafe_code)` roots | no | `-D warnings`, all targets/features | cargo-audit + ignore list | no | changelog-entry CI grep |
| starship | no | yes, 3 commented bans | `-D warnings` on stable, 3 OS | cargo-deny, split legs | typos | schema drift check |
| meilisearch | no | yes, tar unpack ban | `--deny warnings -D clippy::todo`; toolchain 1.91.1 | pins only | no | xtask, OpenAPI lints |
| ruff | yes, pedantic warn + nursery picks | yes, System-trait bans | `-D warnings` | cargo-shear | typos | zizmor, actionlint, pre-commit |
| bevy | yes, `unsafe_code = "deny"`, `missing_docs` warn | yes, f32 bans | tools/ci crate with `-D warnings` | cargo-deny | typos | tools/ci, CodeQL, zizmor |
| helix | no | no | default clippy `-D warnings` | none | no | xtask query/indent/theme/docgen |
| fd | no | no | `-Dwarnings` on stable and MSRV | none | no | version-bump scripts |
| nushell | yes, `unwrap_used = "deny"` | yes, Instant ban | `warnings = "warn"` table + `-D warnings` escalation | cargo-audit workflow | typos | ast-grep rules |
| tokio | yes, mostly `check-cfg` registry | no | `RUSTFLAGS=-Dwarnings`; clippy pinned to 1.88 | cargo-deny, PR + daily cron | cargo-spellcheck | check-external-types, semver-checks |
| gitui | no, per-crate deny walls | yes, msrv + complexity | `make clippy` in CI | cargo-deny `multiple-versions = "deny"`; udeps | typos | changelog extraction |
| clap | yes, ~60 curated lints | yes, style bans + test allows | `-D warnings` on pinned stable | cargo-deny + audit-check | typos | committed (commit lint) |

### 4. What a new Rust project should do

1. Create `[workspace.lints]` in the root manifest and put `[lints] workspace = true` in every crate; add a conformity check so new crates cannot opt out silently (extras/zed/tooling/xtask/src/tasks/package_conformity.rs is 40 lines).
2. Keep levels at `warn` in the table; make CI the wall with `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`. Never put `deny(warnings)` in source.
3. Start `[workspace.lints.rust]` with `unreachable_pub = "warn"`, `unsafe_code = "warn"` (or `deny` with `expect(reason)` opt-ins), and `unexpected_cfgs` with `check-cfg` entries for every custom cfg you introduce.
4. Reserve real `deny` for bug classes: `unwrap_used`, `dbg_macro`, `todo`, `undocumented_unsafe_blocks`. Follow nushell and zed, not a blanket wall.
5. Add `clippy.toml` on day one and treat `disallowed-methods` / `disallowed-types` as your architecture register: ban `std::process::exit` outside one exit path, raw print macros once you have an output layer, and any API that must flow through a sanctioned wrapper. Every entry gets `reason`, and `replacement` where one exists.
6. Set the test escape hatches (`allow-unwrap-in-tests = true` and friends) so production restrictions do not corrode test code, and populate `doc-valid-idents` instead of sprinkling backticks.
7. Warn on `allow_attributes_without_reason` (bevy) so every future suppression documents itself; prefer `#[expect(lint, reason = "...")]` over `#[allow]`.
8. Pin the clippy toolchain (a `rust-toolchain.toml` with the clippy component, or a CI variable like tokio's `rust_clippy: '1.88'`) and mirror your MSRV into `clippy.toml` `msrv` so clippy never suggests too-new APIs.
9. Add `deny.toml` with a license allowlist, `wildcards = "deny"`, and `unknown-registry = "deny"`; run it path-filtered on PRs plus a daily cron so new advisories surface between merges (tokio's split). Every `ignore` and `skip-tree` entry cites an issue link (gitui's discipline).
10. Add `typos` with a config file for exceptions, `cargo shear --deny-warnings` (or machete) for dead dependencies, and zizmor plus actionlint over your workflows.
11. Gate rustdoc as a lint: `RUSTDOCFLAGS="-D warnings"` with `--document-private-items` (ripgrep, bat, clap all do; it catches broken intra-doc links that clippy cannot).
12. When a rule is repo-specific and clippy cannot express it, write a checker rather than a convention: an xtask (helix), an ast-grep rule (nushell), or a dylint library (zed), in roughly that order of escalation cost. Then make CI run it, because an unenforced convention is a wish.

---

## CI/CD Patterns

Continuous integration is where a Rust project's engineering values become enforceable. Across the eighteen repositories studied here (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap), the CI systems range from a single 464-line file to 39-job generated pipelines, yet a surprisingly stable core of practices repeats. This chapter maps the consensus, the genuine disagreements, and the concrete mechanics worth copying.

### Consensus practices

Nearly every project in the set converges on the following, independent of size or domain.

**Trigger discipline: pull_request plus push-to-default plus workflow_dispatch.** The baseline trigger set appears everywhere from ripgrep (extras/ripgrep/.github/workflows/ci.yml) to fd (extras/fd/.github/workflows/CICD.yml). Docs-only changes are kept out of build lanes with path filters: rustdesk excludes `docs/**`, `README.md`, and packaging directories in extras/rustdesk/.github/workflows/flutter-ci.yml, and tauri goes further with per-crate `paths:` filters in extras/tauri/.github/workflows/test-core.yml.

**Concurrency groups that cancel superseded runs.** Fifteen of the eighteen define a `concurrency:` block. The dominant shape is the one in extras/clap/.github/workflows/ci.yml:

```yaml
concurrency:
  group: "${{ github.workflow }}-${{ github.ref }}"
  cancel-in-progress: true
```

Projects that also build main or run merge queues refine this so only pull request runs are cancellable, as in extras/helix/.github/workflows/build.yml:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.head_ref || github.run_id }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

bevy uses the identical conditional in extras/bevy/.github/workflows/ci.yml. The exceptions are ripgrep, alacritty, bat, fd, and gitui, all of which simply let redundant runs finish.

**Warnings as errors, builds with --locked.** Whether via `RUSTFLAGS: "-D warnings"` at workflow env level (extras/meilisearch/.github/workflows/test-suite.yml, line 14) or clippy invocations like `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extras/ruff/.github/workflows/ci.yaml, line 325), every project makes warnings fatal somewhere in CI, and every project with a committed lockfile builds `--locked`.

**fail-fast: false on matrices.** rustdesk, ripgrep, fd, gitui, and others disable fail-fast so one broken target does not hide the state of the rest (extras/gitui/.github/workflows/cd.yml, `fail-fast: false`).

**Least-privilege token permissions.** Sixteen of eighteen restrict the `GITHUB_TOKEN`. ripgrep documents the reasoning inline in extras/ripgrep/.github/workflows/ci.yml:

```yaml
# The section is needed to drop write-all permissions that are granted on
# `schedule` event. By specifying any permission explicitly all others are set
# to none. By using the principle of least privilege the damage a compromised
# workflow can do (because of an injection or compromised third party tool or
# action) is restricted.
permissions:
  # to fetch code (actions/checkout)
  contents: read
```

uv is the strictest: extras/uv/.github/workflows/ci.yml opens with `permissions: {}` and every checkout in the repository sets `persist-credentials: false` (130 occurrences under extras/uv/.github). ruff (57), bevy (39), and fd follow the same pattern.

**Scheduled work runs off the PR path.** Sixteen of eighteen carry at least one cron trigger. Only alacritty and fd (plus bat) have none at all.

**Tag-triggered releases that cross-check the version.** Every project that ships binaries triggers its release pipeline on `v*` tags and most verify the tag against the manifest before building, a pattern detailed later in this chapter.

### Workflow architecture: three camps

```text
Workflow architecture
|
+-- Monolith: one file is both CI and CD
|     bat   extras/bat/.github/workflows/CICD.yml   (464 lines, release steps ref-gated)
|     fd    extras/fd/.github/workflows/CICD.yml    (PR, push, tag, dispatch in one file)
|
+-- Orchestrator of reusable workflow_call units
|     uv        extras/uv/.github/workflows/ci.yml calls plan.yml, check-*.yml
|     rustdesk  thin trigger shells call flutter-build.yml (workflow_call, 2477 lines)
|     tauri     21 per-concern workflows, each path-filtered
|
+-- Generated workflows: YAML is a build artifact
      deno  TypeScript ci.ts emits ci.generated.yml, drift-checked
      zed   cargo xtask workflows emits YAML from Rust, drift-checked
```

The monolith camp optimizes for a single source of truth. bat's and fd's CICD.yml begin with a `crate_metadata` job that extracts name, version, and MSRV from `cargo metadata`, so the manifest drives both testing and packaging (extras/fd/.github/workflows/CICD.yml, the `crate_metadata` job). The cost is a long file where release logic and PR logic interleave behind `if: startsWith(github.ref, 'refs/tags/')` guards.

The orchestrator camp splits triggers from logic. rustdesk's extras/rustdesk/.github/workflows/flutter-ci.yml is nothing but a shell:

```yaml
jobs:
  run-ci:
    uses: ./.github/workflows/flutter-build.yml
    with:
      upload-artifact: false
```

The same reusable build workflow (extras/rustdesk/.github/workflows/flutter-build.yml, `on: workflow_call:`) is invoked by the PR shell, the nightly cron shell (flutter-nightly.yml), and the tag shell (flutter-tag.yml), so PRs, nightlies, and releases cannot drift apart. uv applies the same idea at finer granularity: extras/uv/.github/workflows/ci.yml is a pure dispatcher whose jobs are all `uses:` lines, gated by outputs of a change-detection workflow.

The generated camp treats YAML as untrustworthy at scale. deno's extras/deno/.github/workflows/ci.generated.yml opens with `# GENERATED BY ./ci.ts -- DO NOT DIRECTLY EDIT`; the 39-job pipeline, its cache keys, and its aggregation job are all emitted from typed TypeScript in extras/deno/.github/workflows/ci.ts. zed does the same from Rust: extras/zed/.github/workflows/run_tests.yml begins `# Generated from xtask::workflows::run_tests / # Rebuild with 'cargo xtask workflows'.` Both check the generated output for drift in CI, so hand edits fail the build. The payoff is loops, constants, and type checking for pipeline logic; the cost is a second toolchain contributors must learn before touching CI.

### Change detection as a first-class job

The largest repositories do not rely on GitHub's `paths:` filters alone, because a filtered-out workflow cannot satisfy a required check. Instead they run a cheap job that inspects the diff and gates everything else:

- uv's extras/uv/.github/workflows/plan.yml exposes 17 named outputs (`test-code`, `review-security`, `check-schema`, `build-release-binaries`, and so on) that every downstream job in ci.yml consumes via `needs.plan.outputs.*`.
- ruff's `determine_changes` job in extras/ruff/.github/workflows/ci.yaml feeds conditions like `if: ${{ needs.determine_changes.outputs.code == 'true' || github.ref == 'refs/heads/main' }}` (line 311).
- deno's `pre_build` job computes a docs-only fast path and deno_core change detection before the 39-job fan-out (extras/deno/.github/workflows/ci.generated.yml).
- zed's `orchestrate` job computes a nextest `rdeps()` filterset from changed packages, so tests run only for crates that transitively depend on the diff (extras/zed/.github/workflows/run_tests.yml, conditions on `needs.orchestrate.outputs.run_tests`).

Because the gate job always runs, branch protection can require it (or an aggregator over it) without the "skipped counts as passed" trap.

### Job matrices and OS coverage

Coverage philosophy splits by what the project ships:

- **CLI tools ship binaries for everything, so CI builds everything.** ripgrep's test matrix in extras/ripgrep/.github/workflows/ci.yml has 18 entries: pinned MSRV 1.96.0, stable, beta, nightly, musl, i686, aarch64, three armv7 variants, powerpc64, s390x, riscv64gc, macOS, and three Windows toolchains, with foreign architectures running the full test suite under qemu via a version-pinned `cross` binary. fd's release matrix in extras/fd/.github/workflows/CICD.yml lists 14 targets from `arm-unknown-linux-gnueabihf` on ubuntu-24.04 with cross to `aarch64-pc-windows-msvc` on windows-11-arm.
- **Libraries test the compiler and platform lattice.** tokio's extras/tokio/.github/workflows/ci.yml (1420 lines, 45 jobs) spans Linux, Windows, macOS, native ARM runners, FreeBSD VMs, illumos, wasm, and qemu cross-tests, all gated behind a cheap `basics` job. clap crosses OS with feature bundles (minimal, default, next) in extras/clap/.github/workflows/ci.yml.
- **Toolchain rows are part of the matrix.** gitui runs 3 OSes times nightly/stable/MSRV with `continue-on-error: ${{ matrix.rust == 'nightly' }}` (extras/gitui/.github/workflows/ci.yml, lines 18-22), so nightly breakage is visible but not blocking. nushell runs a daily beta-toolchain cron with the same tolerance (extras/nushell/.github/workflows/beta-test.yml).
- **Expensive platforms move off the PR path.** meilisearch's `test-macos` job in extras/meilisearch/.github/workflows/test-suite.yml runs only `if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'` (line 78), keeping macOS minutes out of every PR while still exercising the platform daily.

### Caching: three camps with real reasoning

**Camp 1: Swatinem/rust-cache with write gating (12 of 18).** The action is the default choice (rustdesk, tauri, uv, starship, meilisearch, ruff, helix, nushell, tokio, gitui, clap, and ruff again for wasm). The refinement that separates mature setups is restricting who writes the cache. ruff saves only from main (extras/ruff/.github/workflows/ci.yaml, line 319):

```yaml
- uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2
  with:
    save-if: ${{ github.ref == 'refs/heads/main' }}
```

tauri saves only from the one matrix leg whose artifacts other legs can reuse: `save-if: ${{ matrix.features.key == 'all' }}` with `key: ${{ matrix.platform.target }}` (extras/tauri/.github/workflows/test-core.yml, lines 93-96). gitui partitions by OS and toolchain with `shared-key: ${{ matrix.os }}-${{ env.cache-name }}-${{ matrix.rust }}` (extras/gitui/.github/workflows/ci.yml, line 32). bevy takes gating to its logical end: PRs use read-only `actions/cache/restore` (extras/bevy/.github/workflows/ci.yml, line 38, with the comment `# key won't match, will rely on restore-keys`), while a dedicated writer workflow, extras/bevy/.github/workflows/update-caches.yml, rebuilds caches on pushes to main and a nightly cron.

**Camp 2: custom caching.** deno builds its own on raw `actions/cache` (136 uses in extras/deno/.github/workflows/) with a bumpable `const cacheVersion = 123;` prefixed into every key (extras/deno/.github/workflows/ci.ts, line 21) and an explicit policy comment: "We force saving a new cache on every main run so that PRs can always be up to date with the freshest information." zed skips artifact caching in favor of a compiler cache, running sccache against a Cloudflare R2 bucket (`SCCACHE_BUCKET: sccache-zed` in extras/zed/.github/workflows/run_tests.yml, line 213). Second-layer caches also appear: helix caches built tree-sitter grammars keyed on `hashFiles('languages.toml')` with a manual bust version (extras/helix/.github/actions/rust-setup/action.yml), and rustdesk layers vcpkg binary caching over rust-cache in extras/rustdesk/.github/workflows/flutter-build.yml.

**Camp 3: deliberately no cache.** ripgrep, fd, bat, and alacritty cache nothing. Every build is from scratch and `--locked` against the committed lockfile (extras/bat/.github/workflows/CICD.yml, extras/fd/.github/workflows/CICD.yml). The reasoning: for a single-crate CLI the clean build is minutes, and a cold-start build is exactly what a packager or contributor experiences, so caching only hides breakage and adds a poisoning surface. This camp correlates strongly with the "small, stable, few dependencies" end of the spectrum.

### Action pinning by SHA

Ten of eighteen pin third-party actions to full 40-character commit SHAs with a human-readable version comment, the form seen in extras/uv/.github/workflows/ci.yml:

```yaml
- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
```

rustdesk (155 pinned uses), deno (385, emitted by the generator), uv (471), zed (393), starship, meilisearch, ruff, bevy, and nushell are all-in. The comment is not decoration: Renovate's `helpers:pinGitHubActionDigests` (zed) and dependabot's github-actions ecosystem (meilisearch, monthly with a 7-day cooldown) parse it to keep the SHA fresh.

The tag camp (tokio at 127 tag-pinned uses, clap, gitui, alacritty) rides major tags like `actions/checkout@v4`, accepting the risk on the grounds that they use few actions, mostly first-party ones. Two hybrid positions are worth noting: tauri pins by SHA only in workflows that hold elevated tokens while routine test workflows use tags, and ripgrep and bat pin exactly the release-critical actions by SHA (`actions/attest-build-provenance@977bb373... # v3.0.0` in extras/ripgrep/.github/workflows/release.yml, the winget publisher in bat) while everything else rides tags. That hybrid is a defensible minimum: the blast radius of a hijacked action is proportional to the token it can reach.

The hardened camp also lints the CI itself: uv runs zizmor as a reusable workflow uploading SARIF (extras/uv/.github/workflows/check-zizmor.yml), ruff runs zizmor and actionlint in extras/ruff/.github/workflows/ci.yaml, zed adds harden-runner egress auditing, and bevy scans workflows with CodeQL (extras/bevy/.github/workflows/security-static-analysis.yml).

### Merge queues and required checks

Five projects have adopted GitHub's merge queue via the `merge_group` trigger: zed (extras/zed/.github/workflows/run_tests.yml, line 9), bevy (extras/bevy/.github/workflows/ci.yml, line 7), helix (extras/helix/.github/workflows/build.yml), meilisearch (extras/meilisearch/.github/workflows/test-suite.yml, line 9), and zed's danger workflow. The sophistication is in what runs inside the queue: since the queue re-tests the merged result, redundant or slow legs are skipped there. meilisearch drops Windows from queue runs (`if: github.event_name != 'merge_group'` on `test-windows`, line 57 of test-suite.yml), and zed skips whole test jobs in the queue with `github.event_name != 'merge_group'` conditions (run_tests.yml, lines 194-374). The other thirteen projects rely on plain branch protection, reasoning that their merge rate does not yet produce the stale-green-check problem queues solve.

For required checks themselves, the standout consensus among large projects is the **single aggregation gate**: one job that `needs:` everything and is the only check branch protection requires, so adding a CI job never requires touching repository settings. Six repositories implement it. clap's is the tersest (extras/clap/.github/workflows/ci.yml, lines 22-32):

```yaml
  ci:
    permissions:
      contents: none
    name: CI
    needs: [test, shell-integration, shell-integration-nu, check, ui, minimal-versions, lockfile, docs, rustfmt, clippy, cffconvert]
    runs-on: ubuntu-latest
    if: "always()"
    steps:
      - name: Failed
        run: exit 1
        if: "contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') || contains(needs.*.result, 'skipped')"
```

bat compresses the check into jq: `jq --exit-status 'all(.result == "success")' <<< '${{ toJson(needs) }}'` (extras/bat/.github/workflows/CICD.yml, line 32) and pairs it with a meta-test, tests/github-actions.rs, that parses the workflow YAML to keep the `needs:` list complete. uv's `required-checks-passed` (extras/uv/.github/workflows/ci.yml, line 294), zed's `tests_pass` (run_tests.yml, line 902), and deno's `ci-status` (ci.generated.yml, line 7588) are the same idea at scale. Note the details that make it correct: `if: always()` so the gate runs even when a dependency fails, and treating `skipped` and `cancelled` as failures so a path filter cannot silently green the build. tokio inverts the topology with the same goal: a cheap `basics` job (fmt, clippy, docs, minrust) must pass before 40+ expensive jobs start (`needs: basics` throughout extras/tokio/.github/workflows/ci.yml, lines 45-57), saving compute on obviously broken PRs.

### Scheduled jobs

Crons carry four distinct workloads across the set:

1. **Advisory audits decoupled from commits.** A new RUSTSEC advisory should surface between merges, not only when someone touches Cargo.toml. tokio runs cargo-deny on both manifest-path pushes and a daily 2 AM cron (extras/tokio/.github/workflows/audit.yml), tauri audits daily, and starship softens the failure mode with `continue-on-error: ${{ matrix.checks == 'advisories' }}` so "the sudden announcement of a new advisory" cannot redden unrelated PRs (extras/starship/.github/workflows/security-audit.yml, lines 20-21).
2. **Nightly builds and toolchain canaries.** gitui rebuilds all platforms nightly and uploads to S3 (extras/gitui/.github/workflows/nightly.yml), nushell publishes tagged nightlies from a synced repo (extras/nushell/.github/workflows/nightly-build.yml), and clap moves beta/nightly and latest-deps testing entirely off the PR path into a monthly cron (extras/clap/.github/workflows/rust-next.yml, `cron: '3 3 3 * *'`).
3. **Flake and fuzz hunting.** meilisearch runs cargo-flaky at 100 iterations every day at 4 AM (extras/meilisearch/.github/workflows/flaky-tests.yml) and a stateful indexing fuzzer (fuzzer-indexing.yml); ruff fuzzes daily (extras/ruff/.github/workflows/daily_fuzz.yaml); deno runs Node.js's own compatibility suite on a weekday cron (extras/deno/.github/workflows/node_compat_test.generated.yml).
4. **Scheduled maintenance that opens PRs.** rustdesk's extras/rustdesk/.github/workflows/update-webpki-roots.yml opens a reviewable cargo-update PR on a schedule under a non-cancelling group (`group: update-webpki-roots`, `cancel-in-progress: false`), and bevy's update-caches.yml refreshes CI caches nightly so PR restores stay warm.

### Release pipelines, signing, and provenance

The release consensus has four load-bearing parts.

**Verify the tag against the manifest before building anything.** ripgrep inlines it (extras/ripgrep/.github/workflows/release.yml, lines 30-40):

```yaml
- name: Check that tag version and Cargo.toml version are the same
  shell: bash
  run: |
    if ! grep -q "version = \"$VERSION\"" Cargo.toml; then
      echo "version does not match Cargo.toml" >&2
      exit 1
    fi
- name: Create GitHub release
  run: gh release create $VERSION --draft --verify-tag --title $VERSION
```

meilisearch scripts it as extras/meilisearch/.github/scripts/check-release.sh, which validates the tag format and matches it against both Cargo.toml and Cargo.lock, and every publish job gates on it.

**Keep releases draft until every artifact exists.** ripgrep creates the release `--draft`, starship keeps releases draft until all 13 target artifacts and checksums upload, and alacritty uploads each platform's artifact to a draft via a small script (extras/alacritty/.github/workflows/upload_asset.sh, called from extras/alacritty/.github/workflows/release.yml) so a human publishes only after every OS job finishes.

**Sign or attest what you ship.** `actions/attest-build-provenance` appears in ripgrep (release.yml, line 288, SHA-pinned), fd (extras/fd/.github/workflows/CICD.yml, gated on `refs/tags/v[0-9]`), and helix, which adds a preview mode so the release pipeline is testable from PRs: `uses: actions/attest-build-provenance@v4` with `if: env.preview == 'false'` (extras/helix/.github/workflows/release.yml, lines 267-271). starship publishes to crates.io with OIDC trusted publishing instead of a stored token (`permissions: id-token: write` plus `rust-lang/crates-io-auth-action` in extras/starship/.github/workflows/release.yml, lines 331-342). rustdesk attaches a Syft-generated CycloneDX SBOM to every release (`syft dir:. -o cyclonedx-json=rustdesk.sbom.json` in extras/rustdesk/.github/workflows/flutter-build.yml, lines 56-73). meilisearch signs its multi-arch Docker publishing path with OIDC (extras/meilisearch/.github/workflows/publish-docker-images.yml).

**Or outsource the whole pipeline.** uv and ruff hand release engineering to cargo-dist: extras/ruff/.github/workflows/release.yml opens with `# This file was autogenerated by dist: https://axodotdev.github.io/cargo-dist` and dist-workspace.toml (extras/ruff/dist-workspace.toml, `cargo-dist-version = "0.31.0"`) declares the 18-target matrix, installers, and attestations declaratively. ruff adds a dispatch-triggered release with a two-person approval environment on top.

### Comparison table

| Repo | Action pinning | Cargo caching | Merge queue | Aggregator gate | Scheduled jobs | Release signing / provenance |
|---|---|---|---|---|---|---|
| rustdesk | SHA + comment (155) | rust-cache + vcpkg + clear-cache workflow | no | no | nightly build, scheduled dep-update PR | Syft SBOM, secret-gated code signing |
| tauri | tags, SHA for elevated tokens | rust-cache, save-if on all-features leg | no | no | daily audit + cargo-vet | covector publish behind 3-OS suite |
| deno | SHA (generated, 385) | custom actions/cache, cacheVersion const, save on main | no | ci-status | weekday Node compat, daily crons | OIDC (id-token) publish jobs |
| uv | SHA (471), permissions {} | rust-cache, save-if gating | no | required-checks-passed | daily cron | cargo-dist, 18 targets, attestations |
| zed | SHA (393), harden-runner | sccache to R2 | yes, heavy jobs skipped | tests_pass | nightly builds every few hours | release on v* tags, drafted notes |
| ripgrep | tags, SHA for attest | none, deliberate | no | no | nightly cron CI | tag==version check, attest-build-provenance |
| alacritty | tags (checkout only) | none | no | no | none (sourcehut nightly fmt) | draft release via upload_asset.sh |
| bat | tags, SHA for winget | none, --locked | no | all-jobs (jq) | none | ref-gated release matrix in CICD.yml |
| starship | SHA (56) | rust-cache | no | no | daily security audit | release-please, OIDC crates.io, SignPath |
| meilisearch | SHA (108) | rust-cache by feature matrix | yes, Windows skipped | no | daily flaky hunt, fuzzer, macOS suite | check-release.sh gate, OIDC Docker |
| ruff | SHA (259), zizmor | rust-cache, save-if main | no | via determine_changes gating | daily fuzz, scheduled reports | cargo-dist, approval-gated, attestations |
| bevy | SHA (128), CodeQL | restore-only in PRs, writer workflow | yes | no | nightly cache refresh, daily cron | docs deploy only, crates via cargo-release |
| helix | mixed (10 SHA, 15 tag) | rust-cache via composite + grammar cache | yes | no | nightly cron | attest-build-provenance, preview mode |
| fd | mixed, persist-credentials false | none, --locked | no | no | none | attest gated on version tags, 14 targets |
| nushell | SHA (40) | rust-cache (single use) | no | no | nightly build, daily beta test, audit | SHA256SUMS, winget, nushell release scripts |
| tokio | tags (127) | rust-cache (49 uses) | no | basics gate (inverse) | daily cargo-deny | crates.io only (library) |
| gitui | tags (25) | rust-cache by os+toolchain | no | no | two nightly crons to S3 | cd.yml on tags, contents: write only |
| clap | tags (37) | rust-cache (12) | no | ci (needs + always()) | monthly rust-next + audit | cargo-release replacements + tag workflow |

### Exemplary excerpts

**One reusable build, three trigger shells** (extras/rustdesk/.github/workflows/flutter-build.yml):

```yaml
name: Build the flutter version of the RustDesk

on:
  workflow_call:
    inputs:
      upload-artifact:
        type: boolean
        default: true
```

**A gate job before the expensive fan-out** (extras/tokio/.github/workflows/ci.yml, lines 44-57):

```yaml
  # Basic actions that must pass before we kick off more expensive tests.
  basics:
    name: basic checks
    runs-on: ubuntu-latest
    needs:
      - clippy
      - fmt
      - docs
      - minrust
```

**Read-only caches for pull requests** (extras/bevy/.github/workflows/ci.yml, lines 38-44):

```yaml
- uses: actions/cache/restore@27d5ce7f107fe9357f9df03efb73ab90386fccae # v5.0.5
  with:
    # key won't match, will rely on restore-keys
    key: ${{ runner.os }}-stable--${{ hashFiles('**/Cargo.toml') }}-
    # See .github/workflows/update-caches.yml for how keys are generated
    restore-keys: |
      ${{ runner.os }}-stable--${{ hashFiles('**/Cargo.toml') }}-
```

**Advisory noise kept out of PR status** (extras/starship/.github/workflows/security-audit.yml, lines 20-21):

```yaml
    # Prevent sudden announcement of a new advisory from failing ci:
    continue-on-error: ${{ matrix.checks == 'advisories' }}
```

**A composite action as the shared CI entry point** (extras/helix/.github/actions/rust-setup/action.yml):

```yaml
name: Rust setup
description: Install a Rust toolchain and warm the cargo + tree-sitter grammar caches.
runs:
  using: composite
  steps:
    - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # master
    - uses: Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4 # v2.9.1
      with:
        shared-key: ${{ inputs.cache-key }}
```

### What a new Rust project should do

- [ ] Trigger CI on `pull_request`, push to the default branch, and `workflow_dispatch`; add `paths-ignore` for docs the way extras/rustdesk/.github/workflows/flutter-ci.yml does.
- [ ] Add a workflow-level `concurrency:` group keyed on workflow plus ref, with `cancel-in-progress` conditional on `github.event_name == 'pull_request'` (extras/helix/.github/workflows/build.yml).
- [ ] Set top-level `permissions: contents: read` (or `permissions: {}` with per-job escalation) and `persist-credentials: false` on every checkout, following extras/uv/.github/workflows/ci.yml and extras/ripgrep/.github/workflows/ci.yml.
- [ ] Pin every third-party action to a full commit SHA with a `# vX.Y.Z` comment, and let Renovate or a github-actions dependabot ecosystem refresh the pins; at minimum pin anything reachable from an elevated token, as ripgrep and tauri do.
- [ ] Lint the CI itself: zizmor and actionlint jobs modeled on extras/uv/.github/workflows/check-zizmor.yml and extras/ruff/.github/workflows/ci.yaml.
- [ ] Build a matrix over ubuntu, windows, and macos plus a pinned-MSRV row read from `Cargo.toml` via `cargo metadata`, with `fail-fast: false` (extras/fd/.github/workflows/CICD.yml, extras/gitui/.github/workflows/ci.yml). Add beta or nightly rows only with `continue-on-error`.
- [ ] Use Swatinem/rust-cache with `save-if` restricted to the default branch (extras/ruff/.github/workflows/ci.yaml, line 319) and keys partitioned by OS, toolchain, and feature set; skip caching entirely only if a cold build is under a few minutes and you value reproducibility more.
- [ ] Create a single always-running aggregation job (`needs` everything, `if: always()`, fail on `failure`, `cancelled`, or unexpected `skipped`) and make it the only required check, copying extras/clap/.github/workflows/ci.yml or extras/bat/.github/workflows/CICD.yml.
- [ ] If PR throughput warrants a merge queue, add `merge_group:` to the trigger list and skip redundant heavy legs inside the queue, as extras/meilisearch/.github/workflows/test-suite.yml does for Windows.
- [ ] Add scheduled jobs for the work that should not wait for a commit: a daily cargo-deny or rustsec audit (extras/tokio/.github/workflows/audit.yml), a nightly or monthly beta-toolchain canary (extras/clap/.github/workflows/rust-next.yml), and flake or fuzz hunting once the suite is large (extras/meilisearch/.github/workflows/flaky-tests.yml).
- [ ] Trigger releases only on `v*` tags, verify tag equals manifest version before building (extras/ripgrep/.github/workflows/release.yml, extras/meilisearch/.github/scripts/check-release.sh), and keep the release draft until all artifacts and checksums are attached.
- [ ] Attest release artifacts with `actions/attest-build-provenance` gated on version tags (extras/fd/.github/workflows/CICD.yml, extras/helix/.github/workflows/release.yml), publish to registries with OIDC trusted publishing instead of stored tokens (extras/starship/.github/workflows/release.yml), and consider attaching an SBOM (extras/rustdesk/.github/workflows/flutter-build.yml).
- [ ] Give the release workflow a preview mode runnable from a PR, as extras/helix/.github/workflows/release.yml does, so the pipeline is tested before the tag exists.
- [ ] When the pipeline outgrows one file, split trigger shells from a `workflow_call` core (extras/rustdesk/.github/workflows/flutter-ci.yml); when it outgrows hand-written YAML, generate it and drift-check the output (extras/deno/.github/workflows/ci.ts, extras/zed/.github/workflows/run_tests.yml).

---

## Project and Workspace Structure

How a repository is carved into crates, modules, and support directories is the first
architectural decision a Rust project makes, and it is the one that every later decision
(CI sharding, publishing, test layout, compile times) has to live with. This chapter
synthesizes the structural choices of eighteen mature repositories: rustdesk, tauri,
deno, uv, zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd,
nushell, tokio, gitui, and clap.

### 22.1 Consensus: what nearly every project does

Across all eighteen projects, a few practices are close to universal.

1. **A workspace appears the moment there is a second crate, and never later.**
   Fifteen of the eighteen repositories are Cargo workspaces. The three that are not
   (bat, starship, fd) are single-crate applications with exactly one `Cargo.toml`.
   Nobody runs sibling crates without a workspace, and nobody nests independent
   checkouts: the workspace is the unit of `Cargo.lock`, lint policy, and CI.

2. **Shared metadata is inherited, not repeated.** Workspaces use `[workspace.package]`
   for edition, license, rust-version, and repository, and `[workspace.dependencies]`
   for version pins. Even ripgrep, which is otherwise minimalist, inherits editions
   (extras/ripgrep/Cargo.toml):

   ```toml
   [workspace.package]
   edition = "2024"
   rust-version = "1.96"
   ```

   uv centralizes all 70+ internal crates as versioned path entries
   (extras/uv/Cargo.toml): `uv-cache = { version = "0.0.72", path = "crates/uv-cache" }`.

3. **Crate directory name equals crate name, with a project prefix.** `uv-*` under
   extras/uv/crates, `nu-*` under extras/nushell/crates, `helix-*` at the root of
   extras/helix, `bevy_*` under extras/bevy/crates, `tauri-*` under extras/tauri/crates.
   Nobody renames a directory away from its crate.

4. **Split boundaries follow capability, not size.** The reusable engine is a library
   crate and the product shell is a thin consumer: `alacritty_terminal` vs the
   `alacritty` GUI crate (extras/alacritty/Cargo.toml), `asyncgit` vs the gitui TUI
   (extras/gitui/Cargo.toml), the ten `grep`/`ignore`/`globset` crates under
   extras/ripgrep/crates vs the `rg` binary, `helix-core` vs `helix-term`.

5. **Special-needs packages are excluded from the workspace rather than contorted to
   fit.** Fuzz targets, nightly-only launchers, and consumer-simulation crates carry
   their own `[workspace]` table so `cargo test` at the root never touches them.
   ripgrep is explicit about it (extras/ripgrep/fuzz/Cargo.toml):

   ```toml
   # Prevent this from interfering with workspaces
   [workspace]
   members = ["."]
   ```

   uv excludes its Windows launcher with a one-line reason (extras/uv/Cargo.toml):

   ```toml
   members = ["crates/*"]
   exclude = [
     "scripts",
     # Needs nightly
     "crates/uv-trampoline",
   ]
   ```

   nushell nests standalone fuzz workspaces inside member crates
   (extras/nushell/crates/nu-parser/fuzz/Cargo.toml declares its own `[workspace]`),
   and bevy keeps a workspace-excluded consumer crate so integration tests use bevy
   exactly as a downstream user would.

6. **Test-only and dev-only crates are first-class members marked `publish = false`.**
   gitui ships `git2-testing` and `invalidstring` purely for tests
   (extras/gitui/git2-testing/Cargo.toml), tauri keeps integration fixtures in
   `crates/tests/restart` and `crates/tests/acl` (extras/tauri/Cargo.toml), tokio
   carries `tests-build` and `tests-integration` as members.

### 22.2 Single crate or workspace: the three camps

**Camp 1: single crate, no workspace.** bat, starship, and fd. All three are
end-user CLI tools whose internals nobody else consumes as separate pieces. fd is the
purest case: one manifest, no `lib.rs`, sixteen files plus three subsystem directories
(extras/fd/src: `exec/`, `filter/`, `fmt/`). starship scales the same shape to a much
larger program by leaning on module directories (extras/starship/src/modules holds one
file per prompt module) instead of crates. The reasoning: a crate boundary buys
compile-time parallelism and an enforced API, but costs version bookkeeping and
cross-crate refactoring friction. If no external consumer exists and the build is
fast enough, the boundary is pure cost.

**Camp 2: a root package that is also the workspace root.** rustdesk, ripgrep, bevy,
nushell, gitui, and clap keep `[package]` and `[workspace]` in the same root manifest.
The product lives at the top; support crates hang off it. ripgrep even keeps the
binary's source in a crate directory without its own manifest
(extras/ripgrep/Cargo.toml):

```toml
[[bin]]
bench = false
path = "crates/core/main.rs"
name = "rg"
```

The reasoning: `cargo run`, `cargo install --path .`, and `cargo test` at the repo
root operate on the product with zero flags, and the repository name stays the crate
name. bevy inverts the direction: the root package is a facade that re-exports
`bevy_internal`, so users depend on one crate while the engine is 90+ manifests
underneath (extras/bevy/Cargo.toml wires every feature through to `bevy_internal/...`).
clap does the same at library scale, with the facade exact-pinning its internals
(extras/clap/Cargo.toml): `clap_builder = { path = "./clap_builder", version = "=4.6.6", ... }`.

**Camp 3: a virtual workspace with no root package.** tauri, deno, uv, zed,
alacritty, meilisearch, ruff, helix, and tokio. The root manifest is coordination
only. The reasoning: with many peers there is no natural "main" crate to privilege,
and a root package muddies `cargo <cmd>` defaults and publishing. The ergonomics of
Camp 2 are recovered with `default-members`. helix (extras/helix/Cargo.toml):

```toml
default-members = [
  "helix-term"
]
```

zed does the same at 243 member crates: `default-members = ["crates/zed"]`
(extras/zed/Cargo.toml). The rule of thumb that falls out: application with one
obvious product, keep a root package; platform or library family, go virtual and
set `default-members` to the binary.

### 22.3 Where crates live, and policing the root

The dominant home is a `crates/` directory (tauri, uv, zed, meilisearch, ruff, bevy,
nushell, tokio's cousins aside). Smaller workspaces skip the extra level and put crate
directories at the root: helix (`helix-core/`, `helix-term/`, ...), tokio (`tokio/`,
`tokio-util/`, ...), alacritty, gitui, and clap. Two projects encode meaning into the
directory level itself:

```text
extras/rustdesk/                    extras/deno/
+-- src/          (orchestration)   +-- cli/       (the deno binary)
+-- libs/                           +-- runtime/   (JS runtime layer)
|   +-- scrap/    (capture)         +-- ext/       (op extensions)
|   +-- hbb_common/                 +-- libs/      (pure Rust libraries)
|   +-- enigo/    (input)           +-- tools/     (lint and CI scripts)
|   \-- clipboard/                  \-- tests/
\-- res/          (packaging)
```

rustdesk's capability crates sit under `libs/` with the thin product on top
(extras/rustdesk/Cargo.toml lists `members = ["libs/scrap", "libs/hbb_common", ...]`
and `exclude = ["vdi/host"]`). deno goes furthest: the four-layer split
(`cli > runtime > ext > libs`) is enforced by a lint that fails CI when anything new
appears at the root (extras/deno/tools/lint.js):

```js
// WARNING: When adding anything to this list it must be discussed!
// Keep the root of the repository clean.
const allowed = new Set([
  ".cargo", ".claude", ".devcontainer", ".github",
  "x", "cli", "doc", "ext", "libs", "runtime", "tests", "tools",
  ...
```

That is the strongest statement in the corpus that structure is an invariant to be
tested, not a convention to be remembered. bevy states the same intent as a comment
(extras/bevy/Cargo.toml): `# All of Bevy's official crates are within the "crates"
folder!` with globbed `members = ["crates/*", ...]`, then adds explicit entries for
compile-fail crates and mobile examples that need to be members individually.

### 22.4 Module tree conventions: mod.rs, named files, and crate-named roots

Counting directories under `src/` that have a sibling `<name>.rs` (named-file style)
versus `mod.rs` files gives a clear picture. The corpus has three positions.

**The mod.rs majority.** deno (74 mod.rs, 0 named), bevy (153/0), nushell (219/0),
alacritty (13/0), ripgrep (7/0), gitui (13/0), clap (17/0), fd (4/0), starship (8/0),
meilisearch (105/4), uv (23/4), tauri (50/9). The `mod.rs` file doubles as the
platform dispatch point, as in alacritty's PTY layer
(extras/alacritty/alacritty_terminal/src/tty/mod.rs):

```rust
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use self::unix::*;

#[cfg(windows)]
pub mod windows;
```

**The named-file minority.** zed (118 named-file directories, 6 mod.rs) and helix
(15 named, 4 mod.rs) put the module's code in `foo.rs` beside a `foo/` directory of
submodules, so an editor tab strip never shows five files all called `mod.rs`. zed
extends the idea to the crate root itself: the lib target is named after the crate
instead of `lib.rs` (extras/zed/crates/editor/Cargo.toml):

```toml
[lib]
path = "src/editor.rs"
doctest = false
```

**The mixed middle.** ruff (153 mod.rs, 56 named), tokio (54/21), rustdesk (16/9),
and bat (2/3) carry both, usually because the convention changed midway. The lesson
from the mixed camp is negative: neither style wins on merit, but a repo that
contains both forces every reader to check which one applies. Pick one and state it.

### 22.5 Bin vs lib splits

Four shapes exist, in increasing order of separation:

1. **Binary only.** fd has `src/main.rs` and no library target at all
   (extras/fd/src). Integration tests run the compiled binary via
   `CARGO_BIN_EXE_fd`, so no library surface is needed.
2. **lib.rs plus main.rs in one crate.** starship keeps `src/lib.rs` and
   `src/main.rs` side by side (extras/starship/src) so integration code and the
   binary share one manifest.
3. **lib.rs plus a src/bin/ directory.** bat separates the reusable printing library
   from the application, giving the binary its own module tree
   (extras/bat/src/bin/bat holds `main.rs`, `app.rs`, `clap_app.rs`,
   `completions.rs`, ...). The library deliberately does not know about clap.
4. **Separate crates.** alacritty (GUI binary vs `alacritty_terminal`), gitui
   (TUI vs `asyncgit`), ripgrep (`rg` over ten library crates), helix
   (`helix-term` over `helix-core`, `helix-view`, `helix-tui`).

The graduation trigger is consumption: bat's library is used by other tools, and
alacritty's headless terminal library exists so golden ref tests can replay PTY
recordings without a window. When the split exists only to make the CLI parser
testable, shape 2 or 3 is enough; shape 4 pays off once a second consumer (tests,
another binary, external users) is real.

### 22.6 xtask and dev-tool crates

Repo-specific automation written in Rust, runnable through cargo, appears in half the
corpus, in three flavors:

- **A literal `xtask` crate plus a cargo alias.** helix
  (extras/helix/xtask/src has `docgen.rs`, `themelint`-style checks) wired through
  extras/helix/.cargo/config.toml:

  ```toml
  xtask = "run --package xtask --"
  ```

  meilisearch does the same with a release-profile twist
  (extras/meilisearch/.cargo/config.toml):
  `xtask = "run --release --package xtask --"`, with the crate at
  extras/meilisearch/crates/xtask driving benchmarks and declarative upgrade tests.
- **A tooling directory of purpose-named crates.** zed keeps `xtask`, custom dylint
  `lints`, `perf`, and `compliance` under extras/zed/tooling, so dev tools do not mix
  with product crates. bevy encodes its entire CI command set as a reviewable crate
  named `ci` (extras/bevy/tools/ci/Cargo.toml, `name = "ci"`), next to
  `build-templated-pages` and `example-showcase` under extras/bevy/tools.
- **Dev crates inside the normal crate tree.** ruff has `ruff_dev` behind a
  `cargo dev` alias and `ruff_benchmark` (extras/ruff/crates), uv has `uv-dev` and
  `uv-bench` (extras/uv/crates).

The alternative camp scripts everything outside cargo: deno's `./x` CLI and
extras/deno/tools JS scripts, rustdesk's `build.py`, nushell's `toolkit.nu`,
ripgrep's `ci/` shell scripts. The pattern within the pattern: projects whose tooling
must understand the workspace (codegen, docgen, invariant lints) put it in a crate;
projects whose tooling is packaging glue keep scripts.

### 22.7 Where large files get split, and where they deliberately do not

The corpus tolerates very large files when the file is a registry with one entry
shape repeated: ripgrep's flag table is 8,161 lines
(extras/ripgrep/crates/core/flags/defs.rs), one unit struct per flag with its unit
tests directly beneath each impl. Splitting it would scatter a table. The
surrounding directory shows the split that did happen: `flags/` holds `defs.rs`,
`parse.rs`, `hiargs.rs`, `lowargs.rs`, `complete/`, and `doc/`
(extras/ripgrep/crates/core/flags), separating flag definitions from parsing and
from documentation rendering.

helix takes the satellite-directory approach: `commands.rs` stays a 7,228-line
command table while cohesive subsystems break out into
extras/helix/helix-term/src/commands as `typed.rs`, `lsp.rs`, `dap.rs`, and
`syntax.rs`. zed accepts a 12,624-line `editor.rs`
(extras/zed/crates/editor/src/editor.rs) as the hub of a crate that splits everything
else into 100+ sibling files. The shared rule: split along subsystem seams
(fd's `exec/`, `filter/`, `fmt/` under extras/fd/src), never by line count alone,
and let a homogeneous table stay whole.

### 22.8 Examples and benches placement

Three placements for examples:

- **Root `examples/` as product documentation.** bevy registers 430 `[[example]]`
  targets in extras/bevy/Cargo.toml and gates them in CI; clap's root examples are
  executed by trycmd transcripts so they are docs that test.
- **A dedicated examples crate.** tokio makes `examples` a `publish = false`
  workspace member (extras/tokio/examples/Cargo.toml) with a comment telling copiers
  to swap the path dependency for a version.
- **Per-crate examples as API docs.** ripgrep keeps `simplegrep`-style examples in
  the library crates that own the API (extras/ripgrep/crates/grep/examples,
  extras/ripgrep/crates/searcher/examples, extras/ripgrep/crates/ignore/examples).

Benches mirror the ownership rule: a dedicated bench crate when benchmarks span the
workspace (`uv-bench`, `ruff_benchmark`, `clap_bench`, tokio's `benches` member,
listed under `# Internal` in extras/tokio/Cargo.toml), per-crate `benches/`
directories when the hot code is local (extras/meilisearch/crates/flatten-serde-json/benches,
extras/helix/helix-view/benches), and a root `[[bench]]` with `harness = false` for
single-crate-rooted workspaces (extras/nushell/Cargo.toml, `name = "benchmarks"`).
Two projects place benchmarks outside the repo entirely (fd's hyperfine suite,
alacritty's vtebench), keeping the workspace free of criterion dependencies.

### 22.9 Comparison table

Module style counts are measured as `mod.rs` files vs directories with a sibling
`<name>.rs` under any `src/` tree.

| Repository | Layout | Manifests | Crate homes | Module style (mod.rs/named) | Bin vs lib | Dev-tool crate | Examples and benches |
|---|---|---|---|---|---|---|---|
| rustdesk | root package + workspace | 9 | libs/ capability crates | mixed (16/9) | root bin over libs/ | none (build.py) | examples inside libs/scrap |
| tauri | virtual workspace | 26 | crates/, packages/ | mostly mod.rs (50/9) | tauri-cli bin, tauri lib | crates/tests/*, bench member | bench/ member crates |
| deno | virtual workspace | 87 | layered cli/ ext/ libs/ runtime/ | mod.rs (74/0) | cli bin over ext and libs | tools/ scripts + ./x | benches in leaf crates |
| uv | virtual workspace | 73 | crates/* glob | mostly mod.rs (23/4) | uv bin over uv-* libs | uv-dev, uv-bench | uv-bench crate |
| zed | virtual workspace | 259 | crates/ (243 members) | named-file (6/118), crate-named roots | zed bin, default-members | tooling/ (xtask, lints, perf) | benches in crates |
| ripgrep | root package + workspace | 12 | crates/, core has no manifest | mod.rs (7/0) | rg bin over 10 lib crates | ci/ scripts, excluded fuzz/ | per-lib-crate examples |
| alacritty | virtual workspace | 5 | top-level crate dirs | mod.rs (13/0) | GUI bin vs alacritty_terminal lib | none (Makefile) | external vtebench |
| bat | single crate | 1 | src/ + src/bin/bat/ | mixed (2/3) | lib.rs + src/bin/bat/ | build/ script dir | root examples/, hyperfine scripts |
| starship | single crate | 1 | src/ module dirs | mod.rs (8/0) | lib.rs + main.rs | none | timings subcommand, no benches |
| meilisearch | virtual workspace | 28 | crates/ | mod.rs (105/4) | meilisearch bin over milli | crates/xtask + alias | per-crate benches/ |
| ruff | virtual workspace | 53 | crates/ | mixed (153/56) | ruff bin over ruff_* libs | ruff_dev + cargo dev | ruff_benchmark crate, excluded fuzz/ |
| bevy | root facade + workspace | 92 | crates/* + tools/ | mod.rs (153/0) | bevy facade over bevy_internal | tools/ci crate | root examples/ (430), root benches/ |
| helix | virtual workspace | 15 | top-level helix-* dirs | named-file (4/15) | helix-term bin, default-members | xtask/ + alias | helix-view/benches |
| fd | single crate | 1 | src/ subsystem dirs | mod.rs (4/0) | binary only, no lib.rs | scripts/ | external hyperfine repo |
| nushell | root package + workspace | 46 | crates/nu-* | mod.rs (219/0) | nu bin over nu-* libs | toolkit.nu script | root [[bench]] harness=false |
| tokio | virtual workspace | 13 | top-level crate dirs | mostly mod.rs (54/21) | library family, no product bin | tests-build, tests-integration | examples and benches member crates |
| gitui | root package + workspace | 7 | top-level member dirs | mod.rs (13/0) | TUI bin vs asyncgit lib | git2-testing, invalidstring | none in repo |
| clap | root facade + workspace | 8 | top-level clap_* dirs | mod.rs (17/0) | facade over clap_builder | clap_bench member | root examples/ run by trycmd |

### 22.10 What a new Rust project should do

- [ ] Start as a single crate; do not pre-split. Add a crate only when a boundary has
      a second consumer, following bat and fd rather than a speculative workspace.
- [ ] The moment a second crate exists, create the workspace, set `resolver` explicitly,
      and move edition, rust-version, license, and shared versions into
      `[workspace.package]` and `[workspace.dependencies]`.
- [ ] For an application, keep a root package (or a virtual workspace with
      `default-members` pointing at the binary, as in extras/helix/Cargo.toml) so
      `cargo run` and `cargo install --path .` need no flags.
- [ ] Put member crates under `crates/` with directory name equal to crate name and a
      consistent project prefix; reserve extra directory levels (deno's cli/ext/libs)
      for enforced layering only.
- [ ] Pick one module style, named-file or mod.rs, write it down, and never mix.
      If large crates are expected, prefer named-file so tabs are distinguishable.
- [ ] Split bin from lib inside one crate first (`src/lib.rs` plus `src/bin/<name>/`
      like extras/bat/src/bin/bat); promote to a separate library crate only when
      headless tests or external consumers demand it, like alacritty_terminal.
- [ ] Add an `xtask` crate plus a `.cargo/config.toml` alias for docgen, codegen
      checks, and repo lints; keep pure packaging glue in scripts.
- [ ] Keep fuzz targets, nightly-only crates, and consumer-simulation tests in
      packages with their own `[workspace]` table, excluded from the main workspace,
      as in extras/ripgrep/fuzz/Cargo.toml.
- [ ] Mark every test-only and dev-only crate `publish = false` and give it a real
      manifest inside the workspace so it shares the lockfile and lint wall.
- [ ] Place examples in the crate whose API they demonstrate, and product-level
      examples at the root; give benchmarks a dedicated `publish = false` crate or
      per-crate `benches/`, never mixed into the product's dependency graph.
- [ ] Split large files along subsystem seams into a satellite directory
      (helix `commands/`), and let homogeneous registries stay as one file with
      their tests inline (ripgrep `flags/defs.rs`).
- [ ] Once the layout matters, enforce it: a CI check that fails on new top-level
      entries or misplaced crates, in the spirit of extras/deno/tools/lint.js.

---

## Testing Strategies

Testing is where the eighteen projects studied in this reference diverge the most in
mechanism and converge the most in intent. Every one of them treats the compiled
artifact, not the unit, as the thing that must be proven correct: CLI tools run their
real binaries, TUIs drive their real event loops, and libraries run their public API
from separate integration crates. This chapter covers integration test layout under
`tests/`, end-to-end CLI harnesses, snapshot testing, property testing, fuzzing,
benchmarking, test-support crates, coverage, and test runners.

### 23.1 Consensus practices

Nearly all eighteen projects share the following habits.

**Test the real artifact, not a simulation.** fd resolves the binary Cargo just built
via the `CARGO_BIN_EXE_<name>` environment variable
(extras/fd/tests/testenv/mod.rs):

```rust
/// Find the *fd* executable.
fn find_fd_exe() -> PathBuf {
    // Read the location of the fd executable from the environment
    PathBuf::from(env::var("CARGO_BIN_EXE_fd").unwrap_or(env!("CARGO_BIN_EXE_fd").to_string()))
}
```

uv does the same through its `uv-test` support crate, bat through an `assert_cmd`
command factory (extras/bat/tests/utils/command.rs), ripgrep through a
`Dir`/`TestCommand` pair (extras/ripgrep/tests/util.rs), and meilisearch by booting
its actual server entry point in a `TempDir`. Even the GUI-shaped projects follow the
principle: helix drives the real `Application` event loop with synthetic key events
(extras/helix/helix-term/tests/test/commands/write.rs), and gitui draws the real app
struct into a ratatui `TestBackend`.

**Isolate every test in a throwaway filesystem.** `tempfile::TempDir` fixtures appear
in ripgrep, fd, bat, uv, tauri, meilisearch, nushell (its `Playground` type), and
helix. Tests that touch global state are serialized rather than deleted: bat uses
`serial_test`, ruff and uv declare nextest `test-groups` with `max-threads = 1`.

**Failures must explain themselves.** ripgrep's `eqnice!` macro prints a framed
expected/got block (extras/ripgrep/tests/macros.rs); fd renders a line diff via the
`diff` crate (extras/fd/tests/testenv/mod.rs, `format_output_error`); snapshot tools
(insta, expect-test, trycmd) produce reviewable diffs by construction.

**Test-support code is a first-class deliverable.** Twelve of the eighteen ship a
dedicated support crate or module: uv's `uv-test` (extras/uv/crates/uv-test/src/lib.rs),
deno's `tests/util` SDK with a PTY driver and mock registry farm, tokio's published
`tokio-test` crate, gitui's `git2-testing` and `invalidstring` helper crates,
meilisearch's `meili-snap` (extras/meilisearch/crates/meili-snap/src/lib.rs), zed's
800+ `test-support` cargo feature references, starship's `ModuleRenderer` builder with
864 call sites (extras/starship/src/test/mod.rs), and nushell's `Playground`.

**Determinism is engineered, not hoped for.** starship ships deterministic git bundle
fixtures with a shared `TEST_GIT_CONFIG` setting `core.fsync=all` to kill Windows
flakes; gitui sandboxes global git config via `git2` `set_search_path` inside a `Once`
so the host machine cannot leak into tests; fd installs a `cfg(test)` thread-local
clock in `src/filter/time.rs`; zed's `#[gpui::test]` macro replays seeded async
schedules so every test doubles as a concurrency fuzzer.

### 23.2 Divergent camps

#### Layout: `tests/` directory versus inline `cfg(test)` modules

The single biggest split is where tests live.

**Camp A: integration tests dominate, under `tests/`.** ripgrep, fd, bat, uv, clap,
tokio, deno, meilisearch, helix. ripgrep goes furthest: `autotests = false` in
extras/ripgrep/Cargo.toml forces every test into one binary:

```text
extras/ripgrep/tests/
|-- tests.rs        (the single registered test binary)
|-- macros.rs       (rgtest! and eqnice!)
|-- util.rs         (Dir + TestCommand harness)
|-- binary.rs  feature.rs  json.rs  misc.rs  multiline.rs
`-- regression.rs   (1,744 lines of issue-numbered tests)
```

The reasoning: one binary links once (integration test link time is the dominant test
cost), and black-box tests survive refactors that inline tests do not. clap's
integration tree outweighs its core crate at 30,959 lines, wired together with
`automod::dir!` for the same one-binary linking benefit. uv is the extreme case, with
roughly 229k lines under extras/uv/crates/uv/tests/ split into parallel binaries
(`it/`, `pip/`, `sync/`, `lock/`, ...).

**Camp B: inline `cfg(test)` only, no `tests/` directory at all.** starship (1,302
tests in 114 inline modules), gitui (318 colocated tests), rustdesk (202 tests beside
platform-gated code), zed (mostly per-crate `*_tests.rs` files inside `src/`). The
reasoning: these codebases test through an in-process harness (`ModuleRenderer`, the
`Gitui` struct, `gpui::TestAppContext`) rather than a spawned binary, so the
white-box access of an inline module is the point, and there is no link-time tax to
amortize.

**Camp C: replace libtest entirely.** nushell registers a single test binary with the
default harness disabled (extras/nushell/Cargo.toml):

```toml
autotests = false

[[test]]
name = "tests"
path = "tests/main.rs"
harness = false
```

Its kitest-plus-linkme harness adds `#[serial]`, `#[env(...)]`, `#[exp(...)]`, and
`#[deps(NU)]` attributes that libtest cannot express. deno's spec suite is likewise a
`harness = false` binary run by `file_test_runner` over 2,087 `__test__.jsonc`
manifests (counted under extras/deno/tests/specs/). The reasoning: when the test
language itself is data (JSONC manifests, transcript files), a custom runner buys
flaky tracking, sharding, and manifest linting that libtest cannot provide.

#### CLI end-to-end harnesses: hand-rolled, assert_cmd, or transcript-driven

Three styles coexist. ripgrep and fd hand-roll a process harness because they predate
the ecosystem crates and want total control of diff output. bat, uv, and ruff build
on `assert_cmd` (bat directly, uv via `uv-test`, ruff via `insta_cmd` which wraps it).
clap owns the third style: `trycmd` replays committed TOML and Markdown transcripts
against compiled example binaries (extras/clap/tests/ui.rs):

```rust
let t = trycmd::TestCases::new();
t.register_bins(trycmd::cargo::compile_examples(["--features", &features]).unwrap());
t.case("tests/ui/*.toml");
```

A transcript fixture is plain data (extras/clap/tests/ui/help_flag_stdout.toml):

```toml
bin.name = "stdio-fixture"
args = ["--help"]
status.code = 0
stdout = """
Usage: stdio-fixture[EXE] [OPTIONS] [NAME] [ENV] [COMMAND]
...
```

The transcript camp argues the fixtures double as documentation and are editable by
non-Rust contributors; the `assert_cmd` camp argues Rust-side assertions compose
better with fixtures and filters. deno's `__test__.jsonc` spec manifests
(`extras/deno/tests/specs/add/no_save/__test__.jsonc`, with `"tempDir": true` and
per-step `args`/`output` pairs) are the same idea scaled to two thousand scenarios,
matched by a custom wildcard language (`[WILDCARD]`, `[WILDLINE]`,
`[UNORDERED_START]`) implemented in extras/deno/tests/util/lib/wildcard.rs.

For terminal-real testing, bat opens actual PTYs via `nix::pty::openpty` with
`wait-timeout` hang protection, deno drives its REPL through a portable-pty wrapper
(extras/deno/tests/util/lib/pty.rs), and clap's `completest-pty` types into real
bash, zsh, fish, elvish, and nushell shells installed in CI.

#### Snapshot testing: insta, expect-test, golden files, or nothing

insta is the plurality choice: uv (6,430 `uv_snapshot!` call sites, counted in
extras/uv/crates/), ruff (84 `snapshots/` directories plus full CLI snapshots), gitui,
tauri (with per-platform `Settings::set_snapshot_path`), and meilisearch. The uv
macro shows the pattern of wrapping insta once per project
(extras/uv/crates/uv-test/src/lib.rs):

```rust
macro_rules! uv_snapshot {
    ($spawnable:expr, @$snapshot:literal) => {{
        uv_snapshot!($crate::INSTA_FILTERS.to_vec(), $spawnable, @$snapshot)
    }};
    ($filters:expr, $spawnable:expr, @$snapshot:literal) => {{
        let (snapshot, output) = $crate::run_and_format($spawnable, &$filters,
            $crate::function_name!(), Some($crate::WindowsFilters::Platform), None);
        ::insta::assert_snapshot!(snapshot, @$snapshot);
        output
    }};
```

Filters normalize paths, timings, and Windows-only diffs so one snapshot serves all
platforms; the same technique appears in gitui (extras/gitui/src/gitui.rs uses
`insta::Settings` `add_filter` to redact temp paths and commit hashes before
`assert_snapshot!("app_loading", terminal.backend())` on a
`Terminal::new(TestBackend::new(90, 12))`). ruff hard-gates hygiene: CI runs
`cargo insta test --all-features --unreferenced reject` so orphaned snapshot files
fail the build (extras/ruff/.github/workflows/ci.yaml, line 386).

meilisearch dissents on snapshot size: `meili-snap` stores only inline md5 hashes and
writes full snapshots to disk only when `MEILI_TEST_FULL_SNAPS=true`
(extras/meilisearch/crates/meili-snap/src/lib.rs), trading reviewability for a diff
that never drowns a PR. bat prefers `expect-test`, snapshotting `--help` into
committed `doc/*.txt` files via `expect_test::expect_file!`
(extras/bat/tests/integration_tests.rs, `fn test_help`), plus a `snapshot_tests!`
macro generating 26 style permutations against a programmatically built git repo
(extras/bat/tests/snapshot_tests.rs). clap snapshots styled help output as
reviewable SVG through snapbox's `term-svg` feature (extras/clap/Cargo.toml,
`snapbox = { version = "1.2.0", features = ["term-svg"] }`).

The oldest camp uses raw golden files with hand-rolled replay: alacritty's 45
recorded PTY sessions are diffed grid cell by grid cell through a declarative macro
(extras/alacritty/alacritty_terminal/tests/ref.rs):

```rust
macro_rules! ref_tests {
    ($($name:ident)*) => {
        $(
            #[test]
            fn $name() {
                let test_dir = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/ref"));
                let test_path = test_dir.join(stringify!($name));
                ref_test(&test_path);
            }
        )*
    };
}
```

fd, starship, and rustdesk use no snapshot tooling at all, on the argument that
their outputs are short enough for literal assertions with diff-formatted failures.

#### Property testing and fuzzing: safety-critical parsers only

No project property-tests everything. The pattern is surgical: proptest or quickcheck
guards algebraic invariants (deno proves `Ord` transitivity and deny-precedence of
its permission system; helix round-trips diff apply/revert via `quickcheck`
(extras/helix/helix-core/Cargo.toml); tauri runs proptest at 10,000 cases on event
listener keys; zed writes a custom `Arbitrary` for its `SumTree`), while cargo-fuzz
covers parsers of untrusted input. Fuzz crates are consistently workspace-excluded
packages: extras/ripgrep/fuzz/fuzz_targets/fuzz_glob.rs asserts glob round-trip
properties, extras/ruff/fuzz/fuzz_targets/ holds six targets including
`ruff_parse_idempotency.rs` and `ruff_formatter_idempotency.rs`, deno fuzzes npm
packument parsing under extras/deno/libs/npm/fuzz/, nushell fuzzes `nu-parser` and
`nu-path`, and meilisearch runs a stateful indexing fuzzer for up to 72 hours on
pushes to main. tokio layers loom model checking on top
(extras/tokio/tokio/Cargo.toml, `[target.'cfg(loom)'.dev-dependencies]`), which no
other project needs because no other project hand-writes a scheduler. Six projects
(fd, bat, starship, gitui, alacritty, rustdesk) ship no fuzzing at all; all six parse
comparatively little untrusted input or delegate parsing to fuzzed dependencies.

#### Benchmarks: criterion, divan, and the continuous-benchmark split

criterion remains the default (bevy at extras/bevy/benches/Cargo.toml pinning
`criterion = { version = "0.8.0", features = ["html_reports"] }` with
`autobenches = false`; tokio at extras/tokio/benches/Cargo.toml; helix behind a
`bench` feature; meilisearch and zed in leaf crates). clap chose divan for its lower
boilerplate, naming benches after real CLIs (extras/clap/clap_bench/benches/ripgrep.rs,
rustup.rs). The continuous camp wires benches to a tracking service: uv and ruff use
the codspeed-criterion-compat shim (extras/uv/crates/uv-bench/Cargo.toml renames the
`criterion` dependency to `codspeed-criterion-compat`), nushell uses tango-bench for
paired runs, and deno benchmarks the release binary with wrk and hyperfine, publishing
to a gh-pages site. fd deliberately keeps benchmarks in a separate hyperfine repo,
and bat commits hyperfine scripts under extras/bat/tests/benchmarks/: macro-level
CLI latency is better measured by an external process timer than by in-process
criterion loops.

#### Runners and coverage

cargo-nextest has majority momentum among the large workspaces: uv
(extras/uv/.config/nextest.toml defines `profile.ci` with `fail-fast = false`,
JUnit output, and per-OS inherited profiles), ruff (extras/ruff/.config/nextest.toml
turns deadlocks into failures with `slow-timeout = { period = "1s", terminate-after
= 60 }` and a `serial` test group for its file watcher), zed
(extras/zed/.config/nextest.toml uses `priority` overrides to run the slowest tests
first), plus tokio and gitui installing it via `taiki-e/install-action` in CI.
Projects on plain libtest (ripgrep, fd, bat, alacritty, clap, starship) are exactly
the ones with a single test binary, where nextest's per-test process isolation and
scheduling buy little.

Coverage as a gate is rare. Only starship runs `cargo llvm-cov --all-features
--locked --workspace --lcov -- --include-ignored` in CI
(extras/starship/.github/workflows/workflow.yml) and nushell scripts it through
`cargo llvm-cov show-env` in extras/nushell/toolkit/coverage.nu; neither enforces a
numeric threshold. The other sixteen enforce behavior directly (snapshots, invariant
tests, fuzzing) rather than a percentage, a deliberate stance that coverage numbers
reward line execution, not assertion quality.

### 23.3 Comparison across the eighteen repositories

| Repository | Integration layout | CLI / E2E harness | Snapshots | Property / fuzz | Benchmarks | Runner |
|---|---|---|---|---|---|---|
| rustdesk | inline `cfg(test)` only | runnable examples as manual harnesses | none | none | example binaries | libtest, `--skip` by name in CI |
| tauri | `crates/tests` + inline | `MockRuntime` headless IPC (1,413 lines) | insta, per-platform paths | proptest 10k cases, quickcheck | custom strace harness | libtest |
| deno | `tests/specs` golden manifests | `file_test_runner` + PTY driver | `.out` golden files + wildcard language | proptest + cargo-fuzz | wrk/hyperfine on release binary | custom `harness = false` |
| uv | `crates/uv/tests` (~229k lines) | `uv-test` + assert_cmd + `uv_snapshot!` | insta with filters, 6,430 sites | test-feature gating, no fuzz | codspeed-criterion | nextest ci profiles |
| zed | inline per-crate test files | `#[gpui::test]` seeded executors | limited | proptest + seeded scheduling | criterion + hyperfine perf | nextest with priorities |
| ripgrep | single binary, `autotests = false` | `Dir`/`TestCommand`, `rgtest!` per engine | golden diffs via `eqnice!` | cargo-fuzz `fuzz_glob` | globset benches + benchsuite | libtest |
| alacritty | `alacritty_terminal/tests/ref` | headless `Term` replay | 45 golden ref fixtures | none | external vtebench | libtest |
| bat | `tests/integration_tests.rs` (4,644 lines) | assert_cmd factory + real PTY (nix) | expect-test help files + style matrix | none | hyperfine scripts | libtest + serial_test |
| starship | inline only, no `tests/` | `ModuleRenderer` (864 sites) | none | none | timings subcommand | libtest + llvm-cov |
| meilisearch | per-crate + real HTTP server | typestate `Server<Owned>/<Shared>` | meili-snap md5-hashed insta | 4 fuzz crates + stateful fuzzer | criterion + span dashboard | libtest |
| ruff | `crates/ruff/tests` + inline | insta-cmd `assert_cmd_snapshot!` | 3,703 insta, unreferenced rejected | 6 libFuzzer + differential | CodSpeed crate | nextest ci profile |
| bevy | `tests/` tutorials + excluded consumer crate | example-run with RON configs | Pixel Eagle screenshots | Miri, ui_test compile-fail | criterion, `autobenches = false` | libtest + ui_test |
| helix | `helix-term/tests/integration.rs` | `AppBuilder` key-sequence DSL | none | quickcheck round-trips | criterion behind `bench` feature | libtest, `integration` profile |
| fd | `tests/tests.rs` + `testenv/` | `TestEnv` via `CARGO_BIN_EXE_fd` | none, diff-based literals | none | external hyperfine repo | libtest + test-case |
| nushell | one `harness = false` binary | in-process `NuTester` + `Playground` | ast-grep rule snapshots | quickcheck + cargo-fuzz | tango-bench paired | kitest + linkme |
| tokio | `tokio/tests`, 174 area files | `tokio-test` mocks + trybuild UI tests | `.stderr` snapshots | proptest + loom + fuzz | criterion `benches/` | nextest |
| gitui | inline only | full-app ratatui `TestBackend` | insta with redaction filters | none | flamegraph feature | nextest |
| clap | `tests/` + transcript fixtures | trycmd + completest-pty shells | snapbox term-svg + trybuild | none | divan `clap_bench` | libtest + automod |

### 23.4 Exemplary excerpts

**Run every end-to-end test under every engine.** ripgrep's `rgtest!` macro reruns
each of its 334 invocations once per regex engine when the pcre2 feature is on
(extras/ripgrep/tests/macros.rs):

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

**One assertion for exit code, stdout, and stderr.** ruff's CLI tests import
`insta_cmd::{assert_cmd_snapshot, get_cargo_bin}` (extras/ruff/crates/ruff/tests/config.rs)
so a single snapshot pins the full observable contract of an invocation, with tempdir
path filters keeping it stable across platforms.

**Timeouts as deadlock detectors.** ruff's nextest CI profile
(extras/ruff/.config/nextest.toml):

```toml
[profile.ci]
failure-output = "immediate-final"
fail-fast = false
slow-timeout =  { period = "1s", terminate-after = 60 }
```

Any test that hangs for sixty periods is terminated and reported instead of wedging
the CI job, which converts event-loop deadlocks from infrastructure mysteries into
named test failures.

**A TUI snapshot in five lines.** gitui builds the real app, draws into a
`TestBackend`, and snapshots the buffer (extras/gitui/src/gitui.rs):

```rust
let mut terminal =
    Terminal::new(TestBackend::new(90, 12)).unwrap();

gitui.draw(&mut terminal).unwrap();

assert_snapshot!("app_loading", terminal.backend());
```

**Declarative scenarios as data.** deno's per-directory manifest
(`extras/deno/tests/specs/add/no_save/__test__.jsonc`) chains real CLI steps in a
temp dir, each checked against a golden `.out` file, and a CI lint fails when any
`.out` file is unreferenced by a manifest. The scenario corpus grows without any new
Rust being written.

### 23.5 What a new Rust project should do

- Put integration tests in one registered binary: `autotests = false` plus a single
  `tests/<name>.rs` including modules, in the ripgrep and clap style, to keep link
  time flat as the suite grows.
- Drive the real binary via `CARGO_BIN_EXE_<name>` inside `TempDir` fixtures, with a
  small harness struct owning setup, invocation, and diff-formatted failure output.
- Adopt insta early and wrap it in one project macro like `uv_snapshot!`, with
  filters for paths, timings, and hashes; snapshot exit code, stdout, and stderr
  together via insta-cmd; run `cargo insta test --unreferenced reject` in CI.
- Snapshot `--help` for every subcommand into committed files (expect-test or trycmd
  transcripts) so the CLI surface cannot drift silently.
- For a TUI, render into `ratatui::backend::TestBackend` and snapshot the buffer;
  for terminal-real behavior, open a PTY with hang protection (`wait-timeout`).
- Property-test only algebraic invariants: parsers round-trip, orderings are
  transitive, apply/revert is identity. Use proptest or quickcheck with seeds
  honored from the environment.
- Add a workspace-excluded `fuzz/` package with a cargo-fuzz target for every parser
  of untrusted input, and at least `cargo check` it in CI on every run.
- Keep shared fakes and builders in a dedicated test-support crate or a
  `test-support` cargo feature, never duplicated per test file.
- Run tests under cargo-nextest with a `ci` profile: `fail-fast = false`, a
  `slow-timeout` with `terminate-after` as a deadlock detector, JUnit output, and
  serial `test-groups` for anything touching shared global state.
- Benchmark at two levels: criterion or divan for hot in-process paths, hyperfine on
  the built binary for end-to-end latency; wire results to a tracking service or a
  committed baseline before optimizing anything.
- Skip a coverage-percentage gate; if coverage is wanted, run cargo-llvm-cov with
  `--include-ignored` as an informational job and enforce behavior through
  snapshots, invariant tests, and fuzzing instead.
- Engineer determinism explicitly: pin git config in fixtures, sandbox global tool
  config, inject clocks behind `cfg(test)` seams, and serialize tests that cannot be
  isolated.

---

## Error Handling and API Design

This chapter synthesizes how eighteen production Rust codebases (rustdesk, tauri, deno, uv,
zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio,
gitui, clap) handle errors and shape their public APIs: library choice (anyhow, thiserror,
hand-rolled), context discipline, exit-code taxonomies, panic and unwrap policy, and the
API-hardening toolkit of builders, newtypes, sealed traits, visibility rules, and `must_use`.

### Consensus practices

Nearly every project in the corpus converges on the same skeleton, regardless of domain:

1. **Structured error types at crate boundaries, opaque aggregation at the top.** Library
   crates define enums (thiserror-derived or hand-written) so callers can match on failure
   modes; the binary crate collapses them into one reporting path. Helix is the cleanest
   two-tier example: `extras/helix/helix-lsp/src/lib.rs` defines a typed enum while
   `extras/helix/helix-term/src/main.rs` wraps startup in `anyhow` context:

   ```rust
   let args = Args::parse_args().context("could not parse arguments")?;
   ```

2. **Exactly one place decides the process exit code.** ripgrep, fd, uv, ruff, bat, and clap
   all funnel every outcome through a single function or enum before `main` returns. Deno
   enforces this mechanically: `extras/deno/cli/clippy.toml` bans the raw call:

   ```toml
   { path = "std::process::exit", reason = "use deno_runtime::exit instead" },
   ```

3. **Broken pipes are success, not a crash.** ripgrep (`extras/ripgrep/crates/core/main.rs`),
   bat (`extras/bat/src/error.rs`), and helix all special-case `ErrorKind::BrokenPipe`.
   Deno goes further and makes every print EPIPE-tolerant via `extras/deno/libs/print/lib.rs`:

   ```rust
   /// Like `std::println!`, but drops write errors instead of panicking.
   #[macro_export]
   macro_rules! drop_println {
   ```

4. **Errors carry actionable context, not just a message.** The pattern is a context payload
   with structure: tauri's `Error::Fs { context, path, error }`, zed's subprocess error with
   stdout, stderr, and status, tokio returning the caller's value back inside the error.

5. **Panics are for developer bugs; user input never panics.** clap panics from
   `assert_app` in `extras/clap/clap_builder/src/builder/debug_asserts.rs` when the CLI
   definition itself is wrong, but returns exit code 2 for user mistakes. Every TUI
   (gitui, helix, nushell) installs a panic hook that restores the terminal first.

6. **Newtypes make invalid states unrepresentable** (nushell `Id<M, V>`, deno
   `CheckedPath`, uv `DisplaySafeUrl`, alacritty `Line`/`Column`), and **builders plus
   `must_use` make misuse loud** (54 `#[must_use]` sites in
   `extras/clap/clap_builder/src/builder/command.rs` alone).

### Divergent camps

#### anyhow vs thiserror vs hand-rolled

**Camp 1: thiserror at boundaries, anyhow at the application layer.** Helix, zed, uv,
ruff, and ripgrep's binary follow this. Zed keeps `thiserror` structs recoverable through
`anyhow` by downcasting (`extras/zed/crates/git/src/repository.rs`):

```rust
if let Some(GitBinaryCommandError { status, .. }) =
    error.downcast_ref::<GitBinaryCommandError>()
    && status.code() == Some(1)
{
    return Ok(false);
}
```

The reasoning: `anyhow` gives free context chaining and one rendering path, while typed
errors survive inside it for callers that need to branch. uv counts 217 `.context(...)`
call sites under `extras/uv/crates/` on top of typed per-crate error enums.

**Camp 2: thiserror everywhere, no anyhow.** bat, gitui, tauri, meilisearch, nushell, and
deno keep a closed error vocabulary. Tauri's CLI even replaces anyhow with its own
`Context` trait so filesystem failures always carry the path
(`extras/tauri/crates/tauri-cli/src/error.rs`):

```rust
#[error("{context} {path}: {error}")]
Fs {
  context: &'static str,
  path: PathBuf,
  error: std::io::Error,
},
```

Deno layers a `Boxed` derive on top so large kind-enums stay one pointer wide
(`use boxed_error::Boxed;` in `extras/deno/libs/package_json/lib.rs`), and maps kinds to
user-facing error classes. Meilisearch splits `UserError` from `InternalError` and folds
foreign errors in with a macro (`extras/meilisearch/crates/meilisearch-types/src/error.rs`):

```rust
macro_rules! internal_error {
    ($target:ty : $($other:path), *) => {
```

The reasoning: servers and libraries need stable, matchable, serializable error taxonomies;
an opaque `anyhow::Error` cannot drive an HTTP status code or a JS exception class.

**Camp 3: hand-rolled `std::error::Error` impls, no derive at all.** alacritty, tokio, and
clap. Alacritty writes `Display` and `source` by hand
(`extras/alacritty/alacritty/src/config/mod.rs`):

```rust
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::ReadingEnvHome(err) => err.source(),
```

Tokio's reason is API precision: `SendError<T>(pub T)` in
`extras/tokio/tokio/src/sync/mpsc/error.rs` returns the caller's payload so a failed send
is recoverable without cloning. Clap's reason is binary size and rich rendering control.
A fourth, minority position is rustdesk: anyhow everywhere via the `ResultType` alias
imported from its shared crate (`extras/rustdesk/src/kcp_stream.rs`), acceptable because
the consumers are its own UIs, not third parties.

#### Exit-code taxonomies

Three designs appear:

- **Semantic enum converted at the edge.** fd (`extras/fd/src/exit_codes.rs`) models
  outcomes, including Unix signal convention, and re-raises SIGINT so the shell sees 130:

  ```rust
  ExitCode::Success => 0,
  ExitCode::HasResults(has_results) => !has_results as i32,
  ExitCode::GeneralError => 1,
  ExitCode::KilledBySigint => 130,
  ```

  ruff (`extras/ruff/crates/ruff/src/lib.rs`) does the same with
  `impl From<ExitStatus> for ExitCode` mapping Success/Failure/Error to 0/1/2, and uv adds
  `External(u8)` to propagate a child process's code
  (`extras/uv/crates/uv/src/commands/mod.rs`). uv further classifies at the top with a
  three-variant `UvError` (User, Argument, Unexpected) that picks the status in
  `extras/uv/crates/uv/src/lib.rs`.
- **grep convention: 0 matched, 1 no match, 2 error.** ripgrep computes this in `run` and
  tracks non-fatal errors in a global flag (`extras/ripgrep/crates/core/messages.rs`):

  ```rust
  /// Flipped to true when an error message is printed.
  static ERRORED: AtomicBool = AtomicBool::new(false);
  ```

- **2 reserved for usage errors.** clap hard-codes it
  (`extras/clap/clap_builder/src/util/mod.rs`):

  ```rust
  pub(crate) const SUCCESS_CODE: i32 = 0;
  pub(crate) const USAGE_CODE: i32 = 2;
  ```

Long-lived processes opt out: starship modules degrade to a logged `None` so the prompt
always renders (`return None;` on command failure in `extras/starship/src/modules/rust.rs`),
and meilisearch maps errors to HTTP codes instead of process codes.

#### Panic policy and unwrap discipline

- **Hard wall:** gitui denies both lints at the crate root (`extras/gitui/src/main.rs`):

  ```rust
  #![deny(
      clippy::unwrap_used,
      clippy::filetype_is_file,
      clippy::cargo,
      clippy::panic,
  ```

  nushell does it once for 34 crates via `unwrap_used = "deny"` under
  `[workspace.lints.clippy]` in `extras/nushell/Cargo.toml`. Both pair the wall with a
  terminal-restoring panic hook; gitui's calls `shutdown_terminal()` before printing the
  backtrace (`extras/gitui/src/main.rs`, `set_panic_handler`).
- **Graded policy:** zed panics in debug and logs with a backtrace in release
  (`extras/zed/crates/gpui_util/src/lib.rs`):

  ```rust
  macro_rules! debug_panic {
      ( $($fmt_arg:tt)* ) => {
          if cfg!(debug_assertions) {
              panic!( $($fmt_arg)* );
  ```

  bevy's `DebugCheckedUnwrap` in `extras/bevy/crates/bevy_ecs/src/query/mod.rs` is the
  performance-critical variant: `unwrap_unchecked` in release, a `#[track_caller]` panic in
  debug.
- **Deliberate panics as API contract:** clap treats a malformed `Command` definition as a
  developer error and panics during the debug-assert validation pass
  (`extras/clap/clap_builder/src/builder/debug_asserts.rs`); tokio puts `#[track_caller]`
  on 167 sites so those panics blame user code, not tokio internals.

### Comparison table

| Repository | Boundary error style | Top-level aggregation | Exit-code taxonomy | Panic/unwrap policy |
|---|---|---|---|---|
| rustdesk | anyhow (`ResultType` alias) | anyhow chains | n/a (GUI/service) | permissive; release `panic=abort` |
| tauri | thiserror, `#[non_exhaustive]` | custom `Context` trait, `fs_context` | CLI nonzero on error | `missing_docs` warned, no unwrap wall |
| deno | thiserror kind-enums + `boxed_error` | kinds to JS error classes | single exit fn; raw `exit` banned | SAFETY comments denied-by-lint |
| uv | thiserror per crate | `UvError` {User, Argument, Unexpected} over anyhow | `ExitStatus` 0/1/2 + `External(u8)` | `unsafe_code = warn`, `expect` over `allow` |
| zed | thiserror structs | anyhow + `downcast_ref` recovery | n/a (GUI) | graded `debug_panic!`, minidumps |
| ripgrep | hand-rolled in lib crates | anyhow in binary | 0/1/2, BrokenPipe to 0, `ERRORED` flag | unwraps allowed, 5 unsafe sites |
| alacritty | hand-rolled, manual `Display`/`source` | log-and-continue | minimal | `must_use` arithmetic, no wall |
| bat | thiserror, `#[non_exhaustive]` | `default_error_handler` | BrokenPipe exits 0 | `#![deny(unsafe_code)]` |
| starship | none central; `Option` degrade | logged `None` per module | prompt always renders | mock seams instead of panics |
| meilisearch | thiserror User/Internal split | `Code` taxonomy to HTTP | server codes, not process codes | fuzzers assert only internal variants panic |
| ruff | thiserror per crate | anyhow at CLI | `ExitStatus` 0/1/2 | `expect(reason)` required |
| bevy | thiserror + boxed `BevyError` | `BevyError` with backtrace | n/a (engine) | `DebugCheckedUnwrap`, SAFETY audits |
| helix | thiserror at boundaries | anyhow + `.context` in binary | BrokenPipe swallowed | graded startup fallback |
| fd | anyhow in binary | anyhow chains | enum incl. 130 for SIGINT | one justified allow in src/ |
| nushell | thiserror + miette `ShellError` | miette diagnostics | pipeline-derived | workspace `unwrap_used = deny` |
| tokio | hand-rolled, payload-returning | n/a (library) | n/a | `track_caller` x167, loom/miri |
| gitui | thiserror in asyncgit | app-level `Result` | TUI, hook-guarded | deny `unwrap_used` + `panic` |
| clap | hand-rolled rich `Error` | `Error::exit()` | 0 success, 2 usage | panic on developer misuse |

### Exemplary excerpts: the API-hardening toolkit

The same repositories that are strict about errors are strict about API shape. The two
disciplines reinforce each other:

```text
public API surface
+-- builders (#[must_use] chain)      -> misuse is a warning
+-- newtypes (private fields)         -> invalid values cannot exist
+-- sealed traits (private supertrait)-> impls cannot exist downstream
+-- visibility lints                  -> accidental API cannot leak
```

**Builders and must_use.** Clap's `Command` builder marks every chaining method
(`extras/clap/clap_builder/src/builder/command.rs`):

```rust
#[must_use]
pub fn arg(mut self, a: impl Into<Arg>) -> Self {
```

Alacritty extends the idea to value arithmetic
(`extras/alacritty/alacritty_terminal/src/index.rs`):

```rust
#[must_use = "this returns the result of the operation, without modifying the original"]
pub fn sub<D>(mut self, dimensions: &D, boundary: Boundary, rhs: usize) -> Self
```

Tokio writes the reason into the message, e.g.
`#[must_use = "futures do nothing unless you .await or poll them"]` at
`extras/tokio/tokio/src/time/sleep.rs`.

**Newtypes.** Nushell prevents index mixups with a phantom marker
(`extras/nushell/crates/nu-protocol/src/id.rs`):

```rust
pub struct Id<M, V = usize> {
    inner: V,
    _phantom: PhantomData<M>,
}
```

uv's `pub struct DisplaySafeUrl(Url);` (`extras/uv/crates/uv-redacted/src/lib.rs`) is
`repr(transparent)` and redacts credentials on `Display`, so logging a URL can never leak a
token. Deno's `CheckedPath<'a>` (`extras/deno/runtime/permissions/lib.rs`) keeps its fields
private so an unchecked path is a compile error outside the permissions crate, and names the
escape hatch `unsafe_new`. Ripgrep prices a soundness contract into a constructor name the
same way: `pub unsafe fn auto() -> MmapChoice` in
`extras/ripgrep/crates/searcher/src/searcher/mmap.rs`.

**Sealed traits.** Tauri shares default methods across six handle types while keeping the
trait unimplementable downstream (`extras/tauri/crates/tauri/src/lib.rs`):

```rust
pub trait Manager<R: Runtime>: sealed::ManagerBase<R> {
```

with `pub(crate) mod sealed` holding `ManagerBase`. Clap seals an entire autoref
specialization ladder (`_impls_ValueParserFactorySealed` and friends in
`extras/clap/clap_builder/src/builder/value_parser.rs`) so parser selection stays correct
on stable Rust while `TypedValueParser` remains open for extension.

**Visibility discipline.** ripgrep makes docs a compile gate per library crate
(`#![deny(missing_docs)]` at `extras/ripgrep/crates/globset/src/lib.rs`). Tokio polices its
public types mechanically via an `allowed_external_types` list in
`extras/tokio/tokio/Cargo.toml`. uv runs a dedicated public-API linter whose overrides carry
reasons (`extras/uv/hawk.toml`: `reason = "shipped package manager binary"`), and deno's
per-crate `clippy.toml` disallowed-methods lists turn layering rules into compile errors.

### What a new Rust project should do

- [ ] Use thiserror enums in every library crate; reserve anyhow (with `.context` at each
      fallible boundary) for the binary crate only.
- [ ] Mark public error enums `#[non_exhaustive]` and give variants structured fields
      (path, command, status), never bare strings, following tauri and bat.
- [ ] Return caller payloads from queue/channel-shaped APIs (`SendError<T>(pub T)`).
- [ ] Model exit codes as an enum with `From<ExitStatus> for ExitCode`; adopt 0/1/2
      semantics, 2 for usage errors, and 130 for SIGINT like fd, ruff, and clap.
- [ ] Exit through exactly one function and ban `std::process::exit` in clippy.toml with a
      reason string, following deno.
- [ ] Map `ErrorKind::BrokenPipe` anywhere in the chain to exit 0, and use EPIPE-tolerant
      print macros for all stdout writing.
- [ ] Deny `clippy::unwrap_used` and `clippy::panic` workspace-wide; allow them only in
      tests, and route unavoidable invariants through a `debug_panic!`-style graded helper.
- [ ] Install a panic hook that restores the terminal (or flushes state) before printing
      the backtrace, and put `#[track_caller]` on every panicking helper.
- [ ] Treat misconfigured API usage as a developer error: validate in a debug-assert pass
      that panics, like clap's `assert_app`.
- [ ] Wrap domain values in newtypes with private fields; name escape hatches loudly
      (`unsafe_new`, `unsafe fn auto`) so review catches them.
- [ ] Put `#[must_use]` on every builder method and on pure arithmetic that returns a new
      value, with a message explaining why.
- [ ] Seal traits that exist for internal dispatch with a `pub(crate) mod sealed`
      supertrait; leave extension points unsealed deliberately.
- [ ] Gate the public surface: `#![deny(missing_docs)]` in library crates, an external-types
      allowlist, and semver checks in CI.

---

## Deep Rust Language Idioms

Tooling chapters describe what surrounds the code. This chapter is about the code itself: how
eighteen mature Rust projects (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat, starship,
meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap) actually use the language. The
dimensions examined are iterator pipelines, zero-copy data flow and `Cow`, borrowing and lifetimes
in public APIs, trait design and generics, interior mutability selection, concurrency primitives,
macro usage, unsafe policy and its documentation, and `cfg`-based platform handling. The projects
disagree loudly about runtimes and unsafe budgets, but they converge on a surprisingly small set of
shared idioms, and the convergence is where the transferable lessons live.

### Consensus practices

**1. `Cow` is the universal answer to "sometimes borrowed, sometimes computed".** Every project in
the set that transforms text or paths on a hot path reaches for `Cow` rather than allocating
unconditionally. ripgrep's printer stores the path it will print as `Cow<'a, [u8]>` so the common
case (print the path as given) borrows, and only separator rewriting allocates
(extras/ripgrep/crates/printer/src/util.rs):

```rust
pub(crate) struct PrinterPath<'a> {
    // On Unix, we can re-materialize a `Path` from our `Cow<'a, [u8]>` with
    // zero cost, so there's no point in storing it. ...
    #[cfg(not(unix))]
    path: &'a Path,
    bytes: Cow<'a, [u8]>,
    hyperlink: OnceCell<Option<HyperlinkPath>>,
}
```

starship parses its entire prompt format string into a `Cow`-based AST so unchanged literal text is
never copied (extras/starship/src/formatter/model.rs):

```rust
pub enum FormatElement<'a> {
    Text(Cow<'a, str>),
    Variable(Cow<'a, str>),
    ...
}
```

alacritty queues PTY writes as `Cow<'static, [u8]>` so static escape sequences cost nothing while
user input is owned (extras/alacritty/alacritty_terminal/src/event_loop.rs, field
`write_list: VecDeque<Cow<'static, [u8]>>`). uv has 168 `Cow` return sites, fd wraps the match
string and sanitized output in `Cow<OsStr>`/`Cow<str>`, ruff's trivia utilities return
`Cow<'_, str>` so a transform that changes nothing allocates nothing, and nushell's
`strip_trailing_slash` does the same. When a project outgrows plain `Cow` it compresses it rather
than abandoning it: helix packs the owned/borrowed bit into the length field of a pointer-sized
string (extras/helix/helix-core/src/graphemes.rs):

```rust
/// A highly compressed Cow<'a, str> that holds
/// atmost u31::MAX bytes and is readonly
pub struct GraphemeStr<'a> {
    ptr: NonNull<u8>,
    len: u32,
    phantom: PhantomData<&'a str>,
}
```

**2. Newtypes carry invariants, not just names.** The consensus is that a wrapper type should make
an illegal state unrepresentable or an expensive mistake impossible, and the wrapper should usually
be `repr(transparent)` when it needs to cast from the inner type. nushell prevents mixing its many
integer ID spaces with a phantom marker (extras/nushell/crates/nu-protocol/src/id.rs):

```rust
pub struct Id<M, V = usize> {
    inner: V,
    _phantom: PhantomData<M>,
}
...
pub type VarId = Id<marker::Var>;
pub type DeclId = Id<marker::Decl>;
pub type BlockId = Id<marker::Block>;
```

uv guarantees credentials can never leak into logs by making the redacting wrapper the only URL
type that circulates (extras/uv/crates/uv-redacted/src/lib.rs):

```rust
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, RefCast)]
#[repr(transparent)]
pub struct DisplaySafeUrl(Url);
```

The same shape recurs everywhere: alacritty's `Line`/`Column` index newtypes
(extras/alacritty/alacritty_terminal/src/index.rs), gitui's `CommitId` and `RepoPath`
(extras/gitui/asyncgit/src/sync/commit_id.rs), deno's `CheckedPath<'a>` with private fields so an
unchecked path is a type error, bevy's `EntityIndex(NonMaxU32)` for niche-optimized
`Option<Entity>`, and nushell's `Path<Form>` typestate over `std::path::Path`.

**3. Lazy initialization uses the standard library.** `OnceLock`, `LazyLock`, and `OnceCell` have
displaced `lazy_static` in every actively modernized codebase. ripgrep's crate docs state the policy
outright and use only std lazies; fd keeps `OnceLock` statics for its regex, Aho-Corasick automaton,
and hostname, plus per-entry `OnceCell` metadata memoization (extras/fd/src/dir_entry.rs); bat
caches theme deserialization in `serde(skip)` `OnceCell` fields; and starship puts the lazies
directly on its context object so the cache lifetime equals one prompt render
(extras/starship/src/context/mod.rs):

```rust
dir_contents: OnceLock<Result<DirContents, std::io::Error>>,
...
git_repo: OnceLock<Result<GitRepo, Box<gix::discover::Error>>>,
```

Only rustdesk still leans on `lazy_static` globals, and it is also the oldest codebase in the set
by idiom vintage.

**4. Unsafe is quarantined, and where it exists it is documented in a `SAFETY:` dialect.** No
project sprinkles unsafe through business logic. The universal move is to confine it to a small
number of modules (platform FFI, one data structure, one launcher crate) and to write a
`// SAFETY:` comment per block. deno and bevy make the comment mandatory through the compiler:
deno's workspace rustflags deny the lint (extras/deno/.cargo/config.toml):

```toml
rustflags = [
  "-D", "clippy::all",
  "-D", "clippy::await_holding_refcell_ref",
  "-D", "clippy::missing_safety_doc",
  "-D", "clippy::undocumented_unsafe_blocks",
]
```

which yields 1,497 `SAFETY:` comments in deno and 2,039 in bevy (1,771 in `bevy_ecs` alone).
Projects that need almost no unsafe make the near-zero count itself an API statement: ripgrep has
five unsafe sites in roughly fifty thousand lines, and one of them is a deliberately unsafe
constructor that prices the memory-map contract into the signature
(extras/ripgrep/crates/searcher/src/searcher/mmap.rs):

```rust
    /// # Safety
    ///
    /// This constructor is not safe because there is no obvious way to
    /// encapsulate the safety of file backed memory maps on all platforms
    /// without simultaneously negating some or all of their benefits.
    pub unsafe fn auto() -> MmapChoice {
        MmapChoice(MmapChoiceImpl::Auto)
    }
```

fd contains exactly one unsafe block in `src/`, and it is the POSIX-mandated one: restoring the
default `SIGINT` handler and re-raising the signal so the shell observes death-by-signal
(extras/fd/src/exit_codes.rs). gitui writes `#![forbid(unsafe_code)]` at the top of its binary and
helper crates (extras/gitui/src/main.rs).

**5. Platform code lives behind a uniform re-exported surface, not scattered branches.** The idiom
is a `cfg`-gated module pair with a glob re-export, so call sites contain zero conditional
compilation (extras/alacritty/alacritty_terminal/src/tty/mod.rs):

```rust
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use self::unix::*;
```

```text
alacritty_terminal/src/tty/
|-- mod.rs        cfg-gated re-exports, shared types
|-- unix.rs       openpty, fork, signal handling
`-- windows/      ConPTY implementation
```

fd writes paired `cfg(unix)`/`cfg(windows)` total functions with identical signatures, nushell
isolates per-OS process code in `nu-system` with one uniform re-exported API, uv and deno go one
step further and dedicate whole crates (`uv-unix`, `uv-windows`, deno's Windows-only crates) gated
by crate-level `#![cfg]`. Where the platform axis is not the OS triple but a capability, projects
register custom cfgs from build scripts and dispatch on those: rustdesk's screen-capture crate
compiles `quartz`, `x11`, or `dxgi` backends behind build-script-emitted cfgs
(extras/rustdesk/libs/scrap/src/lib.rs), tauri registers `desktop`/`mobile`/`dev` aliases with
`rustc-check-cfg`, and nushell injects `cfg(ci)` via `--config .cargo/ci.toml` allowlisted through
`unexpected_cfgs`.

**6. Trait design favors small traits with associated types and default methods at architecture
seams.** ripgrep's `Sink` is the reference example: a callback trait whose associated error type
lets the caller own error semantics, whose default methods make the minimal impl three lines, and
whose `Ok(false)` return is cooperative cancellation (extras/ripgrep/crates/searcher/src/sink.rs).
tauri seals its most powerful trait so downstream crates get the shared default methods but cannot
add impls that would break invariants (extras/tauri/crates/tauri/src/lib.rs):

```rust
pub trait Manager<R: Runtime>: sealed::ManagerBase<R> {
```

When a trait must be object safe, projects accept the constraint consciously: nushell's `Command`
uses a `CommandClone` supertrait plus `Any` downcasting, gitui renders through
`&mut [&mut dyn Component]`, and ripgrep registers every CLI flag as a unit struct implementing a
`Flag` trait object. When a trait can be generic, the ambitious projects use the full width of the
system: zed's `SumTree` uses a generic associated type for its summary context
(extras/zed/crates/sum_tree/src/sum_tree.rs, `pub trait Summary: Clone { type Context<'a>: Copy; }`)
and helix keeps rendering allocation-free by drawing from any iterator of cells
(extras/helix/helix-tui/src/backend/mod.rs):

```rust
    fn draw<'a, I>(&mut self, content: I) -> Result<(), io::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>;
```

**7. Macros are rare, load-bearing, and paired with an escape hatch.** Nobody writes macros for
brevity alone. The recurring justifications are: expressing a partial borrow the borrow checker
cannot see (helix's `current!`, which splits `&mut Editor` into view and document halves,
extras/helix/helix-view/src/macros.rs), giving `?` a scope in a non-`Result` context (zed's
`maybe!`, an immediately invoked closure, extras/zed/crates/gpui_util/src/lib.rs), stamping `cfg`
plus `doc(cfg)` on whole item groups (tokio's 64 `cfg_*` wrappers,
extras/tokio/tokio/src/macros/cfg.rs), and stable-Rust specialization via autoref, the most
technical macro trick in the set (extras/clap/clap_builder/src/builder/value_parser.rs, line 2630:
`(&&&&&&auto).value_parser()` ranks six parser sources by how many auto-derefs each impl needs).
Zero-cost logging twins are the other consensus macro: clap ships an empty `macro_rules!` twin of
its `debug!` macro so tracing costs nothing when the feature is off, and gitui's `scope_time!`
compiles to nothing without its timing feature.

### Divergent camps

#### Concurrency model: async runtime, thread pool, or plain threads

Three camps, each with a coherent rationale.

The **async camp** runs tokio but splits again on flavor. uv, meilisearch's HTTP layer, helix, and
rustdesk use multi-threaded tokio with `Send` futures. deno deliberately runs the current-thread
flavor so its per-isolate state can be `Rc<RefCell<OpState>>` with no atomics on the hot path, and
it builds `MaybeSend`/`MaybeArc` aliases so shared crates compile in both worlds
(extras/deno/libs/maybe_sync/lib.rs):

```rust
  pub use std::rc::Rc as MaybeArc;
  pub trait MaybeSync {}
  impl<T> MaybeSync for T where T: ?Sized {}
  pub trait MaybeSend {}
  impl<T> MaybeSend for T where T: ?Sized {}
```

The **data-parallel camp** (ruff, starship, meilisearch's indexer) uses rayon: the unit of work is
a file or a module, the pipeline is CPU-bound, and work stealing beats an executor. starship caps
the global pool at 8 threads because a prompt render saturates long before core count does.

The **plain-threads camp** (ripgrep, fd, alacritty, gitui, bat, clap) argues that a CLI or a
single-window app does not need a runtime at all. ripgrep builds its parallel walker directly on
`crossbeam-deque` stealers; fd uses `std::thread::scope` so worker threads borrow `&Config` with no
`Arc` in sight (extras/fd/src/walk.rs, `thread::scope(|scope| { ... })`); gitui multiplexes six
`crossbeam_channel` receivers through one `Select` in a single-threaded event loop
(extras/gitui/src/main.rs, line 294: `let mut sel = Select::new();`). Cancellation in this camp is
a relaxed `AtomicBool`, wrapped by nushell into a reusable `Signals` type whose error branch is a
`#[cold]` inner function.

tokio itself sits outside the camps as the arms supplier, and its discipline is the strictest: all
`UnsafeCell` access happens through `with`/`with_mut` closures so raw pointers never escape
(extras/tokio/tokio/src/loom/std/unsafe_cell.rs), and every synchronization primitive is compiled
against either std or the loom model checker through a two-line facade
(extras/tokio/tokio/src/loom/mod.rs):

```rust
#[cfg(not(all(test, loom)))]
mod std;
#[cfg(not(all(test, loom)))]
pub(crate) use self::std::*;

#[cfg(all(test, loom))]
mod mocked;
```

#### Interior mutability: pick the cheapest cell that survives your threading model

The selection logic is consistent even though the selections differ. Single-threaded code uses
`Rc<RefCell<T>>` without apology: gitui's UI thread shares options and its event queue that way
(extras/gitui/src/options.rs, `pub type SharedOptions = Rc<RefCell<Options>>;`) while the same
project uses `Arc<Mutex<T>>` across its threadpool boundary (extras/gitui/asyncgit/src/revlog.rs).
deno bets its whole op system on `RefCell` and then guards the known failure mode with a workspace
lint, `-D clippy::await_holding_refcell_ref`, so a borrow can never be held across an await.

The read-mostly camp replaces locks with atomically swappable snapshots: helix serves language
configuration from `Arc<ArcSwap<Loader>>` so the render path never blocks on a config reload
(extras/helix/helix-core/src/syntax.rs, `scopes: ArcSwap<Vec<String>>`), and nushell clones its
engine state cheaply and mutates through `Arc::make_mut` for copy-on-write semantics.

Two projects invented primitives where nothing off the shelf fit. alacritty's `FairMutex` composes
two `parking_lot` mutexes so the renderer can take a lease that guarantees it the next lock
(extras/alacritty/alacritty_terminal/src/sync.rs):

```rust
pub struct FairMutex<T> {
    /// Data.
    data: Mutex<T>,
    /// Next-to-access.
    next: Mutex<()>,
}
```

uv's `OnceMap` coalesces concurrent requests for the same key on top of a lock-free papaya map plus
`tokio::sync::Notify` (extras/uv/crates/uv-once-map/src/lib.rs). And meilisearch's `RefCellExt`
turns a failed `RefCell` borrow inside a rayon worker into `rayon::yield_local()` instead of a
panic (extras/meilisearch/crates/milli/src/update/new/ref_cell_ext.rs), a pattern worth knowing:
inside a cooperative scheduler, a busy borrow means "another task on this thread is mid-flight",
and yielding is the correct response.

#### Iterator style: pull pipelines versus the push model

Most projects write conventional external-iterator pipelines (`filter_map`, `Either`-based
branching to keep both arms lazily typed: 30 files in uv use `Either::Left`/`Either::Right`, bat
runs `filter_map_ok` pipelines that collect into `Result` in
extras/bat/build/syntax_mapping.rs). ripgrep dissents for its core search interface and documents
why with unusual care (extras/ripgrep/crates/matcher/src/lib.rs):

```text
A key design decision made in this crate is the use of *internal iteration*,
or otherwise known as the "push" model of searching. In this paradigm,
implementations of the `Matcher` trait will drive search and execute callbacks
provided by the caller ...
* Some search implementations may themselves require internal iteration.
* Rust's type system isn't quite expressive enough to write a generic interface
  using external iteration without giving something else up (namely, ease of
  use and/or performance).
```

The lesson is not "use push iteration" but "when a trait must be generic over engines that iterate
internally, push is the lowest common denominator, and the tradeoff belongs in the crate docs".

#### Unsafe budget: zero, minimal, or industrial

The `SAFETY:` comment counts measured across the clones tell the story at a glance: bat, fd, and
gitui sit at zero (with `deny`/`forbid(unsafe_code)`); ripgrep, alacritty, helix, and starship stay
under five; uv, ruff, zed, and nushell run a few dozen with wrapper-crate quarantine; tokio (145),
deno (1,497), and bevy (2,039) run industrial unsafe with lint-enforced documentation, Miri, and in
tokio's case loom and sanitizers. The split is driven by domain, not taste: nothing in a CLI
justifies unsafe, and nothing in a scheduler or ECS avoids it. The transferable rule is that the
budget must be explicit. meilisearch shows the strongest single artifact of that explicitness, an
unsafe marker trait whose entire contract is prose (extras/meilisearch/crates/milli/src/update/new/thread_local.rs):

```rust
/// It is **always safe** to implement this trait on a type that is `Send`, but no
/// placeholder impl is provided due to limitations in coherency. Use the
/// [`FullySend`] wrapper in this situation.
pub unsafe trait MostlySend {}

// SAFETY: a type **fully** send is always mostly send as well.
unsafe impl<T> MostlySend for FullySend<T> where T: Send {}
```

### Comparison across the eighteen repositories

`SAFETY:` counts are measured over the clones with `grep -rno "SAFETY:" --include="*.rs"`.

| Repository | Concurrency model | Interior mutability flavor | Unsafe stance (SAFETY: count) | Signature zero-copy idiom |
|---|---|---|---|---|
| rustdesk | tokio, plus scoped current-thread runtimes | `lazy_static` `Arc<RwLock>` globals | FFI-quarantined, crate-level allows (24) | borrowed `Frame<'a>` capture enum |
| tauri | main-thread event loop plus channels | `Mutex` over a TypeId map | documented one-off sites (15) | `Cow` assets iterator, embedded vs compressed |
| deno | current-thread tokio, `!Send` futures | `Rc<RefCell<OpState>>`, lint-guarded | lint-mandated docs (1,497) | `#[buffer] Cow<[u8]>` V8 fast-call args |
| uv | multi-thread tokio | lock-free papaya `OnceMap` | `unsafe_code = warn`, wrappers (56) | rkyv `OwnedArchive` cache reads, 168 `Cow` sites |
| zed | gpui foreground/background executors | entity model, seeded executor | deny in hot crates, dylint audit (74) | borrowed `Chunks<'a>` rope iterators |
| ripgrep | crossbeam-deque work stealing | atomics only | five sites, unsafe-priced API (4) | `PrinterPath<'a>` `Cow<'a, [u8]>` |
| alacritty | threads around a PTY event loop | custom `FairMutex` | counted-instruction justifications (4) | `Cow<'static, [u8]>` write queue |
| bat | single-threaded | unsync `OnceCell` caches | `#![deny(unsafe_code)]` (0) | `InputKind<'a>` with `Box<dyn Read + 'a>` |
| starship | rayon `par_iter`, pool capped at 8 | `OnceLock` fields, static `parking_lot::Mutex` | one Win32 file, RAII handle (1) | `Cow<'a, str>` format AST |
| meilisearch | rayon indexer, actix HTTP | `RefCellExt` yield-on-borrow | 88 sites, prose contracts (8) | zero-copy `Cow` bitmap codec decode |
| ruff | rayon per file, salsa queries | `OnceCell` lazy `LineIndex` | warn workspace, forbid in leaves (61) | `Cow<'_, str>` trivia returns |
| bevy | custom ECS scheduler | `UnsafeWorldCell` disjoint access | deny plus `expect(reason)` (2,039) | `Cow<'static, str>` names, `bevy_ptr` |
| helix | multi-thread tokio | `Arc<ArcSwap<Loader>>` | three sites, packed pointer (3) | `GraphemeStr` compressed `Cow` |
| fd | `std::thread::scope`, bounded channels | relaxed `AtomicBool` flags | one block, signal re-raise (0) | `Cow<OsStr>` match string |
| nushell | threads, `Arc::make_mut` snapshots | `Signals` over `Option<Arc<AtomicBool>>` | written policy, POSIX citations (34) | `Cow` path returns, `Id<M>` newtypes |
| tokio | is the runtime; loom-verified internals | `UnsafeCell` behind `with` closures | `deny(unsafe_op_in_unsafe_fn)` (145) | vectored IO, `CachePadded` layout |
| gitui | crossbeam `Select` loop plus threadpool | `Rc<RefCell>` UI, `Arc<Mutex>` jobs | `#![forbid(unsafe_code)]` (0) | `Cow<'_, str>` trim, borrowed tree iterator |
| clap | single-threaded parse | none needed | quarantined in clap_lex (31) | `Str` newtype, `&'static str` unless owned |

### Exemplary excerpts

Beyond the excerpts already quoted, three more repay study.

**The callback trait with a caller-owned error type.** ripgrep's `Sink` shows how to design a trait
that a searcher drives without dictating error handling
(extras/ripgrep/crates/searcher/src/sink.rs):

```rust
pub trait Sink {
    /// The type of an error that should be reported by a searcher.
    ///
    /// Errors of this type are not only returned by the methods on this
    /// trait, but the constructors defined in `SinkError` are also used in
    /// the searcher implementation itself.
    type Error: SinkError;
```

**The feature-cfg macro that keeps docs honest.** tokio's `cfg_rt!` stamps both the `cfg` and the
docs.rs `doc(cfg)` badge on every item it wraps, so feature gating and documentation can never
drift apart (extras/tokio/tokio/src/macros/cfg.rs):

```rust
macro_rules! cfg_rt {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "rt")]
            #[cfg_attr(docsrs, doc(cfg(feature = "rt")))]
            $item
        )*
    }
}
```

**The lifetime-carrying extractor.** tauri's managed state is handed to command handlers as a
borrow whose lifetime is the invocation, so state can never escape a request
(extras/tauri/crates/tauri/src/state.rs):

```rust
/// A guard for a state value.
pub struct State<'r, T>(&'r T);
```

The pattern generalizes: when an API hands out access to something it owns, wrap the reference in a
named guard type rather than returning `&T` directly, because the guard is where `Deref`, `Debug`,
and future invariants live.

### What a new Rust project should do

- Return `Cow<'_, str>` (or `Cow<'_, [u8]>`, `Cow<'_, OsStr>`) from any function that transforms
  input it usually leaves unchanged, and audit hot paths for unconditional `to_string` calls.
- Wrap every domain identifier and every string with an invariant in a newtype; use
  `#[repr(transparent)]` plus `RefCast` when you need free conversion from the inner type, and a
  `PhantomData` marker when several ID spaces share one representation.
- Use `OnceLock`/`LazyLock`/`OnceCell` from std for all lazy initialization; do not add
  `lazy_static` or `once_cell` to a new project.
- Decide the unsafe budget on day one and encode it: `#![forbid(unsafe_code)]` for a CLI or TUI,
  or `unsafe_code = "deny"` in workspace lints with per-crate `expect(reason)` opt-in, plus
  `-D clippy::undocumented_unsafe_blocks` so every block that does exist carries a `SAFETY:`
  comment stating the invariant and who upholds it.
- Put platform differences behind `cfg`-gated module pairs with identical public signatures and a
  `pub use self::unix::*` style re-export, so call sites never branch; register any custom cfgs
  through build scripts and `rustc-check-cfg`.
- Pick the concurrency model by workload, not fashion: plain threads with `std::thread::scope` and
  crossbeam channels for a CLI, rayon for per-file data parallelism, tokio only when the program is
  genuinely IO-concurrent, and current-thread tokio with `Rc<RefCell>` state if nothing needs
  `Send` (then deny `clippy::await_holding_refcell_ref`).
- Match interior mutability to the threading model: `Rc<RefCell>` inside one thread, `Arc<Mutex>`
  across threads, `ArcSwap` for read-mostly configuration, `Arc::make_mut` for snapshot-and-mutate
  state, and a relaxed `AtomicBool` (nushell's `Signals` shape) for cancellation.
- Design boundary traits small: an associated error type, default methods so the minimal impl is
  tiny, `Ok(false)` or an enum for cooperative cancellation, and a sealed supertrait when
  downstream impls would be a liability.
- Keep declarative macros for what functions cannot do: partial borrows (`current!`), `?` scoping
  (`maybe!`), item-group `cfg` stamping (`cfg_rt!`), and zero-cost logging twins; give every macro
  a documented expansion and prefer a function the moment one suffices.
- Prefer iterator arguments (`impl Iterator<Item = ...>` or a generic `I: Iterator`) over slices in
  rendering and formatting APIs so callers can stream without collecting; if an interface must wrap
  engines that iterate internally, adopt the push model deliberately and write the rationale into
  the crate docs the way ripgrep does.
- Ban the hazards you have wrapped: once a safe wrapper exists for process spawning, filesystem
  access, or time, add the raw call to `clippy.toml` `disallowed-methods` with a reason and
  replacement so the idiom enforces itself.

---

## Dependencies, Releases, and Distribution

This chapter synthesizes how eighteen mature Rust codebases manage the full lifecycle
from dependency intake to shipped binary: lockfile policy, minimal feature selection,
MSRV declaration and verification, automated update bots, changelog discipline,
release automation, binary distribution matrices, and the packaging of shell
completions and man pages. The projects span applications (ripgrep, fd, bat, gitui,
alacritty, helix, nushell, starship, zed, rustdesk), services (meilisearch, deno, uv,
ruff), frameworks (bevy, tauri, tokio), and a library with a CLI-shaped test surface
(clap). Where they agree, the agreement is strong evidence; where they split, the
split almost always tracks whether the artifact is a binary or a library.

### Consensus practices

**Commit the lockfile, build with `--locked`.** Sixteen of eighteen repositories
commit `Cargo.lock`. The two that do not, tokio and bevy, are libraries that must
prove they build against fresh resolutions (`extras/tokio/.gitignore` line 2 and
`extras/bevy/.gitignore` line 17 both list `Cargo.lock`). Application projects treat
the lockfile as the release manifest: `extras/bat/.github/workflows/CICD.yml`
contains twenty `--locked` invocations and fd's release workflow uses
`cargo build --profile $(PROFILE) --locked` in `extras/fd/Makefile`. Meilisearch goes
further and refuses to tag a release unless the lockfile agrees with the manifest
(see the excerpt from `check-release.sh` below).

**Declare MSRV in exactly one place and read it back mechanically.** Fifteen
projects declare `rust-version` in `Cargo.toml` (rustdesk 1.75, tauri 1.90, uv
1.95.0, ripgrep 1.96, alacritty 1.85.0, bat 1.88, starship 1.95, ruff 1.95, bevy
1.96.0, helix 1.90, fd 1.90.0, nushell 1.95.0, tokio 1.71 in
`extras/tokio/tokio/Cargo.toml`, gitui 1.88, clap 1.85). Deno, meilisearch, and zed
instead pin the whole toolchain in `rust-toolchain.toml` (1.95.0, 1.91.1, and 1.97.1
respectively) and treat the pin as the support statement. The consensus refinement
is that CI must never hardcode a second copy of the number: it extracts the value
from the manifest, so a bump is a one-line change.

**Run an update bot with a cooldown.** Fifteen projects run dependabot or Renovate.
The distinctive shared idea is a deliberate delay before adopting a new release, to
let yanked or malicious versions surface. fd sets `cooldown: default-days: 7` in
`extras/fd/.github/dependabot.yml`, meilisearch does the same, tauri sets
`"minimumReleaseAge": "3 days"` in `extras/tauri/renovate.json`, and starship sets
`minimumReleaseAge: '4 days'` in `extras/starship/.github/renovate.json5`.

**Trim features and write down why.** Nearly every manifest disables default
features on heavy dependencies and carries a comment explaining the trim or the pin.
`extras/starship/Cargo.toml` line 43 reads
`# default feature restriction addresses https://github.com/starship/starship/issues/4251`
above its `gix` dependency; `extras/deno/Cargo.toml` line 292 pins
`reqwest = { version = "=0.12.5", ... } # pinned because of https://github.com/seanmonstar/reqwest/pull/1955`;
`extras/meilisearch/crates/meilisearch/Cargo.toml` line 118 explains
`# fixed version due to format breakages in v1.40` above `insta = { version = "=1.39.0" }`.

**Generate completions and man pages from the CLI definition, never by hand.** Every
CLI in the set derives its completion scripts and man page from the same source that
parses arguments: ripgrep from its `Flag` trait registry
(`extras/ripgrep/crates/core/flags/complete/`), bat in its build script
(`extras/bat/build/application.rs`), fd via a hidden `--gen-completions` flag driven
by `extras/fd/Makefile`, starship and uv as subcommands
(`uv generate-shell-completion` is smoke tested by eval in
`extras/uv/.github/workflows/test-smoke.yml` line 43), alacritty and helix from
checked-in files kept honest by tests and packaging metadata.

**Gate the release on tag-equals-manifest.** ripgrep, meilisearch, and gitui all
refuse to build release artifacts when the pushed tag disagrees with `Cargo.toml`.
This one check prevents the classic failure of shipping a binary whose `--version`
does not match its release page.

### Divergent camps

#### Lockfile policy: applications commit, libraries do not, clap does both

The application camp (all sixteen binaries and services) commits `Cargo.lock` and
passes `--locked` everywhere, making CI and releases reproducible. The library camp
(tokio, bevy) gitignores it so CI always resolves fresh, catching breakage in the
version ranges the library actually publishes. clap occupies a third position: it is
a library but commits its lockfile anyway and adds a CI job that fails when the
committed lockfile drifts from a fresh resolution, in
`extras/clap/.github/workflows/ci.yml`:

```yaml
  lockfile:
    runs-on: ubuntu-latest
    steps:
    ...
    - name: "Is lockfile updated?"
      run: cargo update --workspace --locked
```

Both library repos compensate for the missing lockfile with `-Z minimal-versions`
jobs (`extras/tokio/.github/workflows/ci.yml` line 794, and clap runs
`cargo +nightly generate-lockfile -Z minimal-versions` at line 192 of its ci.yml),
proving that the lower bounds in their version requirements are honest.

#### MSRV verification: five extraction styles, one principle

Every project that verifies MSRV in CI derives the toolchain from the manifest, but
the mechanism varies:

- **cargo metadata plus jq** (fd, bat, bevy). `extras/fd/.github/workflows/CICD.yml`
  line 34: `cargo metadata --no-deps --format-version 1 | jq -r '"msrv=" + .packages[0].rust_version' | tee -a $GITHUB_OUTPUT`.
- **A TOML-reading action** (ruff). `extras/ruff/.github/workflows/ci.yaml` line 530
  uses `SebRollen/toml-action` to read `workspace.package.rust-version`.
- **Shell grep** (alacritty). `extras/alacritty/.github/workflows/ci.yml` line 24:
  `rustup default $(cat Cargo.toml | grep "rust-version" | sed 's/.*"\(.*\)".*/\1/')`.
- **A consistency gate between two declarations** (nushell).
  `extras/nushell/.github/workflows/check-msrv.nu` compares
  `rust-toolchain.toml`'s channel against `workspace.package.rust-version` and exits
  1 on mismatch.
- **A named constant** (tokio: `rust_min: '1.71'` in ci.yml; gitui and clap: a
  literal matrix row, with clap's annotated `rust: "1.85"  # MSRV` so a Renovate
  regex manager can bump it).

Two projects also write the policy down. Helix documents in
`extras/helix/docs/CONTRIBUTING.md`:

```markdown
Helix keeps an intentionally low MSRV for the sake of easy building and packaging
downstream. We follow [Firefox's MSRV policy]. Once Firefox's MSRV increases we
may bump ours as well, but be sure to check that popular distributions like Ubuntu
package the new MSRV version.
```

uv encodes its rolling policy directly in the bot config,
`extras/uv/.github/renovate.json5`:

```json5
      commitMessageTopic: "MSRV",
      // We have a rolling support policy for the MSRV
      // 2 releases back * 6 weeks per release * 7 days per week + 1
      minimumReleaseAge: "85 days",
```

ripgrep adds a per-crate wrinkle: the workspace pins 1.96, but the reusable library
crates keep an older floor for downstream consumers
(`extras/ripgrep/crates/globset/Cargo.toml` and `crates/ignore/Cargo.toml` both
declare `rust-version = "1.88"`).

#### Update bots: dependabot for cadence, Renovate for policy

Ten repositories use dependabot (rustdesk, bat, meilisearch, bevy, helix, fd,
nushell, tokio, gitui, and bat again for submodules); six use Renovate (tauri, uv,
zed, starship, ruff, clap); ripgrep, alacritty, and deno use neither and instead
fold `cargo update` review into a release checklist
(`extras/ripgrep/RELEASE-CHECKLIST.md`: "Run `cargo update` and review dependency
updates. Commit updated `Cargo.lock`."). The Renovate camp chooses it for expressive
policy: tauri disables `oxc_*` crates "because of MSRV and PR spam" and groups all
windows-rs crates in `extras/tauri/renovate.json`; uv and zed use custom regex
managers to bump tool versions embedded inside workflow `run:` steps; clap's
Renovate bumps the pinned lint toolchain via a `# STABLE` comment. The dependabot
camp values simplicity plus grouping: gitui groups cargo updates into rolling minor
and patch PRs, helix groups minor and patch weekly, and rustdesk points dependabot
at a git submodule daily (`extras/rustdesk/.github/dependabot.yml`,
`package-ecosystem: "gitsubmodule"`).

#### Changelog discipline: hand-written, machine-stamped, or externalized

Three camps exist:

1. **Keep-a-changelog by hand, enforced by CI.** gitui's `CHANGELOG.md` opens with
   the Keep a Changelog preamble and CI extracts the release notes on every PR via
   `ffurrer2/extract-release-notes` (`extras/gitui/.github/workflows/ci.yml` line
   334), so a malformed changelog fails before the release. bat goes further with
   `extras/bat/.github/workflows/require-changelog-for-PRs.yml`, which diffs
   `CHANGELOG.md` against the base branch and greps the added lines for the PR
   number and submitter. fd keeps a permanent `# Unreleased` section at the top with
   per-entry credits; ripgrep keeps a standing `TBD` section
   (`extras/ripgrep/CHANGELOG.md`: "Unreleased changes. Release notes have not yet
   been written."); alacritty states its section ordering rule in the file header:
   "The sections should follow the order `Packaging`, `Added`, `Changed`, `Fixed`
   and `Removed`."
2. **Machine-stamped from structured inputs.** clap uses cargo-release
   `pre-release-replacements` in `extras/clap/Cargo.toml` to rewrite `Unreleased`
   headers, compare links, `CITATION.cff`, and even a doc link in `src/lib.rs` at
   tag time. tauri collects per-PR change files under `extras/tauri/.changes/` with
   a tag taxonomy (`feat`, `bug`, `sec`, `breaking`, ...) defined in
   `.changes/config.json`, and covector assembles per-crate changelogs from them.
   starship generates its changelog from conventional commits via release-please
   (`extras/starship/release-please-config.json`, with `"draft": true`).
3. **No changelog file at all.** deno, meilisearch, zed, and nushell write release
   notes outside the repo (release pages or a blog); nushell harvests PR-template
   release-notes sections nearly verbatim into the release blog, and tokio keeps
   per-crate changelogs such as `extras/tokio/tokio/CHANGELOG.md` rather than a
   root file.

Notably, none of the eighteen uses git-cliff; the projects that want generated
changelogs choose tools coupled to their release automation (release-please,
covector, cargo-release) so the changelog and the version bump cannot drift apart.

#### Release automation: three levels of delegation

- **Fully delegated: cargo-dist.** uv and ruff describe their entire release
  pipeline in `dist-workspace.toml` (18 targets each, shell and powershell
  installers, `.tar.gz`/`.zip` archives) and let dist generate the workflows. ruff
  layers governance on top in `extras/ruff/dist-workspace.toml`:

  ```toml
  # Whether CI should trigger releases with dispatches instead of tag pushes
  dispatch-releases = true
  # Whether to enable GitHub Attestations
  github-attestations = true
  ```

  plus a two-person approval environment documented in
  `extras/ruff/.github/workflows/release.yml`: "This environment requires a
  2-factor approval, i.e., the workflow must be approved by another team member."
  uv pairs dist with a `release-prepare.yml` dispatch workflow that runs
  `scripts/release.sh` to open the version-bump PR.
- **Version management delegated, artifacts hand-rolled.** starship lets
  release-please cut the tag and changelog, then a hand-written 13-target matrix
  builds artifacts, publishes to crates.io with OIDC trusted publishing
  (`id-token: write` in `extras/starship/.github/workflows/release.yml`), and only
  flips the draft flag once every artifact and checksum is uploaded
  (`gh release edit ... --draft=false`). tauri's covector and bevy's cargo-release
  (`extras/bevy/.github/workflows/post-release.yml`) sit in this camp too.
- **Fully hand-rolled workflows.** ripgrep, fd, bat, gitui, alacritty, helix,
  meilisearch, nushell, zed, and rustdesk write their own tag-triggered matrix.
  The best of these encode the same safety rails dist gives for free: ripgrep's
  `extras/ripgrep/.github/workflows/release.yml` verifies the tag first:

  ```yaml
      - name: Check that tag version and Cargo.toml version are the same
        shell: bash
        run: |
          if ! grep -q "version = \"$VERSION\"" Cargo.toml; then
            echo "version does not match Cargo.toml" >&2
            exit 1
          fi
  ```

  and meilisearch scripts it in `extras/meilisearch/.github/scripts/check-release.sh`,
  checking both `Cargo.toml` and `Cargo.lock` against `GITHUB_REF`. helix adds a
  preview mode so the release workflow itself can be exercised from a PR without
  tagging (`extras/helix/.github/workflows/release.yml`:
  `preview: ${{ !startsWith(github.ref, 'refs/tags/') || github.repository != 'helix-editor/helix' }}`).
  Version bumps are scripted even here: `extras/fd/scripts/version-bump.sh`,
  `extras/rustdesk/res/bump.sh` (which seds the version across spec files, PKGBUILD,
  pubspec, workflows, and flatpak manifests, then runs `cargo run` to refresh the
  lockfile), and meilisearch's `update-cargo-toml-version.yml` dispatch workflow.

#### Binary distribution matrices and supply-chain proof

Release matrices cluster around 13 to 18 targets. fd's matrix in
`extras/fd/.github/workflows/CICD.yml` is representative: 14 targets spanning
gnu/musl Linux (x86_64, i686, aarch64, arm hard-float), both macOS architectures,
and three Windows toolchains including `aarch64-pc-windows-msvc` on `windows-11-arm`,
with `use-cross: true` rows pinned to cross v0.2.5. ripgrep builds 14 targets with a
dedicated `release-lto` profile and generates docs under qemu for foreign
architectures. meilisearch multiplies 6 platforms by 2 editions
(`edition: [community, enterprise]` in
`extras/meilisearch/.github/workflows/publish-release-assets.yml`). Provenance is
now table stakes: fd runs `actions/attest` gated to version tags, helix and ripgrep
use `actions/attest-build-provenance`, ruff enables `github-attestations` in dist,
and rustdesk attaches a Syft CycloneDX SBOM. Prebuilt-binary installs are served by
`[package.metadata.binstall]` tables in fd, nushell, and tauri's CLI crate
(`extras/fd/Cargo.toml`):

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }.{ archive-format }"
bin-dir = "{ bin }-v{ version }-{ target }/{ bin }{ binary-ext }"
pkg-fmt = "tgz"
```

#### Completions and man pages: generate, verify, package

The strongest pattern is a closed loop: one source of truth generates the artifacts,
a test or CI diff proves the committed copies match, and packaging installs them
into system directories. ripgrep's release job runs the built binary itself:

```yaml
        "$BIN" --generate complete-bash > "$ARCHIVE/complete/rg.bash"
        "$BIN" --generate complete-fish > "$ARCHIVE/complete/rg.fish"
        "$BIN" --generate complete-powershell > "$ARCHIVE/complete/_rg.ps1"
        "$BIN" --generate complete-zsh > "$ARCHIVE/complete/_rg"
        "$BIN" --generate man > "$ARCHIVE/doc/rg.1"
```

(`extras/ripgrep/.github/workflows/release.yml`), producing an archive shaped like:

```text
ripgrep-<version>-<target>/
|-- rg
|-- complete/
|   |-- rg.bash
|   |-- rg.fish
|   |-- _rg
|   `-- _rg.ps1
`-- doc/
    `-- rg.1
```

alacritty checks in its completions but pins them with a unit test at
`extras/alacritty/alacritty/src/cli.rs` line 539 that byte-compares
`extra/completions/*` against `clap_complete` output, and writes its five man pages
in scdoc (`extras/alacritty/extra/man/alacritty.1.scd` and friends), compiled as a
CI docs gate. bat renders both at build time from templates in
`extras/bat/build/application.rs` (`gen_man_and_comp`, with
`cargo:rerun-if-changed=assets/manual/` hooks). helix ships completions through
distro packaging metadata in `extras/helix/helix-term/Cargo.toml`:

```toml
  { source = "../contrib/completion/hx.bash", dest = "/usr/share/bash-completion/completions/hx", mode = "644" },
  { source = "../contrib/completion/hx.fish", dest = "/usr/share/fish/vendor_completions.d/hx.fish", mode = "644" },
```

fd hides the generator behind a default `completions` feature and a Makefile that
also carries the Debian `fdfind` rename variants. starship adds
`clap_complete_nushell` beside `clap_complete` so all six major shells are covered
from one derive.

### Comparison table

| Repository | Cargo.lock | MSRV source | MSRV CI verification | Update bot | Changelog | Release automation |
|---|---|---|---|---|---|---|
| rustdesk | committed | rust-version 1.75 | none found | dependabot (submodule, daily) | none in repo | hand-rolled tag workflows, res/bump.sh |
| tauri | committed | rust-version 1.90 | exact-MSRV toolchain job | Renovate, 3-day age | covector .changes files | covector version-or-publish |
| deno | committed | toolchain pin 1.95.0 | toolchain pin is the check | none | none in repo | scripted, generated workflows |
| uv | committed | rust-version 1.95.0 | pinned toolchain, Renovate-managed | Renovate, 85-day MSRV age | CHANGELOG.md | cargo-dist plus release-prepare dispatch |
| zed | committed | toolchain pin 1.97.1 | toolchain pin is the check | Renovate, weekly | none in repo | hand-rolled v* tag workflow plus nightly |
| ripgrep | committed | rust-version 1.96 (libs 1.88) | pinned MSRV matrix row | none, checklist-driven | manual, standing TBD section | hand-rolled, tag-vs-manifest gate |
| alacritty | committed | rust-version 1.85.0 | grep from Cargo.toml | none | Keep a Changelog, ordered sections | hand-rolled, draft release, human publish |
| bat | committed | rust-version 1.88 | cargo metadata + jq | dependabot, monthly | Keep a Changelog, PR-enforced | hand-rolled single CICD.yml |
| starship | committed | rust-version 1.95 | none dedicated | Renovate, 4-day age | release-please generated | release-please plus 13-target matrix, OIDC publish |
| meilisearch | committed | toolchain pin 1.91.1 | pin mirrored in every job | dependabot, 7-day cooldown | none in repo | hand-rolled, check-release.sh gate, 6x2 matrix |
| ruff | committed | rust-version 1.95 | toml-action read, build on it | Renovate | CHANGELOG.md | cargo-dist, dispatch releases, 2-person gate |
| bevy | not committed | rust-version 1.96.0 | cargo metadata + jq | dependabot | _release-content drafts | cargo-release post-release bump PRs |
| helix | committed | rust-version 1.90 + toolchain | env.MSRV, written Firefox policy | dependabot, grouped weekly | CHANGELOG.md | hand-rolled with preview mode, attestation |
| fd | committed | rust-version 1.90.0 | cargo metadata + jq, clippy+test on MSRV | dependabot, 7-day cooldown | manual, permanent Unreleased | hand-rolled 14-target CICD.yml, attest |
| nushell | committed | rust-version 1.95.0 + toolchain | check-msrv.nu consistency gate | dependabot | none in repo (release blog) | nushell release scripts, WiX, winget |
| tokio | not committed | rust-version 1.71 (per crate) | rust_min env matrix job | dependabot (actions) | per-crate CHANGELOG.md | manual, checklist above version field |
| gitui | committed | rust-version 1.88 (3 places) | literal MSRV matrix row | dependabot, grouped | Keep a Changelog, CI-extracted | hand-rolled cd.yml, homebrew bump |
| clap | committed (library) | rust-version 1.85 | msrv matrix row, minimal-versions, lockfile job | Renovate | cargo-release replacements | cargo-release plus tag notes workflow |

### Exemplary excerpts

**The tag-vs-lockfile release gate**, `extras/meilisearch/.github/scripts/check-release.sh`:

```bash
check_tag() {
    local expected=$1
    local actual=$2
    local filename=$3

    if [[ $actual != $expected ]]; then
        echo >&2 "Error: the current tag does not match the version in $filename: found $actual, expected $expected"
        return 1
    fi
}
```

**MSRV consistency as a hard CI failure**, `extras/nushell/.github/workflows/check-msrv.nu`:

```nu
let toolchain_spec = open rust-toolchain.toml | get toolchain.channel
let msrv_spec = open Cargo.toml | get workspace.package.rust-version

if $toolchain_spec != $msrv_spec {
    print -e "Mismatching rust compiler versions specified in `Cargo.toml` and `rust-toolchain.toml`"
    exit 1
}
```

**Changelog entries as a merge requirement**, `extras/bat/.github/workflows/require-changelog-for-PRs.yml`:

```yaml
      - name: Search for added line in changelog
        run: |
          ADDED=$(git diff -U0 "origin/${PR_BASE}" HEAD -- CHANGELOG.md | grep -P '^\+[^\+].+$')
          grep "#${PR_NUMBER}\\b.*${PR_SUBMITTER}\\b" <<< "$ADDED"
```

**Release mechanics encoded next to the version they release**, `extras/tokio/tokio/Cargo.toml`:

```toml
# When releasing to crates.io:
# - Remove path dependencies (if any)
# - Update doc url
#   - README.md
# - Update CHANGELOG.md.
# - Create "v1.x.y" git tag.
version = "1.53.1"
```

**Facade crates locked in lockstep**, `extras/clap/Cargo.toml`:

```toml
clap_builder = { path = "./clap_builder", version = "=4.6.6", default-features = false }
clap_derive = { path = "./clap_derive", version = "=4.6.4", optional = true }
```

The exact `=` pins guarantee that a `clap` release can never resolve against a
mismatched builder or derive crate, which is the entire point of a facade split.

### What a new Rust project should do

- [ ] Commit `Cargo.lock` for any binary and pass `--locked` to every CI and release cargo invocation; for a library, either gitignore it and add a `-Z minimal-versions` job, or commit it with a `cargo update --workspace --locked` freshness job like clap.
- [ ] Declare `rust-version` once in `Cargo.toml` and have CI extract it (cargo metadata + jq, or a TOML-reading action) to install exactly that toolchain; run clippy and the test suite on it, not just `cargo check`.
- [ ] If you also keep a `rust-toolchain.toml`, add a consistency gate that fails CI when it disagrees with `rust-version`, as nushell does.
- [ ] Write the MSRV bump policy down (rolling window or an external anchor like Firefox's) so bumps are boring.
- [ ] Turn on dependabot or Renovate with a cooldown of 3 to 7 days, group noisy crate families, and let the bot also bump tool versions embedded in workflows via regex managers.
- [ ] Disable default features on heavy dependencies and annotate every pin, trim, or exact version with a comment linking to the issue that forced it.
- [ ] Add unused-dependency detection (cargo-shear, cargo-machete, or cargo-udeps) and a cargo-deny or cargo-audit job with a justified ignore list.
- [ ] Keep a Keep-a-Changelog file with a permanent Unreleased section and enforce entries mechanically per PR, or adopt release automation that generates the changelog from structured inputs; do not do both by hand.
- [ ] Prefer cargo-dist for a standalone binary: one `dist-workspace.toml` buys the target matrix, installers, checksums, and attestations; consider `dispatch-releases` and an approval-gated environment for release control.
- [ ] If hand-rolling the release workflow, gate every job on a tag-equals-`Cargo.toml` (and ideally `Cargo.lock`) check, keep releases draft until all artifacts and checksums upload, and add a preview mode so the workflow is testable from a PR.
- [ ] Script the version bump (one shell script that touches every embedding file and refreshes the lockfile) and keep a committed release checklist.
- [ ] Build at least: gnu and musl Linux on x86_64 and aarch64, both macOS architectures, and x86_64 MSVC Windows; pin cross by version for foreign targets.
- [ ] Sign artifacts with build provenance attestation and publish checksums; publish to crates.io with OIDC trusted publishing instead of a stored token.
- [ ] Generate shell completions and the man page from the clap definition (subcommand or hidden flag), verify the committed copies against generated output in a test, smoke test completions by eval-ing them in a real shell, and package them into release archives and distro metadata.
- [ ] Add `[package.metadata.binstall]` matching the release artifact naming so `cargo binstall` works on day one.

---

## Documentation Practices

Documentation in mature Rust projects is not a single artifact. It is a layered system:
rustdoc on the API surface, a user manual somewhere (in rustdoc, in an mdbook, in a docs
site, or in plain Markdown), a README that acts as the front door, contributor process
docs, and templates that shape every issue and pull request. Across the eighteen
repositories studied here (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat,
starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap), the striking
result is how much of this system is mechanically enforced: doc lints, rustdoc warning
gates, drift checks on generated pages, and CI jobs that reject a PR without a changelog
entry. Documentation that is not checked by a machine decays; these projects know it.

A composite of the documentation surface these repositories converge on:

```text
repo/
|-- README.md                  front door: pitch, install, quickstart
|-- CHANGELOG.md               human-facing history, often CI-enforced
|-- CONTRIBUTING.md            process: build, test, changelog, PR rules
|-- ARCHITECTURE.md            (minority) crate map and design overview
|-- docs/ or book/             manual sources: mdbook, mkdocs, or plain md
|-- .github/
|   |-- ISSUE_TEMPLATE/
|   |   |-- bug_report.yml     structured form with required fields
|   |   `-- config.yml         routes questions to Discussions
|   `-- PULL_REQUEST_TEMPLATE.md
`-- crates/*/src/lib.rs        //! crate docs, doc lints, doc tests
```

### Consensus practices

**Every project ships a CONTRIBUTING document, even when it is tiny.** All eighteen have
one: at the root (extras/uv/CONTRIBUTING.md, extras/ripgrep/CONTRIBUTING.md,
extras/fd/CONTRIBUTING.md, extras/tokio/CONTRIBUTING.md and eleven more), under .github/
(extras/tauri/.github/CONTRIBUTING.md, extras/deno/.github/CONTRIBUTING.md), or inside
the docs tree (extras/helix/docs/CONTRIBUTING.md, plus translated variants such as
extras/rustdesk/docs/CONTRIBUTING-DE.md). Size varies enormously: ripgrep's is eight
lines that mostly defer to a policy file, while uv's opens by teaching contributors how
to pick an issue ("We label issues that we think are a good opportunity for subsequent
contributions as `help wanted`", extras/uv/CONTRIBUTING.md). The consensus is on the
file's existence and its role as the canonical answer to "how do I get a change merged",
not on its length.

**Issue templates plus a config.yml router are universal among projects that take bug
reports, and the routing target is Discussions.** Sixteen of eighteen have an
ISSUE_TEMPLATE directory; alacritty ships none at all, and helix omits the router.
The router pattern is identical everywhere: keep the tracker for actionable reports and
push questions elsewhere. From extras/ripgrep/.github/ISSUE_TEMPLATE/config.yml:

```yaml
blank_issues_enabled: true
contact_links:
  - name: Ask a question
    about: |
      You've come to seek help or want to discuss something related to ripgrep.
    url: https://github.com/BurntSushi/ripgrep/discussions/new
```

**Structured issue forms front-load triage work onto the reporter.** The strongest forms
make the reporter confirm they read the docs before the submit button works. ripgrep's
form lists known non-bugs, then requires a checkbox
(extras/ripgrep/.github/ISSUE_TEMPLATE/bug_report.yml):

```yaml
  - type: checkboxes
    id: issue-not-common
    attributes:
      label: Please tick this box to confirm you have reviewed the above.
      options:
        - label: I have a different issue.
          required: true
```

fd does the same against its README's troubleshooting section and requires the output of
`fd --version` (extras/fd/.github/ISSUE_TEMPLATE/bug_report.yaml). rustdesk, zed, and
starship additionally set `blank_issues_enabled: false` so every issue goes through a
form (extras/rustdesk/.github/ISSUE_TEMPLATE/config.yml).

**The README is a front door, not the manual.** Nearly every README follows the same
arc: pitch, screenshot or demo, install, quickstart, then links out to deeper docs.
ripgrep's README heading list is the archetype: CHANGELOG, documentation quick links,
screenshot, "Why should I use ripgrep?", "Why shouldn't I use ripgrep?", installation,
building, running tests (extras/ripgrep/README.md). Projects with global audiences
maintain translated READMEs: rustdesk links twenty-five language variants from its
README header into docs/ (extras/rustdesk/README.md), and bat keeps README-ja, -ko,
-ru, -zh under extras/bat/doc/.

**Library crates get doc lints; the docs build itself is a CI gate.** Wherever a crate
is meant to be consumed as a library, `missing_docs` appears in some strictness, and six
projects compile rustdoc with warnings denied in CI. ripgrep's matcher crate carries
`#![deny(missing_docs)]` (extras/ripgrep/crates/matcher/src/lib.rs, line 37), and its CI
runs rustdoc with `RUSTDOCFLAGS: -D warnings` (extras/ripgrep/.github/workflows/ci.yml).
The same gate appears in extras/clap/.github/workflows/ci.yml,
extras/bat/.github/workflows/CICD.yml, extras/helix/.github/workflows/build.yml,
extras/ruff/.github/workflows/ci.yaml, and extras/tokio/.github/workflows/ci.yml (which
adds `--cfg docsrs --cfg tokio_unstable` so unstable-feature docs are checked too).

**Generated documentation is committed and drift-checked.** When docs are derived from
code (keymaps, CLI references, config schemas), the generator runs in CI and any diff
fails the build. helix regenerates its mdbook's generated pages and fails with an
actionable message (extras/helix/.github/workflows/build.yml):

```yaml
      - name: Check uncommitted documentation changes
        if: always()
        run: |
          git diff
          git diff-files --quiet \
            || (echo "Run 'cargo xtask docgen', commit the changes and push again" \
            && exit 1)
```

The generated pages live at extras/helix/book/src/generated/ (lang-support.md,
static-cmd.md, typable-cmd.md), produced by extras/helix/xtask/src/main.rs. uv guards
its settings and environment-variable references with `generate-all --mode check`,
starship drift-checks its config schema, and bevy regenerates templated doc pages the
same way.

**The changelog is documentation with a process behind it.** bat enforces it
mechanically: a workflow diffs CHANGELOG.md against the base branch and greps the added
lines for the PR number and submitter
(extras/bat/.github/workflows/require-changelog-for-PRs.yml):

```yaml
          ADDED=$(git diff -U0 "origin/${PR_BASE}" HEAD -- CHANGELOG.md | grep -P '^\+[^\+].+$')
          echo "Added lines in CHANGELOG.md:"
          echo "$ADDED"
          echo "Grepping for PR info (see CONTRIBUTING.md):"
          grep "#${PR_NUMBER}\\b.*${PR_SUBMITTER}\\b" <<< "$ADDED"
```

The policy side lives in extras/bat/CONTRIBUTING.md ("Keeping the `CHANGELOG.md` file
up-to-date makes the release process much easier"), with matching guidance in
extras/fd/CONTRIBUTING.md, gitui's CI job that extracts release notes from the changelog
on every PR, and ripgrep's standing TBD section in extras/ripgrep/CHANGELOG.md.

### Divergent camps

**Where the user manual lives.** This is the deepest split, and it tracks the audience.

- Camp 1, rustdoc is the manual: clap and tokio. clap compiles its tutorial, cookbook,
  and FAQ as rustdoc modules under extras/clap/src/ (`_tutorial.rs`, `_cookbook/`, `_faq.rs`,
  `_derive/`), so every documentation example is a doc test that runs in CI. tokio's
  entire user-facing story is crate docs (extras/tokio/tokio/src/lib.rs), backed by a
  spellcheck dictionary at extras/tokio/spellcheck.dic. Reasoning: for a library, the
  API reference is where users already are, and doc tests make every example
  self-verifying.
- Camp 2, mdbook in-repo: helix and zed. extras/helix/book/book.toml deploys to a
  custom domain with per-page edit links (`edit-url-template =
  "https://github.com/helix-editor/helix/edit/master/book/{path}"`), and zed wraps the
  mdbook HTML renderer with a Rust post-processor (extras/zed/docs/book.toml,
  `command = "cargo run -p docs_preprocessor -- postprocess"`) deployed by
  extras/zed/.github/workflows/deploy_docs.yml. Reasoning: docs versioned with the
  code, reviewable in the same PR, and buildable by the Rust toolchain contributors
  already have.
- Camp 3, a non-Rust static site generator in-repo: uv and ruff use mkdocs-material
  (extras/uv/mkdocs.yml, extras/ruff/mkdocs.template.yml), starship uses vitepress with
  crowdin-managed locale directories (extras/starship/docs/.vitepress/config.mts,
  extras/starship/crowdin.yml). Reasoning: polished product sites, search, theming, and
  translation pipelines that mdbook does not offer.
- Camp 4, plain Markdown in the repo: ripgrep keeps a 1,025-line GUIDE.md and a
  1,063-line FAQ.md at the root, fd keeps the whole manual in README.md ("How to use"
  with fifteen subsections), gitui splits by topic into KEY_CONFIG.md, THEMES.md, and
  FAQ.md at the root, and bat uses doc/ (assets.md, alternatives.md). Reasoning: zero
  build step, zero hosting, and GitHub renders it fine for a single-binary tool.
- Camp 5, the manual lives outside the repo: deno (docs.deno.com), tauri (tauri.app),
  meilisearch, rustdesk, and nushell all point users at separate documentation repos or
  sites, keeping only contributor and process docs in-tree (for nushell, the in-tree
  remainder is extras/nushell/devdocs/ with FAQ.md, HOWTOS.md, PLATFORM_SUPPORT.md,
  rust_style.md). alacritty is its own sub-camp: the manual is five scdoc man pages
  under extras/alacritty/extra/man/ (alacritty.1.scd, alacritty.5.scd,
  alacritty-bindings.5.scd), compiled in CI so broken docs fail the build. Reasoning:
  product-scale docs teams, or in alacritty's case the conviction that a terminal
  emulator's manual belongs in man.

**How strict the missing_docs lint should be.** Four levels are all represented.
gitui forbids it in its library crate (extras/gitui/asyncgit/src/lib.rs, line 11:
`#![forbid(missing_docs)]`). ripgrep denies it in every published crate
(extras/ripgrep/crates/matcher/src/lib.rs). tokio, tauri, and bevy warn: tokio via
`#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms,
unreachable_pub)]` in extras/tokio/tokio/src/lib.rs, tauri via
`#![warn(missing_docs, rust_2018_idioms)]` at line 55 of
extras/tauri/crates/tauri/src/lib.rs, and bevy workspace-wide with
`missing_docs = "warn"` at line 84 of extras/bevy/Cargo.toml, escalated to an error by
`-D warnings` in CI. Application-shaped projects (fd, bat, alacritty, helix, deno,
starship) simply omit it. The reasoning split is clean: the lint pays for itself exactly
when strangers consume the API through docs.rs; forcing doc comments onto internal
binary modules produces boilerplate, not documentation.

**ARCHITECTURE.md: written or skipped.** Only three projects maintain a dedicated
architecture document: tauri at the root (extras/tauri/ARCHITECTURE.md, which opens with
"What Tauri is NOT" before naming each crate's role), helix at
extras/helix/docs/architecture.md (a crate table: "helix-core: Core editing primitives,
functional." then "This document contains a high-level overview of Helix internals"),
and deno, which goes furthest with both extras/deno/doc/architecture.md and a
directory-by-directory extras/deno/doc/codebase-map.md ("A directory-by-directory tour
of the repository, plus the files worth reading first"). The rest either skip it or
substitute narrower dev docs: bevy's extras/bevy/docs/ holds cargo_features.md,
profiling.md, debugging.md, and linters.md; meilisearch versions its process rules in
extras/meilisearch/documentation/ (release.md, versioning-policy.md,
experimental-features.md). The skip camp's implicit argument is visible in ripgrep:
architecture rationale lives in crate docs instead, where it cannot go stale silently
because rustdoc builds it (see the Matcher design discussion below). The write camp's
argument is scale: at 25 to 250 crates, newcomers need a map before an API reference.

**YAML issue forms versus Markdown templates.** Nine projects use YAML forms with typed
fields and `required: true` validation (ripgrep, fd, helix, rustdesk, zed, uv, ruff,
clap, nushell, plus tauri's bug_report.yml). Seven still use free-form Markdown
templates (deno, bat, starship, meilisearch, bevy, tokio, gitui), and alacritty uses
nothing. The forms camp gets machine-enforced version strings and checkboxes; the
Markdown camp keeps friction low and trusts triage. The trend line is one-directional:
every recently overhauled template in this set is a YAML form.

**README as crate docs versus hand-written crate docs.** One camp makes the README the
crate documentation with `#![doc = include_str!("../README.md")]`, which also compiles
and runs the README's code fences as doc tests: meilisearch's utility crates
(extras/meilisearch/crates/permissive-json-pointer/src/lib.rs, line 1) and several
nushell crates (extras/nushell/crates/nu-command/src/lib.rs) do this. The other camp
writes crate docs by hand and keeps the README separate but honest: clap injects the
README into a hidden struct only under `cfg(doctest)` so its examples are tested without
polluting the docs (excerpt below), and tokio's crate docs are independent prose. The
first approach guarantees a single source of truth; the second acknowledges that a good
README and good API docs address different readers.

### Comparison across the eighteen repositories

| Repository | User manual home | Architecture doc | CONTRIBUTING | missing_docs | rustdoc -D warnings CI | Issue templates | PR template |
|---|---|---|---|---|---|---|---|
| rustdesk | external site | none | docs/ + 20 translations | one lib crate | no | YAML form, blank off | no |
| tauri | external site | ARCHITECTURE.md | .github/ | warn in lib crates | no | YAML + md mix | yes, guidelines |
| deno | external site | doc/architecture.md + codebase-map.md | .github/ | no | doctest flags only | Markdown | yes |
| uv | mkdocs in docs/ | none (STYLE.md instead) | root | few lib crates | no | YAML forms | yes |
| zed | mdbook in docs/ | none | root | gpui and lib crates | doctest job | YAML forms, blank off | yes |
| ripgrep | GUIDE.md + FAQ.md in repo | in crate docs | root, 8 lines | deny, all lib crates | yes, private items too | YAML form | no |
| alacritty | scdoc man pages | none | root | no | no | none | yes |
| bat | README + doc/ | none | root | no | yes | Markdown | no |
| starship | vitepress in docs/, crowdin | none | root | no | no | Markdown, blank off | yes |
| meilisearch | external site | documentation/ (process) | root | no | no | Markdown | yes |
| ruff | mkdocs in docs/ | none | root, large | rare | yes | YAML forms | yes |
| bevy | rustdoc + external learn site | docs/ dev guides | root, pointer to site | warn, workspace-wide | deployed docs build | Markdown incl. docs_improvement | yes, Objective/Solution |
| helix | mdbook in book/ | docs/architecture.md | docs/ | no | yes | YAML form | no |
| fd | README is the manual | none | root | no | no | YAML form | no |
| nushell | external book + devdocs/ | devdocs/ | root | rare | no | YAML forms | yes, release notes |
| tokio | rustdoc is the manual | none | root, large | warn, every crate | yes, plus cargo test --doc | Markdown | yes |
| gitui | topic files at root | none | root | forbid in asyncgit | no | Markdown | yes, checklist |
| clap | rustdoc modules (`_tutorial`, `_faq`) | none | root + per-crate | warn | yes | YAML forms | yes |

### Exemplary excerpts

**Design rationale as crate docs, ripgrep.** The Matcher crate's docs explain not just
what the API is but why it is shaped that way, so the architecture discussion is built
and link-checked on every CI run (extras/ripgrep/crates/matcher/src/lib.rs):

```rust
/*!
This crate provides an interface for regular expressions, with a focus on line
oriented search. ...
A key design decision made in this crate is the use of *internal iteration*,
or otherwise known as the "push" model of searching. In this paradigm,
implementations of the `Matcher` trait will drive search and execute callbacks
provided by the caller when a match is found.
*/
```

**Doc tests hardened at the crate root, tokio.** Doc examples are compiled with
warnings denied so samples cannot rot, and docs.rs metadata is pinned in the manifest
(extras/tokio/tokio/src/lib.rs and extras/tokio/tokio/Cargo.toml):

```rust
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
```

```toml
[package.metadata.docs.rs]
all-features = true
# enable unstable features in the documentation
rustdoc-args = ["--cfg", "docsrs", "--cfg", "tokio_unstable"]
```

**README examples as doc tests without README-as-docs, clap.** The README is attached
to a hidden struct only when doc tests run (extras/clap/src/lib.rs, lines 108 to 110):

```rust
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
```

**Help text as a committed doc artifact, bat.** `--help` output is snapshotted into
doc files and asserted by integration tests, so the CLI reference in the repo can never
drift from the binary (extras/bat/tests/integration_tests.rs):

```rust
fn long_help() {
    test_help("--help", "../doc/long-help.txt");
}
```

**An error catalog compiled as documentation, bevy.** Runtime error codes are Markdown
files attached to marker types with `#[doc = include_str!]`, so the catalog is rendered
by rustdoc and its examples are doc-tested (extras/bevy/errors/src/lib.rs):

```rust
//! Definitions of Bevy's error codes that might occur at runtime.
#[doc = include_str!("../B0001.md")]
pub struct B0001;
```

**A PR template that teaches, tauri.** The template shows good and bad PR titles and
points to the change-file requirement
(extras/tauri/.github/PULL_REQUEST_TEMPLATE.md): "Examples of good title:
fix(windows): fix race condition in event loop ... 3. If this change requires a new
version, then add a change file in `.changes` directory". nushell's template goes
further and harvests a "User-facing changes (Release notes)" section nearly verbatim
into the release blog (extras/nushell/.github/pull_request_template.md), while gitui's
is a four-item checklist ending in "I ran `make check` without errors" and "I added an
appropriate item to the changelog" (extras/gitui/.github/PULL_REQUEST_TEMPLATE.md).

**A documentation style contract, uv.** STYLE.md pins wording, punctuation, and
formatting rules for user-facing text ("Use backticks to escape: commands, code
expressions, package names, and file paths", extras/uv/STYLE.md), and clippy's
`doc-valid-idents` list in extras/uv/clippy.toml (also extras/ruff/clippy.toml) keeps
product names like "PyPI" and "CPython" spelled correctly inside doc comments.

### What a new Rust project should do

- [ ] Write a README as a front door: pitch, screenshot or demo, install, quickstart, links to deeper docs; keep the manual elsewhere once it outgrows a screen or two.
- [ ] Add a CONTRIBUTING.md covering build, test, changelog policy, and PR expectations; even a short one beats none.
- [ ] Put `#![deny(missing_docs)]` (or at least `warn`) on every crate meant to be consumed as a library; skip it for internal binary modules.
- [ ] Gate the docs build in CI with `RUSTDOCFLAGS="-D warnings"`, including `--document-private-items` for internal-doc hygiene.
- [ ] Run doc tests in CI (`cargo test --doc`) and harden them with `#![doc(test(attr(deny(warnings))))]` so examples cannot rot.
- [ ] Compile the README's examples: either `#![doc = include_str!("../README.md")]` for a docs-are-the-README crate, or a `#[cfg(doctest)]` ReadmeDoctests struct.
- [ ] Write an ARCHITECTURE.md (or docs/architecture.md) with a crate table and data-flow overview once the workspace passes a handful of crates; add a codebase map when it passes a dozen.
- [ ] Choose one manual home deliberately: rustdoc modules for a library, mdbook in-repo for a tool, mkdocs or similar for a product site; do not split the manual across all three.
- [ ] Generate every derivable doc (CLI reference, keymap, config schema) from code, commit the output, and fail CI on drift with a message naming the regeneration command.
- [ ] Use YAML issue forms with required version fields and a "I read the docs" checkbox, plus a config.yml routing questions to Discussions.
- [ ] Add a PR template that asks for the issue link, testing done, and a changelog or release-notes entry; keep it short enough that people actually fill it in.
- [ ] Enforce the changelog mechanically: a CI job that requires an added CHANGELOG line referencing the PR, with documented exemptions for non-user-facing changes.
- [ ] Snapshot `--help` output into a committed doc file asserted by a test.
- [ ] Pin `[package.metadata.docs.rs]` (all-features, cfg flags) so docs.rs renders what you intend.
- [ ] Add a docs style contract (STYLE.md) and `doc-valid-idents` entries for product names once user-facing prose accumulates.

---

## Quinjet Gap Analysis

This chapter closes the series by turning chapters 01 through 27 back onto quinjet itself.
Part 1 records where quinjet already sits at or above the bar set by the eighteen reference
repositories. Part 2 verifies the scoped claim in ARCHITECTURE.md: every user-visible
repository and GitHub operation reachable in the terminal interface is also reachable from a
command-line subcommand, while presentation state remains terminal-only. Part 3 records the
status of every original gap and retains the evidence that motivated it.

Priorities: P0 is a clear industry consensus quinjet lacks and would gain real value from.
P1 is a strong practice with moderate value. P2 is optional polish. Every recommendation
respects three constraints: the single-crate binary stays single-crate, every feature stays
reachable through the appropriate interface, and each change lands in under roughly 2,000
diff lines.

### Part 1: Where quinjet already meets or exceeds the industry bar

#### The lint wall is stricter than any reference repository

Cargo.toml sets `unsafe_code = "forbid"` plus roughly thirty rustc lints at deny, then puts
clippy `all`, `pedantic`, `nursery`, and `cargo` at deny with priority -1 and layers about
sixty named restriction lints on top: `unwrap_used`, `expect_used`, `panic`,
`indexing_slicing`, `string_slice`, `print_stdout`, `print_stderr`, `exit`, `todo`,
`unimplemented`, `unreachable`, and more. None of the eighteen reference repositories runs a
wall this tall; [Lints and Static Analysis](./patterns/lints-and-static-analysis.md) records nushell and zed as the strictest, and both deny far fewer
restriction lints than quinjet does. The escape hatches are configured correctly too:
clippy.toml carries `msrv = "1.88"`, `allow-unwrap-in-tests`, `allow-expect-in-tests`,
`allow-panic-in-tests`, `allow-indexing-slicing-in-tests`, tuned thresholds, and
`disallowed-methods` entries for `std::env::set_var` and `std::env::remove_var` with reason
strings, the pattern chapters 03, 04, and 15 recommend from extras/deno/cli/clippy.toml and
extras/nushell/clippy.toml. Suppressions use `#[expect(lint, reason = "...")]` because
`allow_attributes` and `allow_attributes_without_reason` are denied in Cargo.toml, which is
the bevy discipline from extras/bevy/Cargo.toml already fully adopted.

#### MSRV is pinned three times, exactly as gitui recommends

`rust-version = "1.88"` in Cargo.toml, `msrv = "1.88"` in clippy.toml, and a CI job in
.github/workflows/ci.yml that installs 1.88 and asserts
`cargo metadata ... | jq -r '.packages[0].rust_version'` equals 1.88 before running
`cargo check`. That is the triple pin from extras/gitui/.github/workflows/ci.yml, plus
.github/workflows/deep.yml runs `cargo msrv verify` weekly to prove 1.88 is the true minimum,
which goes beyond every reference repository.

#### CI topology matches the best of the corpus

.github/workflows/ci.yml has the aggregation gate job (`ci`, `if: always()`, `needs:` on
everything, failing on `failure` or `cancelled`) that [CI CD Patterns](./patterns/ci-cd-patterns.md) traces to
extras/clap/.github/workflows/ci.yml and extras/bat/.github/workflows/CICD.yml, and
.github/workflows/hygiene.yml repeats the pattern for its own fifteen jobs. Every workflow
sets top-level `permissions: contents: read`, every checkout sets
`persist-credentials: false`, every action is pinned to a full commit SHA with a version
comment, every workflow carries a `concurrency` group whose `cancel-in-progress` is
conditional on `pull_request`, and `merge_group:` is in the trigger lists. The test matrix
covers ubuntu, ubuntu-24.04-arm, macos, and windows with `fail-fast: false`, a beta clippy
row runs with `continue-on-error`, and a cross-check matrix covers four extra targets. This
is the union of the hardening chapters 21 and the uv, ripgrep, fd, and bevy workflow lessons,
already implemented.

#### The verification breadth exceeds every reference repository

The Makefile `ci` target chains formatting, clippy, tests in two feature configurations,
rustdoc with `-D warnings`, `--document-private-items`, and denied broken-link lints,
comment and secret checkers, typos, cargo-spellcheck, cargo-deny, cargo-audit, osv-scanner, cargo-machete
plus cargo-shear, cargo-sort, cargo-hack feature powerset, shellcheck plus shfmt, actionlint
plus zizmor in pedantic persona, yamllint strict, markdownlint, taplo fmt plus lint,
editorconfig-checker, ruff, the wiki drift check, and `cargo package`. The `deep` target and
.github/workflows/deep.yml add miri, three sanitizers, cargo-careful, cargo-mutants sharded
six ways, cargo-minimal-versions, cargo-udeps, and cargo-bloat, run weekly and on a
`deep-check` PR label. [Testing Strategies](./patterns/testing-strategies.md) asks new projects for a fraction of this. The rustdoc gate
with private items ([Lints and Static Analysis](./patterns/lints-and-static-analysis.md) item 11, extras/ripgrep/.github/workflows/ci.yml docs job) is
in both the Makefile and ci.yml.

#### The security posture is broader than any single reference

.github/workflows/security.yml runs cargo-audit, cargo-deny split into a four-way matrix per
check (the split from extras/starship/.github/workflows/security-audit.yml), osv-scanner,
gitleaks, semgrep, trivy, a cargo-cyclonedx SBOM artifact, dependency-review on PRs, CodeQL,
and OpenSSF Scorecard, on a weekly cron plus every PR and push. deny.toml documents both
advisory ignores with reasons and removal conditions (the discipline of the [tauri study](./studies/tauri.md) item 3,
extras/tauri style), sets `wildcards = "deny"` and `unknown-registry = "deny"` as [Lints and Static Analysis](./patterns/lints-and-static-analysis.md)
item 9 asks, and bans openssl with a reason string.

#### Releases are automated end to end with provenance

.github/workflows/release.yml picks the next free patch version against crates.io, re-runs
fmt, clippy, tests, and `cargo package` before tagging, builds five targets including both
macOS architectures and aarch64 musl with `cargo auditable`, smoke-tests each artifact,
generates SHA256SUMS and a syft SBOM, signs with `actions/attest-build-provenance`, publishes
through an environment-gated crates.io job, and is idempotent when re-run. That covers the
[Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md) checklist items on tag-manifest agreement, draft-until-complete semantics,
checksums, attestation, and SBOM (extras/ripgrep/.github/workflows/release.yml,
extras/fd/.github/workflows/CICD.yml, extras/rustdesk/.github/workflows/flutter-build.yml)
in one workflow. install.sh and install.ps1 are themselves tested by tests/install.sh and
tests/install.ps1 on all three desktop OSes in ci.yml, which no reference repository does.

#### The command layer, exit discipline, and output contract are exemplary

`fn main() -> ExitCode` in src/main.rs, a typed `Failure { code, message, hint }` in
src/cli/mod.rs with named codes (1 failure, 3 not found, 4 unavailable, clap's own 2 for
usage), `ErrorKind::BrokenPipe` mapped to exit 0 in `cli::report`, and hint lines rendered
uniformly: that is the fd exit-code enum (extras/fd/src/exit_codes.rs), the ripgrep and bat
broken-pipe rule (extras/bat/src/error.rs), and the uv `Hint` pattern
(extras/uv/crates/uv-errors/src/lib.rs) all present. The `Emitter` in src/cli/mod.rs
guarantees one JSON document per invocation on a locked stdout, documented as a contract in
docs/cli/conventions.md. Destructive verbs (`discard`, `branch delete`, `stash drop`,
`stash clear`, `cherry-pick`, and `revert`) report what they would do and require `--yes`.
This is the alacritty dry-run-before-wet-run lesson
(extras/alacritty/alacritty/src/migrate/mod.rs) already built into the CLI surface.

#### Tests cover the process boundary

Inline tests cover real Git fixtures, terminal geometry, parser behavior, generations, and
the clap tree. tests/cli.rs additionally executes `CARGO_BIN_EXE_quinjet` with real argv and
captured stdout, stderr, and exit status. Its process fixture removes repository-affecting Git
environment variables, disables system configuration, and points global configuration at the
null device. It covers all five completion generators outside a repository, validates bash
output with `bash -n`, verifies nested man pages include their full command path and inherited
global options, and proves discard, cherry-pick, and revert preview before `--yes` performs the
mutation. .config/nextest.toml defines default and ci profiles with `fail-fast = false` and a
`slow-timeout` with `terminate-after`, matching the [Testing Strategies](./patterns/testing-strategies.md) nextest checklist.

#### Documentation and repository hygiene

docs/cli has a page per verb under branch/, changes/, pull-request/, remotes/, repository/,
and stash/, plus conventions.md documenting the exit-code table and the `--json` guarantee.
scripts/sync_wiki.py generates the GitHub wiki from docs/ and `--check` gates broken links in
hygiene.yml. ARCHITECTURE.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, structured
issue forms (.github/ISSUE_TEMPLATE/bug.yml, feature.yml), a PR template, CODEOWNERS, a
grouped weekly dependabot.yml for cargo and github-actions, a labeler, and a stale sweeper
all exist. .github/workflows/pr.yml enforces conventional PR titles, conventional commit
subjects, and linear history, the committed/semantic-pr pattern from
extras/clap/committed.toml. scripts/check_comments.py and scripts/check_secrets.py each ship
a `--selftest` and run in hygiene.yml, and a grep confines `Command::new` to src/git,
src/cli, and src/main.rs, which is a repo-specific checker in the spirit of [Lints and Static Analysis](./patterns/lints-and-static-analysis.md)
item 12.

### Part 2: The scoped CLI parity claim, verified

ARCHITECTURE.md scopes parity to user-visible repository and GitHub operations. The terminal's
focus, selection, scrolling, folding, filtering, cache indicators, and mouse capture are
presentation state, not operations that need verbs. Repository and GitHub data work goes
through the same `cli::Command` vocabulary and `cli::Session` as the command line. Browser
opening uses the shared `cli::open_url` helper after both faces resolve the same pull request
or check.

Mutating operations: every `GitOperation` variant maps to a verb in src/cli/mod.rs:
`Stage`/`StageAll` to `stage`, `Unstage`/`UnstageAll` to `unstage`, `Discard` to `discard`,
`Commit` to `commit` with `--amend`, `Fetch`/`Pull`/`Push`/`Sync` to their verbs, `Checkout`
to `branch switch`, `CreateBranch` to `branch create`, `RenameBranch` to `branch rename`,
`DeleteBranch` to `branch delete`, the five stash variants to `stash push`, `apply`, `pop`,
`drop`, and `clear`, `ResolveConflict` to `resolve --ours|--theirs` (with `--stage` mapping
to `Stage`), `CherryPick` to `cherry-pick`, and `Revert` to `revert`. Pressing `x` on a
conflict opens the resolution path; conflict discard is deliberately not a `GitOperation`.

Read operations: every query the worker issues in src/git/worker.rs has a verb. `Refresh`
is `status`, `LoadHistory` is `log`, `LoadBranches` is `branch list`, `LoadHistoryBranches`
is `branch list --all`, `LoadStashes` is `stash list`, `PrepareLocalDiff` and
`LoadLocalDiffFile` back `diff`, `show`, `branch compare`, and `stash show`,
`LoadGitHubRepositories` is `repos`, `LookupPullRequest` is `pr view`, `PreparePullRequest`
and `LoadPullRequestFile` back `pr files` and `pr diff`, `LoadPullRequestChecks` is
`pr checks`, `LoadPullRequestConversation` is `pr conversation`, and `LoadCheckRunLog` is
`pr logs`. Opening a pull request or selected check in a browser exists on both sides:
`AppEffect::OpenUrl` in src/main.rs and `pr open [--check <name>]` in src/cli/mod.rs share
`cli::open_url`.

Several `Command` variants are internal stages of an observable read rather than separate
operations. `PrepareLocalDiff` and `LocalDiffFile` compose local diff verbs,
`PreparePullRequest`, `PullRequestFile`, and `PullRequestFileBatch` compose `pr files` and
`pr diff`, and `WarmCheckRunLogs` prefetches the same logs exposed by `pr logs`. Metadata
verbs such as `completions` and `man`, and script-oriented output modes such as `--json`, are
intentionally command-line-only. The scoped parity claim holds.

### Part 3: Gap status, ordered by original priority

#### Resolved P0-1 (QJ-01): Terminal restoration across setup and panic paths

The original evidence came from the nushell, gitui, and meilisearch panic hooks. src/main.rs
now installs a hook before terminal entry and marks the terminal entered immediately after
raw mode succeeds. A rollback guard restores from that first successful mutation if any later
setup step fails. In release abort mode the hook restores from any thread because destructors
will not run. In unwind mode it restores only for a panic on the terminal-owning thread, so a
worker panic cannot tear down a terminal whose event loop is still running. Restoration is
idempotent across the rollback guard, terminal guard, and panic hook.

#### Resolved P0-2 (QJ-02): On-demand completions and man pages

This was the most uniform consensus in the corpus: tauri ships a
completions subcommand (extras/tauri/crates/tauri-cli/src/completions.rs), zed covers six
shells (extras/zed/crates/cli/src/completions.rs), fd generates completions and a man page
and installs them from its Makefile (extras/fd/src/cli.rs, extras/fd/Makefile), ripgrep
generates both from the binary, alacritty tests generated completions against checked-in
files (extras/alacritty/alacritty/src/cli.rs), starship, ruff, deno, and bat all ship
completions, and clap documents the mechanism itself (extras/clap/clap_mangen/Cargo.toml).
Quinjet now generates bash, zsh, fish, elvish, and PowerShell completions on demand. `man`
fully builds one clap tree and renders the root plus every nested command from it, preserving
full nested command paths and global options. Both verbs run outside a repository and install
nothing automatically. Process tests exercise all five generators, syntax-check bash with
`bash -n`, and verify nested manual output.

#### Resolved P0-3 (QJ-03): Black-box tests run the shipped binary

The original [Testing Strategies](./patterns/testing-strategies.md) evidence calls the
real-binary harness the backbone of CLI testing: extras/ripgrep/tests/util.rs drives the
compiled binary in scratch directories, extras/fd/tests/testenv/mod.rs locates it with
`env!("CARGO_BIN_EXE_fd")` and isolates the environment,
extras/bat/tests/utils/command.rs scrubs every relevant variable, and
extras/uv/crates/uv-test/src/lib.rs wraps insta so exit code, stdout, and stderr are pinned
together. tests/cli.rs now runs the shipped binary in scratch directories, isolates
repository-affecting Git environment and configuration, parses JSON output, tests destructive
previews and confirmations, covers all completion generators, validates bash syntax, and
checks root and nested manual pages.

#### P1-1 (QJ-04): Help text and the hand-written CLI reference can drift

docs/cli is written by hand and scripts/sync_wiki.py only checks links, so a flag added in
src/cli/mod.rs never fails CI when docs/cli misses it, and `--help` output is not snapshot
anywhere. The [Documentation Practices](./patterns/documentation-practices.md) checklist asks for `--help` snapshots asserted by a test and for
derivable docs to be drift-checked. Evidence: extras/bat/tests/integration_tests.rs asserts
`--help` against extras/bat/doc/long-help.txt with expect-test;
extras/clap/tests/ui/help_flag_stdout.toml pins help output as a trycmd case;
extras/uv/.github/workflows/check-generated-files.yml re-runs generators and fails on diff;
extras/starship/.github/workflows/workflow.yml does the same for the config schema. Fix: add
`trycmd` as a dev-dependency with `tests/ui/*.toml` cases pinning `--help` for the root and
every verb, and add one `#[test]` that walks `Cli::command().get_subcommands()` recursively
and asserts a matching page exists under docs/cli, so a new verb without documentation fails
the build.

#### Resolved P1-2 (QJ-05): Mutation route parity is machine-checked

Evidence: extras/rustdesk/src/core_main.rs keeps a test that the IPC-scoped CLI command set
matches the management commands exactly, and extras/ripgrep/crates/core/flags/defs.rs tests
the flag inventory exhaustively. src/cli/mod.rs now has one `operation_routes!` declaration
that generates both the exhaustive match and the route fixtures. Every `GitOperation` variant
has exactly one fixture, and every named route is resolved against the real clap tree. Adding
a variant without a route fails to compile; duplicating a variant fixture fails the test.

#### P1-3 (QJ-06): No changelog, and no mechanical release-notes discipline

There is no CHANGELOG.md; .github/workflows/release.yml relies on
`generate_release_notes: true`, which produces a raw PR list. The corpus is near-unanimous
that user-facing changes deserve a curated or structured changelog: the [Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md) checklist,
extras/alacritty/CHANGELOG.md with its legislated section order, extras/fd/CHANGELOG.md with
its permanent Unreleased section, extras/bat/.github/workflows/require-changelog-for-PRs.yml
enforcing entries, extras/gitui/.github/workflows/cd.yml extracting release notes, and
extras/clap/Cargo.toml `pre-release-replacements`. quinjet already enforces conventional
commits in .github/workflows/pr.yml, so the structured input exists. Fix: adopt git-cliff
with a committed cliff.toml, generate the release body from the tag range in release.yml in
place of `generate_release_notes`, and commit a generated CHANGELOG.md refreshed by the
release job.

#### Resolved P1-4 (QJ-07): cargo-binstall maps released targets

The original evidence was that extras/fd/Cargo.toml ships binstall metadata
with per-target overrides, extras/nushell/Cargo.toml and
extras/tauri/crates/tauri-cli/Cargo.toml do the same, and the same checklist calls for
it on day one. Cargo.toml now maps every currently released supported target to the existing
artifact names: x86-64 and AArch64 Linux GNU and musl triples, x86-64 and Apple Silicon macOS,
and x86-64 Windows with its `.exe` suffix.

#### P1-5 (QJ-08): The parsers of untrusted Git output have no property tests or fuzzing

src/git/status.rs (`parse_porcelain_v2`), src/git/diff.rs (`parse_diff`, `parse_numstat`),
and src/git/history.rs (`parse_log`) parse bytes that arbitrary repositories control:
branch names, paths, and commit subjects are attacker-influenced. They have example-based
tests but no property tests and no fuzz targets, and quinjet has no fuzz/ directory.
[Testing Strategies](./patterns/testing-strategies.md) reserves property testing for exactly this shape, and the corpus agrees:
extras/deno/runtime/permissions/lib.rs proptests ordering invariants,
extras/nushell/crates/nu-parser/fuzz/fuzz_targets/parse.rs and
extras/ripgrep/fuzz/fuzz_targets/fuzz_glob.rs keep three-line libfuzzer targets in
workspace-excluded packages, and extras/meilisearch/crates/filter-parser/fuzz treats parse
errors as success and panics only on internal errors. Fix: add proptest as a dev-dependency
with never-panics and round-trip properties for the three parser modules, and a fuzz/
package with its own `[workspace]` table (so the main lint wall and lockfile are untouched)
holding one target per parser, with `cargo check --manifest-path fuzz/Cargo.toml` in ci.yml.

#### P2-1 (QJ-09): Spawned git and gh processes have no time limit

`run_bounded_command` in src/git/github/mod.rs bounds output bytes but not wall time, so a
hung credential helper or a wedged `gh` blocks its worker lane forever (src/git/worker.rs).
Evidence: extras/starship/src/utils/mod.rs wraps every external command in `exec_timeout`
built on the process_control crate and degrades to a logged `None`. Fix: add
`process_control` and give `run_bounded_command` a `time_limit` with
`terminate_for_timeout`, surfacing the timeout as a normal `Failure` on the CLI and a toast
in the interface.

#### P2-2 (QJ-10): No .gitattributes

The repository has no .gitattributes, so line endings depend on each contributor's autocrlf
and no diff drivers are declared. Evidence: extras/helix/.gitattributes sets `* text=auto`
with per-extension diff drivers; extras/starship/.gitattributes forces `eol=lf` on files
whose bytes matter. Fix: commit a .gitattributes with `* text=auto eol=lf`, `*.rs diff=rust`,
`*.toml diff=toml`, and binary markers for any future image assets.

#### P2-3 (QJ-11): No .git-blame-ignore-revs

Formatting-only commits will eventually pollute blame. Evidence:
extras/zed/.git-blame-ignore-revs, honored automatically by GitHub. Fix: commit the file now
with a header explaining its use, and add revisions when a rustfmt or style migration lands.

#### P2-4 (QJ-12): Blank issues are still enabled

.github/ISSUE_TEMPLATE has bug.yml and feature.yml but no config.yml, so the forms can be
bypassed. Evidence: extras/rustdesk/.github/ISSUE_TEMPLATE/config.yml sets
`blank_issues_enabled: false` and routes questions to Discussions;
extras/fd/.github/ISSUE_TEMPLATE and extras/ripgrep's config.yml do the same. Fix: add
.github/ISSUE_TEMPLATE/config.yml with `blank_issues_enabled: false` and a contact link to
the repository's Discussions.

#### P2-5 (QJ-13): Almost no job sets timeout-minutes

Only .github/workflows/wiki.yml sets `timeout-minutes`; every other job inherits the 360
minute default, so a hung step burns six hours of runner time. Evidence: the [deno study](./studies/deno.md) item 8,
and extras/bevy/.github/workflows/ci.yml and extras/helix/.github/workflows/build.yml set
`timeout-minutes` on every job. Fix: add `timeout-minutes` (15 for lint-shaped jobs, 30 for
test and build jobs, 60 for mutants shards) across .github/workflows/.

#### P2-6 (QJ-14): Dependabot has no cooldown

.github/dependabot.yml updates weekly with grouping but proposes releases published minutes
earlier, which is the supply-chain window [Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md) warns about. Evidence:
extras/fd/.github/dependabot.yml and extras/bevy/.github/dependabot.yml set
`cooldown: default-days: 7`. Fix: add the `cooldown` block to both ecosystems in
.github/dependabot.yml.

#### P2-7 (QJ-15): `--version` carries no build metadata

There is no build.rs, so a dev build and a release build of 0.0.6 are indistinguishable in a
bug report. Evidence: extras/alacritty/alacritty/build.rs embeds the short commit hash into
the clap version string, extras/ripgrep/build.rs exposes it through `option_env!`, and
extras/gitui/build.rs honors `SOURCE_DATE_EPOCH` for reproducibility. Fix: add a build.rs
that runs `git rev-parse --short HEAD`, emits `cargo:rustc-env=QUINJET_BUILD_INFO=...`, and
wire `#[command(version = ...)]` in src/cli/mod.rs to include it. Note the lint wall: the
`[lints]` table applies to build scripts, so the `println!` directives need one scoped
`#[expect(clippy::print_stdout, reason = "cargo build-script directives")]`.

#### P2-8 (QJ-16): No bug-report subcommand

Evidence: extras/gitui/src/bug_report.rs assembles version, OS, and compile-time information
with the bugreport crate; extras/starship ships `starship bug-report`. For quinjet the same
verb would also report `git --version` and whether `gh` authenticates, the two facts every
issue needs. Fix: a `quinjet bug-report` verb using the `bugreport` crate, emitted through
the existing `Emitter` so `--json` works, and a link to it from
.github/ISSUE_TEMPLATE/bug.yml.

#### P2-9 (QJ-17): The subprocess-confinement rule lives in a workflow grep

hygiene.yml greps that `Command::new` appears only under src/git, src/cli, and src/main.rs.
That works but is invisible to local `cargo clippy` and editors. Evidence: [Deep Rust Language Idioms](./patterns/rust-language-idioms.md) closes
with "ban the hazards you have wrapped": extras/zed/clippy.toml bans
`std::process::Command::spawn` with a reason, and extras/starship/src/utils/mod.rs pairs the
ban with one sanctioned `#[allow]` site. Fix: add `std::process::Command::new` to
`disallowed-methods` in clippy.toml with a reason naming the sanctioned modules, put
`#[expect(clippy::disallowed_methods, reason = "...")]` on the few call sites in src/git and
src/cli, and keep or retire the grep.

#### P2-10 (QJ-18): No benchmarks anywhere

There is no benches/ directory and no hyperfine script, so diff-rendering and startup
regressions are invisible. Evidence: extras/bat/tests/benchmarks/run-benchmarks.sh measures
startup with hyperfine; extras/nushell/benches/benchmarks.rs uses tango for paired runs;
[Testing Strategies](./patterns/testing-strategies.md) recommends criterion or divan in-process plus hyperfine end-to-end. Fix: one
criterion bench over `parse_diff` and `parse_porcelain_v2` with `[[bench]] harness = false`,
plus a scripts/bench.sh wrapping hyperfine on `quinjet status` in a fixture repository,
reported in deep.yml rather than gating CI.

#### P2-11 (QJ-19): No third-party audit sharing via cargo-vet

cargo-deny, cargo-audit, osv, and dependency-review check advisories, but nothing asserts a
human audited the code of new dependencies. Evidence: extras/tauri/supply-chain/config.toml
imports the mozilla, google, and bytecode-alliance audit sets. Fix: `cargo vet init`, import
the same sets, commit supply-chain/, and add `cargo vet --locked` to security.yml. Optional
because quinjet's dependency tree is small and already tightly banned in deny.toml.

#### P2-12 (QJ-20): src/app.rs and src/ui/mod.rs are outsized

src/app.rs is 6,694 lines (about 4,865 before its inline test module) and src/ui/mod.rs is
6,006 lines (about 4,770 before tests). [Formatting and Style](./patterns/formatting-and-style.md) advises keeping production modules
roughly under 2,000 to 3,000 lines unless they are deliberate single-source registries, and
[Project and Workspace Structure](./patterns/project-structure.md) shows the seam-based split in extras/helix (the helix-term commands/ satellite
directory). These files are cohesive state machines, so this is polish, not damage. Fix:
split src/app.rs along its existing regions (palette, prompts and modals, pull-request
state, toasts) into an app/ directory, and src/ui/mod.rs into sidebar, content, and overlay
modules, one file per PR to stay under the diff budget, recording any formatting-only
commit in .git-blame-ignore-revs (QJ-11).

### Summary

| Priority | Resolved | Remaining |
| --- | --- | --- |
| P0 | terminal restoration; completions and man pages; black-box binary tests | none |
| P1 | mutation route parity; binstall metadata | help and docs drift gate; changelog discipline; parser property tests and fuzzing |
| P2 | none in this update | subprocess time limits; .gitattributes; .git-blame-ignore-revs; issue config.yml; job timeouts; dependabot cooldown; version build metadata; bug-report verb; disallowed-methods consolidation; benchmarks; cargo-vet; module splits |

The pattern across the corpus is clear: quinjet's static-analysis, CI, security, and release
machinery already exceed the eighteen reference repositories, often substantially. The
highest-risk process-boundary gaps from the original audit are now covered. Remaining work is
concentrated in documentation drift, release-note discipline, parser hardening, subprocess
timeouts, and optional repository polish.
