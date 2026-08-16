# alacritty/alacritty (65390 stars)

## 1. What the project is and why it matters

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

## 2. Repository layout

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

## 3. Cargo manifest practices

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

## 4. Formatting

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

## 5. Linting

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

## 6. CI/CD

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

## 7. Testing

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

## 8. Error handling and API design

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

## 9. Deep Rust usage: ten cited idioms

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

## 10. Documentation practices

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

## 11. Release and distribution

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

## 12. Lessons for quinjet

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
