# Lints and Static Analysis

This chapter surveys how eighteen production Rust repositories (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap) configure clippy, rustc lints, `clippy.toml`, `cargo-deny` and friends, spell checkers, and their own custom check scripts. The single most important finding: the best projects treat lint configuration as executable architecture documentation, not as style policing. A `disallowed-methods` entry with a `reason` string is a design decision the compiler enforces forever.

## 1. Consensus practices

### 1.1 Zero warnings, enforced at CI rather than in source

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

### 1.2 `disallowed-methods` and `disallowed-types` as architecture enforcement

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

### 1.3 Every exception carries a written reason

Across all projects that suppress anything, the suppression is annotated: gitui's `deny.toml` links an upstream issue for every `skip-tree` entry, bevy's advisory ignores each cite the RUSTSEC page and the blocking dependency, tauri's `.cargo/audit.toml` explains each ignored advisory, and ruff enables `clippy::disallowed_methods` per crate with a machine-checked reason (extras/ruff/crates/ty_python_semantic/src/lib.rs):

```rust
#![warn(
    clippy::disallowed_methods,
    reason = "Prefer System trait methods over std methods in ty crates"
)]
```

bevy goes further and makes undocumented suppression itself a lint (extras/bevy/Cargo.toml): `allow_attributes = "warn"` and `allow_attributes_without_reason = "warn"`, and deno denies `clippy::allow_attributes_without_reason` on the CI command line (extras/deno/tools/lint.js, `clippyDenyFlags`).

### 1.4 Pinned lint toolchains

Because clippy adds lints every release, projects that gate merges on it pin the toolchain: tokio sets `rust_clippy: '1.88'` in extras/tokio/.github/workflows/ci.yml (line 22), meilisearch pins 1.91.1 with the clippy component in extras/meilisearch/rust-toolchain.toml, deno pins 1.95.0 with clippy in extras/deno/rust-toolchain.toml, gitui mirrors its MSRV into extras/gitui/.clippy.toml (`msrv = "1.88.0"`), and fd runs a second clippy pass on the exact MSRV toolchain (extras/fd/.github/workflows/CICD.yml line 81: "Run clippy (on minimum supported rust version to prevent warnings we can't fix)").

## 2. Divergent camps

### 2.1 Where the lint policy lives

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

### 2.2 Pedantic wall versus curated list versus defaults only

- Pedantic-on-by-default: uv and ruff set `pedantic = { level = "warn", priority = -2 }` then allow a documented list of exceptions (extras/uv/Cargo.toml, extras/ruff/Cargo.toml). ruff annotates individual allows, for example `needless_continue = "allow" # An explicit continue can be more readable`. gitui reaches the same end state via crate attributes, denying `all + perf + nursery + pedantic`.
- Curated warn list: clap hand-picks roughly sixty lints in `[workspace.lints.clippy]` (extras/clap/Cargo.toml), from `str_to_string` to `lossy_float_literal`, with inline notes such as `let_and_return = "allow"  # sometimes good to name what you are returning`.
- Defaults only: helix, fd, bat, meilisearch, alacritty run stock `clippy::all` and rely on `-D warnings`. helix holds 107k lines to only about 41 scoped `#[allow]` attributes with default lints; fd has exactly one clippy allow in all of src/ (extras/fd/src/walk.rs line 38, `#[allow(clippy::large_enum_variant)]`).

The pedantic camp argues the allow list is cheaper to maintain than reviewing for the same issues by hand. The defaults camp argues pedantic noise trains contributors to reach for `#[allow]`, and that a small clean codebase does not need it. Both camps produce near-zero suppression counts; what matters is that the choice is enforced, not which choice is made.

### 2.3 True `deny` levels: reserved for correctness classes

Where projects do put `deny` in configuration, it marks classes of bug, not style: nushell denies `unwrap_used` and `unchecked_time_subtraction` workspace-wide while merely warning on everything else (extras/nushell/Cargo.toml); zed denies `dbg_macro`, `todo`, `declare_interior_mutable_const`, `redundant_clone`, and `disallowed_methods` (extras/zed/Cargo.toml); bevy denies `unsafe_code` across the workspace with per-crate `expect(reason)` opt-ins (extras/bevy/Cargo.toml, `unsafe_code = "deny"`); bat denies `unsafe_code` at both crate roots (extras/bat/src/lib.rs line 22, extras/bat/src/bin/bat/main.rs line 1); gitui uses `forbid(unsafe_code)` so not even an allow can reopen the door.

### 2.4 Restriction lints for output hygiene

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

### 2.5 rustc lints: `unexpected_cfgs` as a custom-cfg registry

Projects with custom `--cfg` flags register them through the `unexpected_cfgs` lint so typos in cfg names fail the build: tokio lists nine (extras/tokio/Cargo.toml, `check-cfg = ['cfg(fuzzing)', 'cfg(loom)', ... 'cfg(tokio_unstable)']`), ruff registers `cfg(fuzzing)` and `cfg(codspeed)` (extras/ruff/Cargo.toml), and nushell registers `cfg(ci)` (extras/nushell/Cargo.toml). The other consensus rustc lints are `unreachable_pub = "warn"` (uv, ruff, clap), `unsafe_op_in_unsafe_fn` (clap, bevy), and `missing_docs` (bevy warns workspace-wide, ripgrep and gitui deny or forbid per crate, tauri warns via `#![warn(missing_docs, rust_2018_idioms)]` at extras/tauri/crates/tauri/src/lib.rs line 55).

### 2.6 `clippy.toml` tuning knobs beyond the bans

The corpus uses a wide set of secondary knobs: `msrv` and `cognitive-complexity-threshold = 18` (extras/gitui/.clippy.toml); `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-print-in-tests`, `allow-dbg-in-tests` so production restrictions do not poison test code (extras/clap/.clippy.toml, extras/nushell/clippy.toml); `doc-valid-idents` to stop `doc_markdown` mangling product names such as `"PyPI"` and `"PowerShell"` (extras/uv/clippy.toml, extras/ruff/clippy.toml, extras/bevy/clippy.toml, extras/clap/.clippy.toml); `ignore-interior-mutability` for false-positive `mutable_key_type` hits, each with a comment (extras/zed/clippy.toml, extras/ruff/clippy.toml); `avoid-breaking-exported-api = false` for pre-1.0 freedom (extras/zed/clippy.toml); `check-private-items = true` so doc lints reach private code (extras/bevy/clippy.toml); and `standard-macro-braces` to standardize `children![...]` (extras/bevy/clippy.toml).

### 2.7 Supply chain: four tiers of paranoia

1. Full cargo-deny: tokio, bevy, gitui, starship, clap. tokio's is the strictest license posture in the corpus (extras/tokio/deny.toml): `allow = ["MIT", "Apache-2.0"]` with a single `Unicode-3.0` exception for `unicode-ident`, plus `wildcards = "deny"` and `unknown-registry = "deny"` / `unknown-git = "deny"` under `[sources]`. gitui uniquely sets `multiple-versions = "deny"` and documents every `skip-tree` escape with the upstream issue that forces it (extras/gitui/deny.toml, for example `# currently needed due to: * dirs-sys v0.4.1 (https://github.com/dirs-dev/dirs-sys-rs/issues/29)`).
2. Advisory-only scanning: bat and tauri run cargo-audit with a versioned ignore file (extras/bat/.cargo/audit.toml, extras/tauri/.cargo/audit.toml, where every RUSTSEC id carries a comment such as `# proc-macro-error is unmaintained`); nushell runs rustsec via a dedicated workflow (extras/nushell/.github/workflows/audit.yml). starship splits its cargo-deny legs so a brand-new advisory cannot redden unrelated PRs (advisories run `continue-on-error`), invoked through a SHA-pinned action (extras/starship/.github/workflows/security-audit.yml line 27).
3. Human audit trails: tauri is the only project running cargo-vet, importing third-party audit sets so most dependencies arrive pre-audited (extras/tauri/supply-chain/config.toml: imports from bytecode-alliance, embark-studios, google, isrg, mozilla, zcash).
4. Nothing beyond a lockfile: ripgrep, alacritty, helix, fd, rustdesk, deno, meilisearch, uv, zed. These lean on exact pins, `--locked` builds, and update bots (Renovate or dependabot) instead of scanners. Several compensate with unused-dependency scanners: `cargo shear --deny-warnings` (extras/uv/.github/workflows/check-lint.yml line 196, extras/ruff/.github/workflows/ci.yaml line 830), cargo-machete with a metadata ignore list (zed), cargo-udeps on nightly (tauri, gitui).

### 2.8 Spell checkers

Nine projects check spelling mechanically. Eight use `typos` with a curated exception file: extras/uv/_typos.toml, extras/zed/typos.toml (a 118-line exclusion list where every entry says why, for example `# Contributor names aren't typos.`), extras/ruff/_typos.toml, extras/bevy/typos.toml, extras/nushell/typos.toml, extras/starship/typos.toml, extras/gitui/typos.toml, extras/clap/typos.toml. tokio alone uses cargo-spellcheck with a committed dictionary whose first line is a word count, validated for sortedness and uniqueness by a CI shell step (extras/tokio/spellcheck.dic, extras/tokio/.github/workflows/ci.yml around line 1263). The `typos` camp wins on setup cost; the tokio approach wins on documentation-heavy crates where API names dominate prose.

### 2.9 Custom static analysis where clippy cannot reach

The mature projects all grow at least one bespoke checker:

- zed writes real compiler plugins with dylint when a rule needs type information, pinned to their own nightly (extras/zed/tooling/lints/rust-toolchain.toml, `channel = "nightly-2026-03-21"`). Its `BLOCKING_IO_ON_FOREGROUND` lint flags `std::fs` calls inside functions holding a synchronous UI context (extras/zed/tooling/lints/src/blocking_io_on_foreground.rs).
- nushell uses ast-grep structural rules with autofixes, wired through extras/nushell/sgconfig.yml and snapshot-tested; a rule is ten lines of YAML (extras/nushell/ast-grep/rules/empty_if_branch.yml: `message: "An empty block if(-else) expression is confusing or a potential bug."`).
- uv runs hawk, its own public-API dead-code linter, as `cargo +1.97.1 hawk check --target-dir target/hawk -D warnings` with reasoned overrides in extras/uv/hawk.toml (`[[override]] lint = "hawk::unnecessary_public"`).
- helix's xtask implements domain lints no general tool could express: `query-check`, `indent-check`, `highlight-check`, `theme-check`, and `docgen` drift detection (extras/helix/xtask/src/main.rs).
- deno's tools/lint.js layers repo-structure checks (the clippy.toml completeness audit, copyright headers, unreferenced expectation files) on top of clippy.
- uv, ruff, bevy, and zed also lint their CI itself with zizmor and actionlint (extras/uv/.github/workflows/check-zizmor.yml, extras/bevy CodeQL actions coverage).

## 3. Comparison table

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

## 4. What a new Rust project should do

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
