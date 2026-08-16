# denoland/deno (108251 stars)

## 1. What the project is and what the clone measures

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

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

## 5. Linting

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

## 6. CI/CD

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

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage: ten-plus cited idioms

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

## 10. Documentation practices

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

## 11. Release and distribution

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

## 12. Lessons for quinjet

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
