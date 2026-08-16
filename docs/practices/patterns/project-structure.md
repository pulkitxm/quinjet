# Project and Workspace Structure

How a repository is carved into crates, modules, and support directories is the first
architectural decision a Rust project makes, and it is the one that every later decision
(CI sharding, publishing, test layout, compile times) has to live with. This chapter
synthesizes the structural choices of eighteen mature repositories: rustdesk, tauri,
deno, uv, zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd,
nushell, tokio, gitui, and clap.

## 22.1 Consensus: what nearly every project does

Across all eighteen projects, a few practices are close to universal.

1. **A workspace appears the moment there is a second crate, and never later.**
   Fifteen of the eighteen repositories are Cargo workspaces. The three that are not
   (bat, starship, fd) are single-crate applications with exactly one `Cargo.toml`.
   Nobody runs sibling crates without a workspace, and nobody nests independent
   checkouts: the workspace is the unit of `Cargo.lock`, lint policy, and CI.

2. **Shared metadata is inherited, not repeated.** Workspaces use `[workspace.package]`
   for edition, license, rust-version, and repository, and `[workspace.dependencies]`
   for version pins. Even ripgrep, which is otherwise minimalist, inherits editions
   (extras/ripgrep/Cargo.toml):

   ```toml
   [workspace.package]
   edition = "2024"
   rust-version = "1.96"
   ```

   uv centralizes all 70+ internal crates as versioned path entries
   (extras/uv/Cargo.toml): `uv-cache = { version = "0.0.72", path = "crates/uv-cache" }`.

3. **Crate directory name equals crate name, with a project prefix.** `uv-*` under
   extras/uv/crates, `nu-*` under extras/nushell/crates, `helix-*` at the root of
   extras/helix, `bevy_*` under extras/bevy/crates, `tauri-*` under extras/tauri/crates.
   Nobody renames a directory away from its crate.

4. **Split boundaries follow capability, not size.** The reusable engine is a library
   crate and the product shell is a thin consumer: `alacritty_terminal` vs the
   `alacritty` GUI crate (extras/alacritty/Cargo.toml), `asyncgit` vs the gitui TUI
   (extras/gitui/Cargo.toml), the ten `grep`/`ignore`/`globset` crates under
   extras/ripgrep/crates vs the `rg` binary, `helix-core` vs `helix-term`.

5. **Special-needs packages are excluded from the workspace rather than contorted to
   fit.** Fuzz targets, nightly-only launchers, and consumer-simulation crates carry
   their own `[workspace]` table so `cargo test` at the root never touches them.
   ripgrep is explicit about it (extras/ripgrep/fuzz/Cargo.toml):

   ```toml
   # Prevent this from interfering with workspaces
   [workspace]
   members = ["."]
   ```

   uv excludes its Windows launcher with a one-line reason (extras/uv/Cargo.toml):

   ```toml
   members = ["crates/*"]
   exclude = [
     "scripts",
     # Needs nightly
     "crates/uv-trampoline",
   ]
   ```

   nushell nests standalone fuzz workspaces inside member crates
   (extras/nushell/crates/nu-parser/fuzz/Cargo.toml declares its own `[workspace]`),
   and bevy keeps a workspace-excluded consumer crate so integration tests use bevy
   exactly as a downstream user would.

6. **Test-only and dev-only crates are first-class members marked `publish = false`.**
   gitui ships `git2-testing` and `invalidstring` purely for tests
   (extras/gitui/git2-testing/Cargo.toml), tauri keeps integration fixtures in
   `crates/tests/restart` and `crates/tests/acl` (extras/tauri/Cargo.toml), tokio
   carries `tests-build` and `tests-integration` as members.

## 22.2 Single crate or workspace: the three camps

**Camp 1: single crate, no workspace.** bat, starship, and fd. All three are
end-user CLI tools whose internals nobody else consumes as separate pieces. fd is the
purest case: one manifest, no `lib.rs`, sixteen files plus three subsystem directories
(extras/fd/src: `exec/`, `filter/`, `fmt/`). starship scales the same shape to a much
larger program by leaning on module directories (extras/starship/src/modules holds one
file per prompt module) instead of crates. The reasoning: a crate boundary buys
compile-time parallelism and an enforced API, but costs version bookkeeping and
cross-crate refactoring friction. If no external consumer exists and the build is
fast enough, the boundary is pure cost.

**Camp 2: a root package that is also the workspace root.** rustdesk, ripgrep, bevy,
nushell, gitui, and clap keep `[package]` and `[workspace]` in the same root manifest.
The product lives at the top; support crates hang off it. ripgrep even keeps the
binary's source in a crate directory without its own manifest
(extras/ripgrep/Cargo.toml):

```toml
[[bin]]
bench = false
path = "crates/core/main.rs"
name = "rg"
```

The reasoning: `cargo run`, `cargo install --path .`, and `cargo test` at the repo
root operate on the product with zero flags, and the repository name stays the crate
name. bevy inverts the direction: the root package is a facade that re-exports
`bevy_internal`, so users depend on one crate while the engine is 90+ manifests
underneath (extras/bevy/Cargo.toml wires every feature through to `bevy_internal/...`).
clap does the same at library scale, with the facade exact-pinning its internals
(extras/clap/Cargo.toml): `clap_builder = { path = "./clap_builder", version = "=4.6.6", ... }`.

**Camp 3: a virtual workspace with no root package.** tauri, deno, uv, zed,
alacritty, meilisearch, ruff, helix, and tokio. The root manifest is coordination
only. The reasoning: with many peers there is no natural "main" crate to privilege,
and a root package muddies `cargo <cmd>` defaults and publishing. The ergonomics of
Camp 2 are recovered with `default-members`. helix (extras/helix/Cargo.toml):

```toml
default-members = [
  "helix-term"
]
```

zed does the same at 243 member crates: `default-members = ["crates/zed"]`
(extras/zed/Cargo.toml). The rule of thumb that falls out: application with one
obvious product, keep a root package; platform or library family, go virtual and
set `default-members` to the binary.

## 22.3 Where crates live, and policing the root

The dominant home is a `crates/` directory (tauri, uv, zed, meilisearch, ruff, bevy,
nushell, tokio's cousins aside). Smaller workspaces skip the extra level and put crate
directories at the root: helix (`helix-core/`, `helix-term/`, ...), tokio (`tokio/`,
`tokio-util/`, ...), alacritty, gitui, and clap. Two projects encode meaning into the
directory level itself:

```text
extras/rustdesk/                    extras/deno/
+-- src/          (orchestration)   +-- cli/       (the deno binary)
+-- libs/                           +-- runtime/   (JS runtime layer)
|   +-- scrap/    (capture)         +-- ext/       (op extensions)
|   +-- hbb_common/                 +-- libs/      (pure Rust libraries)
|   +-- enigo/    (input)           +-- tools/     (lint and CI scripts)
|   \-- clipboard/                  \-- tests/
\-- res/          (packaging)
```

rustdesk's capability crates sit under `libs/` with the thin product on top
(extras/rustdesk/Cargo.toml lists `members = ["libs/scrap", "libs/hbb_common", ...]`
and `exclude = ["vdi/host"]`). deno goes furthest: the four-layer split
(`cli > runtime > ext > libs`) is enforced by a lint that fails CI when anything new
appears at the root (extras/deno/tools/lint.js):

```js
// WARNING: When adding anything to this list it must be discussed!
// Keep the root of the repository clean.
const allowed = new Set([
  ".cargo", ".claude", ".devcontainer", ".github",
  "x", "cli", "doc", "ext", "libs", "runtime", "tests", "tools",
  ...
```

That is the strongest statement in the corpus that structure is an invariant to be
tested, not a convention to be remembered. bevy states the same intent as a comment
(extras/bevy/Cargo.toml): `# All of Bevy's official crates are within the "crates"
folder!` with globbed `members = ["crates/*", ...]`, then adds explicit entries for
compile-fail crates and mobile examples that need to be members individually.

## 22.4 Module tree conventions: mod.rs, named files, and crate-named roots

Counting directories under `src/` that have a sibling `<name>.rs` (named-file style)
versus `mod.rs` files gives a clear picture. The corpus has three positions.

**The mod.rs majority.** deno (74 mod.rs, 0 named), bevy (153/0), nushell (219/0),
alacritty (13/0), ripgrep (7/0), gitui (13/0), clap (17/0), fd (4/0), starship (8/0),
meilisearch (105/4), uv (23/4), tauri (50/9). The `mod.rs` file doubles as the
platform dispatch point, as in alacritty's PTY layer
(extras/alacritty/alacritty_terminal/src/tty/mod.rs):

```rust
#[cfg(not(windows))]
mod unix;
#[cfg(not(windows))]
pub use self::unix::*;

#[cfg(windows)]
pub mod windows;
```

**The named-file minority.** zed (118 named-file directories, 6 mod.rs) and helix
(15 named, 4 mod.rs) put the module's code in `foo.rs` beside a `foo/` directory of
submodules, so an editor tab strip never shows five files all called `mod.rs`. zed
extends the idea to the crate root itself: the lib target is named after the crate
instead of `lib.rs` (extras/zed/crates/editor/Cargo.toml):

```toml
[lib]
path = "src/editor.rs"
doctest = false
```

**The mixed middle.** ruff (153 mod.rs, 56 named), tokio (54/21), rustdesk (16/9),
and bat (2/3) carry both, usually because the convention changed midway. The lesson
from the mixed camp is negative: neither style wins on merit, but a repo that
contains both forces every reader to check which one applies. Pick one and state it.

## 22.5 Bin vs lib splits

Four shapes exist, in increasing order of separation:

1. **Binary only.** fd has `src/main.rs` and no library target at all
   (extras/fd/src). Integration tests run the compiled binary via
   `CARGO_BIN_EXE_fd`, so no library surface is needed.
2. **lib.rs plus main.rs in one crate.** starship keeps `src/lib.rs` and
   `src/main.rs` side by side (extras/starship/src) so integration code and the
   binary share one manifest.
3. **lib.rs plus a src/bin/ directory.** bat separates the reusable printing library
   from the application, giving the binary its own module tree
   (extras/bat/src/bin/bat holds `main.rs`, `app.rs`, `clap_app.rs`,
   `completions.rs`, ...). The library deliberately does not know about clap.
4. **Separate crates.** alacritty (GUI binary vs `alacritty_terminal`), gitui
   (TUI vs `asyncgit`), ripgrep (`rg` over ten library crates), helix
   (`helix-term` over `helix-core`, `helix-view`, `helix-tui`).

The graduation trigger is consumption: bat's library is used by other tools, and
alacritty's headless terminal library exists so golden ref tests can replay PTY
recordings without a window. When the split exists only to make the CLI parser
testable, shape 2 or 3 is enough; shape 4 pays off once a second consumer (tests,
another binary, external users) is real.

## 22.6 xtask and dev-tool crates

Repo-specific automation written in Rust, runnable through cargo, appears in half the
corpus, in three flavors:

- **A literal `xtask` crate plus a cargo alias.** helix
  (extras/helix/xtask/src has `docgen.rs`, `themelint`-style checks) wired through
  extras/helix/.cargo/config.toml:

  ```toml
  xtask = "run --package xtask --"
  ```

  meilisearch does the same with a release-profile twist
  (extras/meilisearch/.cargo/config.toml):
  `xtask = "run --release --package xtask --"`, with the crate at
  extras/meilisearch/crates/xtask driving benchmarks and declarative upgrade tests.
- **A tooling directory of purpose-named crates.** zed keeps `xtask`, custom dylint
  `lints`, `perf`, and `compliance` under extras/zed/tooling, so dev tools do not mix
  with product crates. bevy encodes its entire CI command set as a reviewable crate
  named `ci` (extras/bevy/tools/ci/Cargo.toml, `name = "ci"`), next to
  `build-templated-pages` and `example-showcase` under extras/bevy/tools.
- **Dev crates inside the normal crate tree.** ruff has `ruff_dev` behind a
  `cargo dev` alias and `ruff_benchmark` (extras/ruff/crates), uv has `uv-dev` and
  `uv-bench` (extras/uv/crates).

The alternative camp scripts everything outside cargo: deno's `./x` CLI and
extras/deno/tools JS scripts, rustdesk's `build.py`, nushell's `toolkit.nu`,
ripgrep's `ci/` shell scripts. The pattern within the pattern: projects whose tooling
must understand the workspace (codegen, docgen, invariant lints) put it in a crate;
projects whose tooling is packaging glue keep scripts.

## 22.7 Where large files get split, and where they deliberately do not

The corpus tolerates very large files when the file is a registry with one entry
shape repeated: ripgrep's flag table is 8,161 lines
(extras/ripgrep/crates/core/flags/defs.rs), one unit struct per flag with its unit
tests directly beneath each impl. Splitting it would scatter a table. The
surrounding directory shows the split that did happen: `flags/` holds `defs.rs`,
`parse.rs`, `hiargs.rs`, `lowargs.rs`, `complete/`, and `doc/`
(extras/ripgrep/crates/core/flags), separating flag definitions from parsing and
from documentation rendering.

helix takes the satellite-directory approach: `commands.rs` stays a 7,228-line
command table while cohesive subsystems break out into
extras/helix/helix-term/src/commands as `typed.rs`, `lsp.rs`, `dap.rs`, and
`syntax.rs`. zed accepts a 12,624-line `editor.rs`
(extras/zed/crates/editor/src/editor.rs) as the hub of a crate that splits everything
else into 100+ sibling files. The shared rule: split along subsystem seams
(fd's `exec/`, `filter/`, `fmt/` under extras/fd/src), never by line count alone,
and let a homogeneous table stay whole.

## 22.8 Examples and benches placement

Three placements for examples:

- **Root `examples/` as product documentation.** bevy registers 430 `[[example]]`
  targets in extras/bevy/Cargo.toml and gates them in CI; clap's root examples are
  executed by trycmd transcripts so they are docs that test.
- **A dedicated examples crate.** tokio makes `examples` a `publish = false`
  workspace member (extras/tokio/examples/Cargo.toml) with a comment telling copiers
  to swap the path dependency for a version.
- **Per-crate examples as API docs.** ripgrep keeps `simplegrep`-style examples in
  the library crates that own the API (extras/ripgrep/crates/grep/examples,
  extras/ripgrep/crates/searcher/examples, extras/ripgrep/crates/ignore/examples).

Benches mirror the ownership rule: a dedicated bench crate when benchmarks span the
workspace (`uv-bench`, `ruff_benchmark`, `clap_bench`, tokio's `benches` member,
listed under `# Internal` in extras/tokio/Cargo.toml), per-crate `benches/`
directories when the hot code is local (extras/meilisearch/crates/flatten-serde-json/benches,
extras/helix/helix-view/benches), and a root `[[bench]]` with `harness = false` for
single-crate-rooted workspaces (extras/nushell/Cargo.toml, `name = "benchmarks"`).
Two projects place benchmarks outside the repo entirely (fd's hyperfine suite,
alacritty's vtebench), keeping the workspace free of criterion dependencies.

## 22.9 Comparison table

Module style counts are measured as `mod.rs` files vs directories with a sibling
`<name>.rs` under any `src/` tree.

| Repository | Layout | Manifests | Crate homes | Module style (mod.rs/named) | Bin vs lib | Dev-tool crate | Examples and benches |
|---|---|---|---|---|---|---|---|
| rustdesk | root package + workspace | 9 | libs/ capability crates | mixed (16/9) | root bin over libs/ | none (build.py) | examples inside libs/scrap |
| tauri | virtual workspace | 26 | crates/, packages/ | mostly mod.rs (50/9) | tauri-cli bin, tauri lib | crates/tests/*, bench member | bench/ member crates |
| deno | virtual workspace | 87 | layered cli/ ext/ libs/ runtime/ | mod.rs (74/0) | cli bin over ext and libs | tools/ scripts + ./x | benches in leaf crates |
| uv | virtual workspace | 73 | crates/* glob | mostly mod.rs (23/4) | uv bin over uv-* libs | uv-dev, uv-bench | uv-bench crate |
| zed | virtual workspace | 259 | crates/ (243 members) | named-file (6/118), crate-named roots | zed bin, default-members | tooling/ (xtask, lints, perf) | benches in crates |
| ripgrep | root package + workspace | 12 | crates/, core has no manifest | mod.rs (7/0) | rg bin over 10 lib crates | ci/ scripts, excluded fuzz/ | per-lib-crate examples |
| alacritty | virtual workspace | 5 | top-level crate dirs | mod.rs (13/0) | GUI bin vs alacritty_terminal lib | none (Makefile) | external vtebench |
| bat | single crate | 1 | src/ + src/bin/bat/ | mixed (2/3) | lib.rs + src/bin/bat/ | build/ script dir | root examples/, hyperfine scripts |
| starship | single crate | 1 | src/ module dirs | mod.rs (8/0) | lib.rs + main.rs | none | timings subcommand, no benches |
| meilisearch | virtual workspace | 28 | crates/ | mod.rs (105/4) | meilisearch bin over milli | crates/xtask + alias | per-crate benches/ |
| ruff | virtual workspace | 53 | crates/ | mixed (153/56) | ruff bin over ruff_* libs | ruff_dev + cargo dev | ruff_benchmark crate, excluded fuzz/ |
| bevy | root facade + workspace | 92 | crates/* + tools/ | mod.rs (153/0) | bevy facade over bevy_internal | tools/ci crate | root examples/ (430), root benches/ |
| helix | virtual workspace | 15 | top-level helix-* dirs | named-file (4/15) | helix-term bin, default-members | xtask/ + alias | helix-view/benches |
| fd | single crate | 1 | src/ subsystem dirs | mod.rs (4/0) | binary only, no lib.rs | scripts/ | external hyperfine repo |
| nushell | root package + workspace | 46 | crates/nu-* | mod.rs (219/0) | nu bin over nu-* libs | toolkit.nu script | root [[bench]] harness=false |
| tokio | virtual workspace | 13 | top-level crate dirs | mostly mod.rs (54/21) | library family, no product bin | tests-build, tests-integration | examples and benches member crates |
| gitui | root package + workspace | 7 | top-level member dirs | mod.rs (13/0) | TUI bin vs asyncgit lib | git2-testing, invalidstring | none in repo |
| clap | root facade + workspace | 8 | top-level clap_* dirs | mod.rs (17/0) | facade over clap_builder | clap_bench member | root examples/ run by trycmd |

## 22.10 What a new Rust project should do

- [ ] Start as a single crate; do not pre-split. Add a crate only when a boundary has
      a second consumer, following bat and fd rather than a speculative workspace.
- [ ] The moment a second crate exists, create the workspace, set `resolver` explicitly,
      and move edition, rust-version, license, and shared versions into
      `[workspace.package]` and `[workspace.dependencies]`.
- [ ] For an application, keep a root package (or a virtual workspace with
      `default-members` pointing at the binary, as in extras/helix/Cargo.toml) so
      `cargo run` and `cargo install --path .` need no flags.
- [ ] Put member crates under `crates/` with directory name equal to crate name and a
      consistent project prefix; reserve extra directory levels (deno's cli/ext/libs)
      for enforced layering only.
- [ ] Pick one module style, named-file or mod.rs, write it down, and never mix.
      If large crates are expected, prefer named-file so tabs are distinguishable.
- [ ] Split bin from lib inside one crate first (`src/lib.rs` plus `src/bin/<name>/`
      like extras/bat/src/bin/bat); promote to a separate library crate only when
      headless tests or external consumers demand it, like alacritty_terminal.
- [ ] Add an `xtask` crate plus a `.cargo/config.toml` alias for docgen, codegen
      checks, and repo lints; keep pure packaging glue in scripts.
- [ ] Keep fuzz targets, nightly-only crates, and consumer-simulation tests in
      packages with their own `[workspace]` table, excluded from the main workspace,
      as in extras/ripgrep/fuzz/Cargo.toml.
- [ ] Mark every test-only and dev-only crate `publish = false` and give it a real
      manifest inside the workspace so it shares the lockfile and lint wall.
- [ ] Place examples in the crate whose API they demonstrate, and product-level
      examples at the root; give benchmarks a dedicated `publish = false` crate or
      per-crate `benches/`, never mixed into the product's dependency graph.
- [ ] Split large files along subsystem seams into a satellite directory
      (helix `commands/`), and let homogeneous registries stay as one file with
      their tests inline (ripgrep `flags/defs.rs`).
- [ ] Once the layout matters, enforce it: a CI check that fails on new top-level
      entries or misplaced crates, in the spirit of extras/deno/tools/lint.js.
