# Deep Rust Language Idioms

Tooling chapters describe what surrounds the code. This chapter is about the code itself: how
eighteen mature Rust projects (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat, starship,
meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap) actually use the language. The
dimensions examined are iterator pipelines, zero-copy data flow and `Cow`, borrowing and lifetimes
in public APIs, trait design and generics, interior mutability selection, concurrency primitives,
macro usage, unsafe policy and its documentation, and `cfg`-based platform handling. The projects
disagree loudly about runtimes and unsafe budgets, but they converge on a surprisingly small set of
shared idioms, and the convergence is where the transferable lessons live.

## Consensus practices

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

## Divergent camps

### Concurrency model: async runtime, thread pool, or plain threads

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

### Interior mutability: pick the cheapest cell that survives your threading model

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

### Iterator style: pull pipelines versus the push model

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

### Unsafe budget: zero, minimal, or industrial

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

## Comparison across the eighteen repositories

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

## Exemplary excerpts

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

## What a new Rust project should do

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
