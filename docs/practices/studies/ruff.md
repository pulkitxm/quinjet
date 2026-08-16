# astral-sh/ruff (49222 stars)

## 1. What the project is and how big it is

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

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

## 5. Linting

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

## 6. CI/CD

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

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage: ten cited idioms

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

## 10. Documentation practices

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

## 11. Release and distribution

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

## 12. Lessons for quinjet

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
