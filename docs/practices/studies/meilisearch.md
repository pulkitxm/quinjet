# meilisearch/meilisearch (58979 stars)

## 1. What the project is and how big it is

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

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

## 5. Linting

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

## 6. CI/CD

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

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage: ten-plus cited idioms

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

## 10. Documentation practices

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

## 11. Release and distribution

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

## 12. Lessons for quinjet

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
