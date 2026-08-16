# Error Handling and API Design

This chapter synthesizes how eighteen production Rust codebases (rustdesk, tauri, deno, uv,
zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio,
gitui, clap) handle errors and shape their public APIs: library choice (anyhow, thiserror,
hand-rolled), context discipline, exit-code taxonomies, panic and unwrap policy, and the
API-hardening toolkit of builders, newtypes, sealed traits, visibility rules, and `must_use`.

## Consensus practices

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

## Divergent camps

### anyhow vs thiserror vs hand-rolled

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

### Exit-code taxonomies

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

### Panic policy and unwrap discipline

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

## Comparison table

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

## Exemplary excerpts: the API-hardening toolkit

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

## What a new Rust project should do

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
