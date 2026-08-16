# sharkdp/bat (60188 stars)

## 1. What the project is and why it matters

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

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

## 5. Linting

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

## 6. CI/CD

There are exactly two workflows in extras/bat/.github/workflows: CICD.yml (464 lines, the whole
pipeline) and require-changelog-for-PRs.yml (33 lines).

### CICD.yml

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

### require-changelog-for-PRs.yml

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

### Dependabot

extras/bat/.github/dependabot.yml updates three ecosystems monthly on the same schedule: cargo,
gitsubmodule (the 93 syntax/theme submodules), and github-actions. Dependabot PRs are auto-merged
when CI passes, which is exactly why the changelog gate excludes them and the release checklist
reminds maintainers to backfill their entries.

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage: ten cited idioms

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

## 10. Documentation practices

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

## 11. Release and distribution

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

## 12. Lessons for quinjet

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
