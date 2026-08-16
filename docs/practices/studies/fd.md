# sharkdp/fd (44095 stars)

## 1. What the project is and why it matters

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

extras/fd/rustfmt.toml is a single line:

```toml
# Defaults are used
```

That one line is a deliberate practice, not an omission. The file exists so that editors and CI agree there is a rustfmt configuration, and its content documents the policy: stock rustfmt, no overrides, no style debates. CI enforces it with `cargo fmt -- --check` (the `ensure_cargo_fmt` job in extras/fd/.github/workflows/CICD.yml).

There is no `.editorconfig` and no formatter for YAML or Markdown; the non-Rust surface is small enough that review covers it. The only formatting-adjacent config for non-Rust files is extras/fd/doc/.gitattributes, which marks the generated screencast as vendored so it does not pollute language statistics:

```text
* linguist-vendored
```

## 5. Linting

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

## 6. CI/CD

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

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage

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

## 10. Documentation practices

- Rustdoc is used for maintainers, not for docs.rs (a binary crate has no API consumers): every `Config` field has a `///` line (extras/fd/src/config.rs), non-obvious functions carry doc comments that explain rationale and link issues (the 20-line comment on `ensure_search_pattern_is_not_a_path` in extras/fd/src/main.rs reads like a design note), and extras/fd/src/sanitize.rs opens with a `//!` module doc: "TTY-output sanitization to prevent terminal escape injection via filenames."
- The user manual is the hand-maintained man page extras/fd/doc/fd.1 (587 lines of roff) plus a 800-line README with a troubleshooting section that the bug-report template points at. The README's help output is kept honest mechanically: extras/fd/scripts/update-help.awk re-runs `cargo run --release --quiet -- -h` and splices the result into the README's fenced block.
- extras/fd/CONTRIBUTING.md sets pull-request expectations, requires an entry in the "Upcoming release" section of the changelog with the exact format `- Short description of what has been changed, see #123 (@user)`, and asks contributors to open an issue before a PR.
- extras/fd/SECURITY.md defines a private vulnerability-reporting path via GitHub advisories with explicit confidentiality expectations.
- Issue templates: extras/fd/.github/ISSUE_TEMPLATE/bug_report.yaml is a structured GitHub form with a required checkbox ("I have read the troubleshooting section and still think this is a bug"), a required version input, and a required OS textarea rendered as shell; feature requests and questions get lighter Markdown templates, and config.yml keeps blank issues enabled. There is no PR template; CONTRIBUTING carries that weight.
- There is no ARCHITECTURE.md; at 5k lines the module names and doc comments are the architecture document.

## 11. Release and distribution

Versioning is semver on the crate (`10.4.2` at extras/fd/Cargo.toml), tags are `vX.Y.Z`, and the changelog is the source of release notes. extras/fd/CHANGELOG.md keeps a permanent `# Unreleased` section with `## Features`, `## Bugfixes`, `## Changes`, `## Other` subsections; every entry credits the contributor and cites the issue or PR number. MSRV bumps are announced as changelog entries ("Minimum required rust version has been increased to 1.90.0").

The release process is a documented checklist plus scripts:

- extras/fd/doc/release-checklist.md is a copy-pasteable checklist covering version bump, README/MSRV sync, `cargo publish --dry-run`, tagging, verifying binary deployment, and post-release changelog scaffolding.
- extras/fd/scripts/version-bump.sh automates the mechanical part: creates a `release-$version` branch, seds the version into Cargo.toml, updates the MSRV note in the README, and renames the changelog heading.
- Pushing the tag triggers the CD half of extras/fd/.github/workflows/CICD.yml: 14 target archives (tar.gz/zip with binary, man page, completions, licenses, changelog inside), Debian packages from extras/fd/scripts/create-deb.sh (including musl variants with correct `Conflicts:` metadata and `fdfind` alias symlinks for Debian), provenance attestation via actions/attest, upload to the GitHub release, and winget publication.

Completions and man page distribution is handled by the extras/fd/Makefile: generated completions come from the binary itself (`$(EXE) --gen-completions bash > $@`), the zsh completion is a hand-written file copied from extras/fd/contrib/completion/_fd, and `make install` places binary, completions for bash/fish/zsh, and the man page into FHS paths. Runtime generation is also a user feature: `fd --gen-completions <shell>` works on any installed binary because clap_complete ships in the default `completions` feature. Finally, the binstall metadata in Cargo.toml (section 3) makes `cargo binstall` a first-class install path.

## 12. Lessons for quinjet

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
