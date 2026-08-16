# Testing Strategies

Testing is where the eighteen projects studied in this reference diverge the most in
mechanism and converge the most in intent. Every one of them treats the compiled
artifact, not the unit, as the thing that must be proven correct: CLI tools run their
real binaries, TUIs drive their real event loops, and libraries run their public API
from separate integration crates. This chapter covers integration test layout under
`tests/`, end-to-end CLI harnesses, snapshot testing, property testing, fuzzing,
benchmarking, test-support crates, coverage, and test runners.

## 23.1 Consensus practices

Nearly all eighteen projects share the following habits.

**Test the real artifact, not a simulation.** fd resolves the binary Cargo just built
via the `CARGO_BIN_EXE_<name>` environment variable
(extras/fd/tests/testenv/mod.rs):

```rust
/// Find the *fd* executable.
fn find_fd_exe() -> PathBuf {
    // Read the location of the fd executable from the environment
    PathBuf::from(env::var("CARGO_BIN_EXE_fd").unwrap_or(env!("CARGO_BIN_EXE_fd").to_string()))
}
```

uv does the same through its `uv-test` support crate, bat through an `assert_cmd`
command factory (extras/bat/tests/utils/command.rs), ripgrep through a
`Dir`/`TestCommand` pair (extras/ripgrep/tests/util.rs), and meilisearch by booting
its actual server entry point in a `TempDir`. Even the GUI-shaped projects follow the
principle: helix drives the real `Application` event loop with synthetic key events
(extras/helix/helix-term/tests/test/commands/write.rs), and gitui draws the real app
struct into a ratatui `TestBackend`.

**Isolate every test in a throwaway filesystem.** `tempfile::TempDir` fixtures appear
in ripgrep, fd, bat, uv, tauri, meilisearch, nushell (its `Playground` type), and
helix. Tests that touch global state are serialized rather than deleted: bat uses
`serial_test`, ruff and uv declare nextest `test-groups` with `max-threads = 1`.

**Failures must explain themselves.** ripgrep's `eqnice!` macro prints a framed
expected/got block (extras/ripgrep/tests/macros.rs); fd renders a line diff via the
`diff` crate (extras/fd/tests/testenv/mod.rs, `format_output_error`); snapshot tools
(insta, expect-test, trycmd) produce reviewable diffs by construction.

**Test-support code is a first-class deliverable.** Twelve of the eighteen ship a
dedicated support crate or module: uv's `uv-test` (extras/uv/crates/uv-test/src/lib.rs),
deno's `tests/util` SDK with a PTY driver and mock registry farm, tokio's published
`tokio-test` crate, gitui's `git2-testing` and `invalidstring` helper crates,
meilisearch's `meili-snap` (extras/meilisearch/crates/meili-snap/src/lib.rs), zed's
800+ `test-support` cargo feature references, starship's `ModuleRenderer` builder with
864 call sites (extras/starship/src/test/mod.rs), and nushell's `Playground`.

**Determinism is engineered, not hoped for.** starship ships deterministic git bundle
fixtures with a shared `TEST_GIT_CONFIG` setting `core.fsync=all` to kill Windows
flakes; gitui sandboxes global git config via `git2` `set_search_path` inside a `Once`
so the host machine cannot leak into tests; fd installs a `cfg(test)` thread-local
clock in `src/filter/time.rs`; zed's `#[gpui::test]` macro replays seeded async
schedules so every test doubles as a concurrency fuzzer.

## 23.2 Divergent camps

### Layout: `tests/` directory versus inline `cfg(test)` modules

The single biggest split is where tests live.

**Camp A: integration tests dominate, under `tests/`.** ripgrep, fd, bat, uv, clap,
tokio, deno, meilisearch, helix. ripgrep goes furthest: `autotests = false` in
extras/ripgrep/Cargo.toml forces every test into one binary:

```text
extras/ripgrep/tests/
|-- tests.rs        (the single registered test binary)
|-- macros.rs       (rgtest! and eqnice!)
|-- util.rs         (Dir + TestCommand harness)
|-- binary.rs  feature.rs  json.rs  misc.rs  multiline.rs
`-- regression.rs   (1,744 lines of issue-numbered tests)
```

The reasoning: one binary links once (integration test link time is the dominant test
cost), and black-box tests survive refactors that inline tests do not. clap's
integration tree outweighs its core crate at 30,959 lines, wired together with
`automod::dir!` for the same one-binary linking benefit. uv is the extreme case, with
roughly 229k lines under extras/uv/crates/uv/tests/ split into parallel binaries
(`it/`, `pip/`, `sync/`, `lock/`, ...).

**Camp B: inline `cfg(test)` only, no `tests/` directory at all.** starship (1,302
tests in 114 inline modules), gitui (318 colocated tests), rustdesk (202 tests beside
platform-gated code), zed (mostly per-crate `*_tests.rs` files inside `src/`). The
reasoning: these codebases test through an in-process harness (`ModuleRenderer`, the
`Gitui` struct, `gpui::TestAppContext`) rather than a spawned binary, so the
white-box access of an inline module is the point, and there is no link-time tax to
amortize.

**Camp C: replace libtest entirely.** nushell registers a single test binary with the
default harness disabled (extras/nushell/Cargo.toml):

```toml
autotests = false

[[test]]
name = "tests"
path = "tests/main.rs"
harness = false
```

Its kitest-plus-linkme harness adds `#[serial]`, `#[env(...)]`, `#[exp(...)]`, and
`#[deps(NU)]` attributes that libtest cannot express. deno's spec suite is likewise a
`harness = false` binary run by `file_test_runner` over 2,087 `__test__.jsonc`
manifests (counted under extras/deno/tests/specs/). The reasoning: when the test
language itself is data (JSONC manifests, transcript files), a custom runner buys
flaky tracking, sharding, and manifest linting that libtest cannot provide.

### CLI end-to-end harnesses: hand-rolled, assert_cmd, or transcript-driven

Three styles coexist. ripgrep and fd hand-roll a process harness because they predate
the ecosystem crates and want total control of diff output. bat, uv, and ruff build
on `assert_cmd` (bat directly, uv via `uv-test`, ruff via `insta_cmd` which wraps it).
clap owns the third style: `trycmd` replays committed TOML and Markdown transcripts
against compiled example binaries (extras/clap/tests/ui.rs):

```rust
let t = trycmd::TestCases::new();
t.register_bins(trycmd::cargo::compile_examples(["--features", &features]).unwrap());
t.case("tests/ui/*.toml");
```

A transcript fixture is plain data (extras/clap/tests/ui/help_flag_stdout.toml):

```toml
bin.name = "stdio-fixture"
args = ["--help"]
status.code = 0
stdout = """
Usage: stdio-fixture[EXE] [OPTIONS] [NAME] [ENV] [COMMAND]
...
```

The transcript camp argues the fixtures double as documentation and are editable by
non-Rust contributors; the `assert_cmd` camp argues Rust-side assertions compose
better with fixtures and filters. deno's `__test__.jsonc` spec manifests
(`extras/deno/tests/specs/add/no_save/__test__.jsonc`, with `"tempDir": true` and
per-step `args`/`output` pairs) are the same idea scaled to two thousand scenarios,
matched by a custom wildcard language (`[WILDCARD]`, `[WILDLINE]`,
`[UNORDERED_START]`) implemented in extras/deno/tests/util/lib/wildcard.rs.

For terminal-real testing, bat opens actual PTYs via `nix::pty::openpty` with
`wait-timeout` hang protection, deno drives its REPL through a portable-pty wrapper
(extras/deno/tests/util/lib/pty.rs), and clap's `completest-pty` types into real
bash, zsh, fish, elvish, and nushell shells installed in CI.

### Snapshot testing: insta, expect-test, golden files, or nothing

insta is the plurality choice: uv (6,430 `uv_snapshot!` call sites, counted in
extras/uv/crates/), ruff (84 `snapshots/` directories plus full CLI snapshots), gitui,
tauri (with per-platform `Settings::set_snapshot_path`), and meilisearch. The uv
macro shows the pattern of wrapping insta once per project
(extras/uv/crates/uv-test/src/lib.rs):

```rust
macro_rules! uv_snapshot {
    ($spawnable:expr, @$snapshot:literal) => {{
        uv_snapshot!($crate::INSTA_FILTERS.to_vec(), $spawnable, @$snapshot)
    }};
    ($filters:expr, $spawnable:expr, @$snapshot:literal) => {{
        let (snapshot, output) = $crate::run_and_format($spawnable, &$filters,
            $crate::function_name!(), Some($crate::WindowsFilters::Platform), None);
        ::insta::assert_snapshot!(snapshot, @$snapshot);
        output
    }};
```

Filters normalize paths, timings, and Windows-only diffs so one snapshot serves all
platforms; the same technique appears in gitui (extras/gitui/src/gitui.rs uses
`insta::Settings` `add_filter` to redact temp paths and commit hashes before
`assert_snapshot!("app_loading", terminal.backend())` on a
`Terminal::new(TestBackend::new(90, 12))`). ruff hard-gates hygiene: CI runs
`cargo insta test --all-features --unreferenced reject` so orphaned snapshot files
fail the build (extras/ruff/.github/workflows/ci.yaml, line 386).

meilisearch dissents on snapshot size: `meili-snap` stores only inline md5 hashes and
writes full snapshots to disk only when `MEILI_TEST_FULL_SNAPS=true`
(extras/meilisearch/crates/meili-snap/src/lib.rs), trading reviewability for a diff
that never drowns a PR. bat prefers `expect-test`, snapshotting `--help` into
committed `doc/*.txt` files via `expect_test::expect_file!`
(extras/bat/tests/integration_tests.rs, `fn test_help`), plus a `snapshot_tests!`
macro generating 26 style permutations against a programmatically built git repo
(extras/bat/tests/snapshot_tests.rs). clap snapshots styled help output as
reviewable SVG through snapbox's `term-svg` feature (extras/clap/Cargo.toml,
`snapbox = { version = "1.2.0", features = ["term-svg"] }`).

The oldest camp uses raw golden files with hand-rolled replay: alacritty's 45
recorded PTY sessions are diffed grid cell by grid cell through a declarative macro
(extras/alacritty/alacritty_terminal/tests/ref.rs):

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

fd, starship, and rustdesk use no snapshot tooling at all, on the argument that
their outputs are short enough for literal assertions with diff-formatted failures.

### Property testing and fuzzing: safety-critical parsers only

No project property-tests everything. The pattern is surgical: proptest or quickcheck
guards algebraic invariants (deno proves `Ord` transitivity and deny-precedence of
its permission system; helix round-trips diff apply/revert via `quickcheck`
(extras/helix/helix-core/Cargo.toml); tauri runs proptest at 10,000 cases on event
listener keys; zed writes a custom `Arbitrary` for its `SumTree`), while cargo-fuzz
covers parsers of untrusted input. Fuzz crates are consistently workspace-excluded
packages: extras/ripgrep/fuzz/fuzz_targets/fuzz_glob.rs asserts glob round-trip
properties, extras/ruff/fuzz/fuzz_targets/ holds six targets including
`ruff_parse_idempotency.rs` and `ruff_formatter_idempotency.rs`, deno fuzzes npm
packument parsing under extras/deno/libs/npm/fuzz/, nushell fuzzes `nu-parser` and
`nu-path`, and meilisearch runs a stateful indexing fuzzer for up to 72 hours on
pushes to main. tokio layers loom model checking on top
(extras/tokio/tokio/Cargo.toml, `[target.'cfg(loom)'.dev-dependencies]`), which no
other project needs because no other project hand-writes a scheduler. Six projects
(fd, bat, starship, gitui, alacritty, rustdesk) ship no fuzzing at all; all six parse
comparatively little untrusted input or delegate parsing to fuzzed dependencies.

### Benchmarks: criterion, divan, and the continuous-benchmark split

criterion remains the default (bevy at extras/bevy/benches/Cargo.toml pinning
`criterion = { version = "0.8.0", features = ["html_reports"] }` with
`autobenches = false`; tokio at extras/tokio/benches/Cargo.toml; helix behind a
`bench` feature; meilisearch and zed in leaf crates). clap chose divan for its lower
boilerplate, naming benches after real CLIs (extras/clap/clap_bench/benches/ripgrep.rs,
rustup.rs). The continuous camp wires benches to a tracking service: uv and ruff use
the codspeed-criterion-compat shim (extras/uv/crates/uv-bench/Cargo.toml renames the
`criterion` dependency to `codspeed-criterion-compat`), nushell uses tango-bench for
paired runs, and deno benchmarks the release binary with wrk and hyperfine, publishing
to a gh-pages site. fd deliberately keeps benchmarks in a separate hyperfine repo,
and bat commits hyperfine scripts under extras/bat/tests/benchmarks/: macro-level
CLI latency is better measured by an external process timer than by in-process
criterion loops.

### Runners and coverage

cargo-nextest has majority momentum among the large workspaces: uv
(extras/uv/.config/nextest.toml defines `profile.ci` with `fail-fast = false`,
JUnit output, and per-OS inherited profiles), ruff (extras/ruff/.config/nextest.toml
turns deadlocks into failures with `slow-timeout = { period = "1s", terminate-after
= 60 }` and a `serial` test group for its file watcher), zed
(extras/zed/.config/nextest.toml uses `priority` overrides to run the slowest tests
first), plus tokio and gitui installing it via `taiki-e/install-action` in CI.
Projects on plain libtest (ripgrep, fd, bat, alacritty, clap, starship) are exactly
the ones with a single test binary, where nextest's per-test process isolation and
scheduling buy little.

Coverage as a gate is rare. Only starship runs `cargo llvm-cov --all-features
--locked --workspace --lcov -- --include-ignored` in CI
(extras/starship/.github/workflows/workflow.yml) and nushell scripts it through
`cargo llvm-cov show-env` in extras/nushell/toolkit/coverage.nu; neither enforces a
numeric threshold. The other sixteen enforce behavior directly (snapshots, invariant
tests, fuzzing) rather than a percentage, a deliberate stance that coverage numbers
reward line execution, not assertion quality.

## 23.3 Comparison across the eighteen repositories

| Repository | Integration layout | CLI / E2E harness | Snapshots | Property / fuzz | Benchmarks | Runner |
|---|---|---|---|---|---|---|
| rustdesk | inline `cfg(test)` only | runnable examples as manual harnesses | none | none | example binaries | libtest, `--skip` by name in CI |
| tauri | `crates/tests` + inline | `MockRuntime` headless IPC (1,413 lines) | insta, per-platform paths | proptest 10k cases, quickcheck | custom strace harness | libtest |
| deno | `tests/specs` golden manifests | `file_test_runner` + PTY driver | `.out` golden files + wildcard language | proptest + cargo-fuzz | wrk/hyperfine on release binary | custom `harness = false` |
| uv | `crates/uv/tests` (~229k lines) | `uv-test` + assert_cmd + `uv_snapshot!` | insta with filters, 6,430 sites | test-feature gating, no fuzz | codspeed-criterion | nextest ci profiles |
| zed | inline per-crate test files | `#[gpui::test]` seeded executors | limited | proptest + seeded scheduling | criterion + hyperfine perf | nextest with priorities |
| ripgrep | single binary, `autotests = false` | `Dir`/`TestCommand`, `rgtest!` per engine | golden diffs via `eqnice!` | cargo-fuzz `fuzz_glob` | globset benches + benchsuite | libtest |
| alacritty | `alacritty_terminal/tests/ref` | headless `Term` replay | 45 golden ref fixtures | none | external vtebench | libtest |
| bat | `tests/integration_tests.rs` (4,644 lines) | assert_cmd factory + real PTY (nix) | expect-test help files + style matrix | none | hyperfine scripts | libtest + serial_test |
| starship | inline only, no `tests/` | `ModuleRenderer` (864 sites) | none | none | timings subcommand | libtest + llvm-cov |
| meilisearch | per-crate + real HTTP server | typestate `Server<Owned>/<Shared>` | meili-snap md5-hashed insta | 4 fuzz crates + stateful fuzzer | criterion + span dashboard | libtest |
| ruff | `crates/ruff/tests` + inline | insta-cmd `assert_cmd_snapshot!` | 3,703 insta, unreferenced rejected | 6 libFuzzer + differential | CodSpeed crate | nextest ci profile |
| bevy | `tests/` tutorials + excluded consumer crate | example-run with RON configs | Pixel Eagle screenshots | Miri, ui_test compile-fail | criterion, `autobenches = false` | libtest + ui_test |
| helix | `helix-term/tests/integration.rs` | `AppBuilder` key-sequence DSL | none | quickcheck round-trips | criterion behind `bench` feature | libtest, `integration` profile |
| fd | `tests/tests.rs` + `testenv/` | `TestEnv` via `CARGO_BIN_EXE_fd` | none, diff-based literals | none | external hyperfine repo | libtest + test-case |
| nushell | one `harness = false` binary | in-process `NuTester` + `Playground` | ast-grep rule snapshots | quickcheck + cargo-fuzz | tango-bench paired | kitest + linkme |
| tokio | `tokio/tests`, 174 area files | `tokio-test` mocks + trybuild UI tests | `.stderr` snapshots | proptest + loom + fuzz | criterion `benches/` | nextest |
| gitui | inline only | full-app ratatui `TestBackend` | insta with redaction filters | none | flamegraph feature | nextest |
| clap | `tests/` + transcript fixtures | trycmd + completest-pty shells | snapbox term-svg + trybuild | none | divan `clap_bench` | libtest + automod |

## 23.4 Exemplary excerpts

**Run every end-to-end test under every engine.** ripgrep's `rgtest!` macro reruns
each of its 334 invocations once per regex engine when the pcre2 feature is on
(extras/ripgrep/tests/macros.rs):

```rust
macro_rules! rgtest {
    ($name:ident, $fun:expr) => {
        #[test]
        fn $name() {
            let (dir, cmd) = crate::util::setup(stringify!($name));
            $fun(dir, cmd);

            if cfg!(feature = "pcre2") {
                let (dir, cmd) = crate::util::setup_pcre2(stringify!($name));
                $fun(dir, cmd);
            }
        }
    };
}
```

**One assertion for exit code, stdout, and stderr.** ruff's CLI tests import
`insta_cmd::{assert_cmd_snapshot, get_cargo_bin}` (extras/ruff/crates/ruff/tests/config.rs)
so a single snapshot pins the full observable contract of an invocation, with tempdir
path filters keeping it stable across platforms.

**Timeouts as deadlock detectors.** ruff's nextest CI profile
(extras/ruff/.config/nextest.toml):

```toml
[profile.ci]
failure-output = "immediate-final"
fail-fast = false
slow-timeout =  { period = "1s", terminate-after = 60 }
```

Any test that hangs for sixty periods is terminated and reported instead of wedging
the CI job, which converts event-loop deadlocks from infrastructure mysteries into
named test failures.

**A TUI snapshot in five lines.** gitui builds the real app, draws into a
`TestBackend`, and snapshots the buffer (extras/gitui/src/gitui.rs):

```rust
let mut terminal =
    Terminal::new(TestBackend::new(90, 12)).unwrap();

gitui.draw(&mut terminal).unwrap();

assert_snapshot!("app_loading", terminal.backend());
```

**Declarative scenarios as data.** deno's per-directory manifest
(`extras/deno/tests/specs/add/no_save/__test__.jsonc`) chains real CLI steps in a
temp dir, each checked against a golden `.out` file, and a CI lint fails when any
`.out` file is unreferenced by a manifest. The scenario corpus grows without any new
Rust being written.

## 23.5 What a new Rust project should do

- Put integration tests in one registered binary: `autotests = false` plus a single
  `tests/<name>.rs` including modules, in the ripgrep and clap style, to keep link
  time flat as the suite grows.
- Drive the real binary via `CARGO_BIN_EXE_<name>` inside `TempDir` fixtures, with a
  small harness struct owning setup, invocation, and diff-formatted failure output.
- Adopt insta early and wrap it in one project macro like `uv_snapshot!`, with
  filters for paths, timings, and hashes; snapshot exit code, stdout, and stderr
  together via insta-cmd; run `cargo insta test --unreferenced reject` in CI.
- Snapshot `--help` for every subcommand into committed files (expect-test or trycmd
  transcripts) so the CLI surface cannot drift silently.
- For a TUI, render into `ratatui::backend::TestBackend` and snapshot the buffer;
  for terminal-real behavior, open a PTY with hang protection (`wait-timeout`).
- Property-test only algebraic invariants: parsers round-trip, orderings are
  transitive, apply/revert is identity. Use proptest or quickcheck with seeds
  honored from the environment.
- Add a workspace-excluded `fuzz/` package with a cargo-fuzz target for every parser
  of untrusted input, and at least `cargo check` it in CI on every run.
- Keep shared fakes and builders in a dedicated test-support crate or a
  `test-support` cargo feature, never duplicated per test file.
- Run tests under cargo-nextest with a `ci` profile: `fail-fast = false`, a
  `slow-timeout` with `terminate-after` as a deadlock detector, JUnit output, and
  serial `test-groups` for anything touching shared global state.
- Benchmark at two levels: criterion or divan for hot in-process paths, hyperfine on
  the built binary for end-to-end latency; wire results to a tracking service or a
  committed baseline before optimizing anything.
- Skip a coverage-percentage gate; if coverage is wanted, run cargo-llvm-cov with
  `--include-ignored` as an informational job and enforce behavior through
  snapshots, invariant tests, and fuzzing instead.
- Engineer determinism explicitly: pin git config in fixtures, sandbox global tool
  config, inject clocks behind `cfg(test)` seams, and serialize tests that cannot be
  isolated.
