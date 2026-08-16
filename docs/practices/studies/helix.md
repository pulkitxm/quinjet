# helix-editor/helix (45833 stars)

## 1. What Helix is and why it matters

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

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

## 5. Linting

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

## 6. CI/CD

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

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage, ten cited examples

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

## 10. Documentation practices

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

## 11. Release and distribution

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

## 12. Lessons for quinjet

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
