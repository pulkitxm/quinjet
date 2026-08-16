# Quinjet Gap Analysis

This chapter closes the series by turning chapters 01 through 27 back onto quinjet itself.
Part 1 records where quinjet already sits at or above the bar set by the eighteen reference
repositories. Part 2 verifies the strongest claim in ARCHITECTURE.md: that every operation
reachable in the terminal interface is also a command-line subcommand. Part 3 lists every gap
that remains, ordered by priority, each with the reference evidence that makes it a gap.

Priorities: P0 is a clear industry consensus quinjet lacks and would gain real value from.
P1 is a strong practice with moderate value. P2 is optional polish. Every recommendation
respects three constraints: the single-crate binary stays single-crate, every feature stays
reachable through the CLI, and each change lands in under roughly 2,000 diff lines.

## Part 1: Where quinjet already meets or exceeds the industry bar

### The lint wall is stricter than any reference repository

Cargo.toml sets `unsafe_code = "forbid"` plus roughly thirty rustc lints at deny, then puts
clippy `all`, `pedantic`, `nursery`, and `cargo` at deny with priority -1 and layers about
sixty named restriction lints on top: `unwrap_used`, `expect_used`, `panic`,
`indexing_slicing`, `string_slice`, `print_stdout`, `print_stderr`, `exit`, `todo`,
`unimplemented`, `unreachable`, and more. None of the eighteen reference repositories runs a
wall this tall; [Lints and Static Analysis](./patterns/lints-and-static-analysis.md) records nushell and zed as the strictest, and both deny far fewer
restriction lints than quinjet does. The escape hatches are configured correctly too:
clippy.toml carries `msrv = "1.88"`, `allow-unwrap-in-tests`, `allow-expect-in-tests`,
`allow-panic-in-tests`, `allow-indexing-slicing-in-tests`, tuned thresholds, and
`disallowed-methods` entries for `std::env::set_var` and `std::env::remove_var` with reason
strings, the pattern chapters 03, 04, and 15 recommend from extras/deno/cli/clippy.toml and
extras/nushell/clippy.toml. Suppressions use `#[expect(lint, reason = "...")]` because
`allow_attributes` and `allow_attributes_without_reason` are denied in Cargo.toml, which is
the bevy discipline from extras/bevy/Cargo.toml already fully adopted.

### MSRV is pinned three times, exactly as gitui recommends

`rust-version = "1.88"` in Cargo.toml, `msrv = "1.88"` in clippy.toml, and a CI job in
.github/workflows/ci.yml that installs 1.88 and asserts
`cargo metadata ... | jq -r '.packages[0].rust_version'` equals 1.88 before running
`cargo check`. That is the triple pin from extras/gitui/.github/workflows/ci.yml, plus
.github/workflows/deep.yml runs `cargo msrv verify` weekly to prove 1.88 is the true minimum,
which goes beyond every reference repository.

### CI topology matches the best of the corpus

.github/workflows/ci.yml has the aggregation gate job (`ci`, `if: always()`, `needs:` on
everything, failing on `failure` or `cancelled`) that [CI CD Patterns](./patterns/ci-cd-patterns.md) traces to
extras/clap/.github/workflows/ci.yml and extras/bat/.github/workflows/CICD.yml, and
.github/workflows/hygiene.yml repeats the pattern for its own fifteen jobs. Every workflow
sets top-level `permissions: contents: read`, every checkout sets
`persist-credentials: false`, every action is pinned to a full commit SHA with a version
comment, every workflow carries a `concurrency` group whose `cancel-in-progress` is
conditional on `pull_request`, and `merge_group:` is in the trigger lists. The test matrix
covers ubuntu, ubuntu-24.04-arm, macos, and windows with `fail-fast: false`, a beta clippy
row runs with `continue-on-error`, and a cross-check matrix covers four extra targets. This
is the union of the hardening chapters 21 and the uv, ripgrep, fd, and bevy workflow lessons,
already implemented.

### The verification breadth exceeds every reference repository

The Makefile `ci` target chains formatting, clippy, tests in two feature configurations,
rustdoc with `-D warnings` and `--document-private-items`, cargo-deadlinks, comment and
secret checkers, typos, cargo-spellcheck, cargo-deny, cargo-audit, osv-scanner, cargo-machete
plus cargo-shear, cargo-sort, cargo-hack feature powerset, shellcheck plus shfmt, actionlint
plus zizmor in pedantic persona, yamllint strict, markdownlint, taplo fmt plus lint,
editorconfig-checker, ruff, the wiki drift check, and `cargo package`. The `deep` target and
.github/workflows/deep.yml add miri, three sanitizers, cargo-careful, cargo-mutants sharded
six ways, cargo-minimal-versions, cargo-udeps, and cargo-bloat, run weekly and on a
`deep-check` PR label. [Testing Strategies](./patterns/testing-strategies.md) asks new projects for a fraction of this. The rustdoc gate
with private items ([Lints and Static Analysis](./patterns/lints-and-static-analysis.md) item 11, extras/ripgrep/.github/workflows/ci.yml docs job) is
in both the Makefile and ci.yml.

### The security posture is broader than any single reference

.github/workflows/security.yml runs cargo-audit, cargo-deny split into a four-way matrix per
check (the split from extras/starship/.github/workflows/security-audit.yml), osv-scanner,
gitleaks, semgrep, trivy, a cargo-cyclonedx SBOM artifact, dependency-review on PRs, CodeQL,
and OpenSSF Scorecard, on a weekly cron plus every PR and push. deny.toml documents both
advisory ignores with reasons and removal conditions (the discipline of the [tauri study](./studies/tauri.md) item 3,
extras/tauri style), sets `wildcards = "deny"` and `unknown-registry = "deny"` as [Lints and Static Analysis](./patterns/lints-and-static-analysis.md)
item 9 asks, and bans openssl with a reason string.

### Releases are automated end to end with provenance

.github/workflows/release.yml picks the next free patch version against crates.io, re-runs
fmt, clippy, tests, and `cargo package` before tagging, builds five targets including both
macOS architectures and aarch64 musl with `cargo auditable`, smoke-tests each artifact,
generates SHA256SUMS and a syft SBOM, signs with `actions/attest-build-provenance`, publishes
through an environment-gated crates.io job, and is idempotent when re-run. That covers the
[Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md) checklist items on tag-manifest agreement, draft-until-complete semantics,
checksums, attestation, and SBOM (extras/ripgrep/.github/workflows/release.yml,
extras/fd/.github/workflows/CICD.yml, extras/rustdesk/.github/workflows/flutter-build.yml)
in one workflow. install.sh and install.ps1 are themselves tested by tests/install.sh and
tests/install.ps1 on all three desktop OSes in ci.yml, which no reference repository does.

### The command layer, exit discipline, and output contract are exemplary

`fn main() -> ExitCode` in src/main.rs, a typed `Failure { code, message, hint }` in
src/cli/mod.rs with named codes (1 failure, 3 not found, 4 unavailable, clap's own 2 for
usage), `ErrorKind::BrokenPipe` mapped to exit 0 in `cli::report`, and hint lines rendered
uniformly: that is the fd exit-code enum (extras/fd/src/exit_codes.rs), the ripgrep and bat
broken-pipe rule (extras/bat/src/error.rs), and the uv `Hint` pattern
(extras/uv/crates/uv-errors/src/lib.rs) all present. The `Emitter` in src/cli/mod.rs
guarantees one JSON document per invocation on a locked stdout, documented as a contract in
docs/cli/conventions.md. Destructive verbs (`discard`, `branch delete`, `stash drop`,
`stash clear`) report what they would do and require `--yes`, which is the alacritty
dry-run-before-wet-run lesson (extras/alacritty/alacritty/src/migrate/mod.rs) already built
into the CLI surface.

### Tests exist at every layer short of the process boundary

The crate carries about 180 inline `#[test]` functions: `TestRepository` fixtures in
src/cli/mod.rs run real `git` in scratch directories; src/ui/mod.rs renders into
`ratatui::backend::TestBackend` and asserts geometry and buffer content, the gitui frame-test
idea from extras/gitui/src/gitui.rs; src/cli/mod.rs holds `Cli::command().debug_assert()`,
verb-versus-path disambiguation tests, a test that every subcommand answers `-C` and
`--json`, generation-scoped workspace tests, and dry-run tests for every destructive verb.
.config/nextest.toml defines default and ci profiles with `fail-fast = false` and a
`slow-timeout` with `terminate-after`, matching the [Testing Strategies](./patterns/testing-strategies.md) nextest checklist.

### Documentation and repository hygiene

docs/cli has a page per verb under branch/, changes/, pull-request/, remotes/, repository/,
and stash/, plus conventions.md documenting the exit-code table and the `--json` guarantee.
scripts/sync_wiki.py generates the GitHub wiki from docs/ and `--check` gates broken links in
hygiene.yml. ARCHITECTURE.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, structured
issue forms (.github/ISSUE_TEMPLATE/bug.yml, feature.yml), a PR template, CODEOWNERS, a
grouped weekly dependabot.yml for cargo and github-actions, a labeler, and a stale sweeper
all exist. .github/workflows/pr.yml enforces conventional PR titles, conventional commit
subjects, and linear history, the committed/semantic-pr pattern from
extras/clap/committed.toml. scripts/check_comments.py and scripts/check_secrets.py each ship
a `--selftest` and run in hygiene.yml, and a grep confines `Command::new` to src/git,
src/cli, and src/main.rs, which is a repo-specific checker in the spirit of [Lints and Static Analysis](./patterns/lints-and-static-analysis.md)
item 12.

## Part 2: The CLI parity claim, verified

ARCHITECTURE.md states that an operation reachable on screen and not from the command line
cannot exist, because the interface's Git worker executes the same `cli::Command` vocabulary
a subcommand does. The claim was checked in both directions against the code.

Mutating operations: `GitOperation` in src/git/mod.rs has 23 variants, and src/app.rs
constructs every one of them from the interface. Each maps to a verb in src/cli/mod.rs:
`Stage`/`StageAll` to `stage`, `Unstage`/`UnstageAll` to `unstage`, `Discard` to `discard`,
`Commit` to `commit` with `--amend`, `Fetch`/`Pull`/`Push`/`Sync` to their verbs, `Checkout`
to `branch switch`, `CreateBranch` to `branch create`, `RenameBranch` to `branch rename`,
`DeleteBranch` to `branch delete`, the five stash variants to `stash push`, `apply`, `pop`,
`drop`, and `clear`, `ResolveConflict` to `resolve --ours|--theirs` (with `--stage` mapping
to `Stage`), `CherryPick` to `cherry-pick`, and `Revert` to `revert`.

Read operations: every query the worker issues in src/git/worker.rs has a verb. `Refresh`
is `status`, `LoadHistory` is `log`, `LoadBranches` is `branch list`, `LoadHistoryBranches`
is `branch list --all`, `LoadStashes` is `stash list`, `PrepareLocalDiff` and
`LoadLocalDiffFile` back `diff`, `show`, `branch compare`, and `stash show`,
`LoadGitHubRepositories` is `repos`, `LookupPullRequest` is `pr view`, `PreparePullRequest`
and `LoadPullRequestFile` back `pr files` and `pr diff`, `LoadPullRequestChecks` is
`pr checks`, `LoadPullRequestConversation` is `pr conversation`, and `LoadCheckRunLog` is
`pr logs`. Opening a pull request in a browser exists on both sides: `AppEffect::OpenUrl` in
src/main.rs and `pr open` in src/cli/mod.rs, sharing `cli::open_url`.

The only `Command` variants in src/cli/command.rs without a dedicated verb are
`WarmCheckRunLogs`, which prefetches logs and changes nothing observable, and
`PullRequestFileBatch`, an internal batching form of what `pr diff` already exposes. Neither
is an operation visible on screen. In the other direction, every verb's behavior is reachable
in the interface. Conclusion: the claim holds, and there is no P0 parity gap to report. What
is missing is a machine check that keeps it true, which Part 3 files as gap QJ-05.

## Part 3: Gaps, ordered by priority

### P0-1 (QJ-01): No panic hook restores the terminal

Cargo.toml sets `panic = "abort"` in `[profile.release]`, and src/main.rs installs no panic
hook. `TerminalGuard::drop` in src/main.rs restores raw mode, the alternate screen, mouse
capture, and keyboard enhancement flags, but with abort semantics destructors never run on
panic, so any panic in a dependency (ratatui, crossterm, syntect, std) leaves the user's
shell in raw mode on the alternate screen with no visible error. The lint wall prevents
first-party panics but cannot prevent third-party ones. The [nushell study](./studies/nushell.md) calls this the single
highest-value item for a ratatui binary. Evidence: extras/nushell/src/main.rs registers
`std::panic::set_hook` that disables raw mode before reporting;
extras/gitui/src/main.rs `set_panic_handler` restores the terminal and prints the backtrace;
extras/meilisearch/crates/meilisearch/src/main.rs logs panics through a hook. Fix: in
src/main.rs, before `TerminalGuard::enter`, install a hook that calls
`crossterm::terminal::disable_raw_mode`, executes `LeaveAlternateScreen` and
`DisableMouseCapture` on a fresh `io::stdout()`, then writes the panic payload and location
with `writeln!` to stderr. The hook runs before the abort, so `panic = "abort"` can stay.

### P0-2 (QJ-02): No shell completions and no man page

Nothing in Cargo.toml or src/ references clap_complete or clap_mangen, and docs/cli has no
completions page. This is the most uniform consensus in the corpus: tauri ships a
completions subcommand (extras/tauri/crates/tauri-cli/src/completions.rs), zed covers six
shells (extras/zed/crates/cli/src/completions.rs), fd generates completions and a man page
and installs them from its Makefile (extras/fd/src/cli.rs, extras/fd/Makefile), ripgrep
generates both from the binary, alacritty tests generated completions against checked-in
files (extras/alacritty/alacritty/src/cli.rs), starship, ruff, deno, and bat all ship
completions, and clap documents the mechanism itself (extras/clap/clap_mangen/Cargo.toml).
For a keyboard-first Git tool whose whole surface is subcommands, tab completion is not
polish, it is the product. Fix: add `clap_complete` (plus `clap_complete_nushell`) and
`clap_mangen` as dependencies, add a `quinjet completions <shell>` verb and a hidden
`quinjet man` verb to `Verb` in src/cli/mod.rs writing to the locked stdout, package the
generated files into the release artifacts in .github/workflows/release.yml, and smoke-test
`eval "$(quinjet completions bash)"` in ci.yml the way
extras/uv/.github/workflows/test-smoke.yml does.

### P0-3 (QJ-03): No black-box tests run the shipped binary

tests/ contains only install.sh and install.ps1, which test the installers, not the program.
Every Rust test constructs a `Session` or an `App` in process; nothing executes the compiled
`quinjet` binary with real argv and asserts stdout, stderr, and the exit code as a child
process, so `main`'s dispatch, clap parsing of actual argument vectors, the broken-pipe exit
path, and the `--json` document shape have no end-to-end coverage. [Testing Strategies](./patterns/testing-strategies.md) calls the
real-binary harness the backbone of CLI testing: extras/ripgrep/tests/util.rs drives the
compiled binary in scratch directories, extras/fd/tests/testenv/mod.rs locates it with
`env!("CARGO_BIN_EXE_fd")` and isolates the environment,
extras/bat/tests/utils/command.rs scrubs every relevant variable, and
extras/uv/crates/uv-test/src/lib.rs wraps insta so exit code, stdout, and stderr are pinned
together. Fix: add a `tests/cli.rs` integration target using `assert_cmd`, `predicates`, and
`tempfile`, with a fixture that runs `git init` plus seeded commits, `env_remove`s
`GIT_DIR`, `GIT_CONFIG_*`, and `HOME`-scoped config, and covers each verb's text output,
`--json` output parsed with `serde_json`, the `--yes` gates, and the documented exit codes
3 and 4. Registering it with `autotests = false` and one `[[test]]` keeps link time flat as
the suite grows, per extras/ripgrep/Cargo.toml.

### P1-1 (QJ-04): Help text and the hand-written CLI reference can drift

docs/cli is written by hand and scripts/sync_wiki.py only checks links, so a flag added in
src/cli/mod.rs never fails CI when docs/cli misses it, and `--help` output is not snapshot
anywhere. The [Documentation Practices](./patterns/documentation-practices.md) checklist asks for `--help` snapshots asserted by a test and for
derivable docs to be drift-checked. Evidence: extras/bat/tests/integration_tests.rs asserts
`--help` against extras/bat/doc/long-help.txt with expect-test;
extras/clap/tests/ui/help_flag_stdout.toml pins help output as a trycmd case;
extras/uv/.github/workflows/check-generated-files.yml re-runs generators and fails on diff;
extras/starship/.github/workflows/workflow.yml does the same for the config schema. Fix: add
`trycmd` as a dev-dependency with `tests/ui/*.toml` cases pinning `--help` for the root and
every verb, and add one `#[test]` that walks `Cli::command().get_subcommands()` recursively
and asserts a matching page exists under docs/cli, so a new verb without documentation fails
the build.

### P1-2 (QJ-05): The CLI-TUI parity invariant is not machine-checked

Part 2 verified parity by hand; no test enforces it. The realistic drift is a new
`GitOperation` variant wired into src/app.rs whose verb is forgotten in src/cli/mod.rs,
which no compiler error catches because `run` matches on `Verb`, not on `GitOperation`.
Evidence: extras/rustdesk/src/core_main.rs keeps a test that the IPC-scoped CLI command set
matches the management commands exactly, and extras/ripgrep/crates/core/flags/defs.rs tests
the flag inventory exhaustively. Fix: add a `#[test]` in src/cli/mod.rs with an exhaustive
`match` over a sample of every `GitOperation` variant that returns the argv which produces
it, then round-trips each through `Cli::try_parse_from` and asserts the parsed verb rebuilds
the same variant. Exhaustive matching makes adding a variant a compile error until the test
names its argv.

### P1-3 (QJ-06): No changelog, and no mechanical release-notes discipline

There is no CHANGELOG.md; .github/workflows/release.yml relies on
`generate_release_notes: true`, which produces a raw PR list. The corpus is near-unanimous
that user-facing changes deserve a curated or structured changelog: the [Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md) checklist,
extras/alacritty/CHANGELOG.md with its legislated section order, extras/fd/CHANGELOG.md with
its permanent Unreleased section, extras/bat/.github/workflows/require-changelog-for-PRs.yml
enforcing entries, extras/gitui/.github/workflows/cd.yml extracting release notes, and
extras/clap/Cargo.toml `pre-release-replacements`. quinjet already enforces conventional
commits in .github/workflows/pr.yml, so the structured input exists. Fix: adopt git-cliff
with a committed cliff.toml, generate the release body from the tag range in release.yml in
place of `generate_release_notes`, and commit a generated CHANGELOG.md refreshed by the
release job.

### P1-4 (QJ-07): `cargo binstall quinjet` does not work

Cargo.toml has no `[package.metadata.binstall]`, and the release artifacts
(quinjet-linux-x86_64 and friends, from .github/workflows/release.yml) are bare binaries
whose naming binstall cannot guess. Evidence: extras/fd/Cargo.toml ships binstall metadata
with per-target overrides, extras/nushell/Cargo.toml and
extras/tauri/crates/tauri-cli/Cargo.toml do the same, and the same checklist calls for
it on day one. Fix: add `[package.metadata.binstall]` with
`pkg-url = "{ repo }/releases/download/v{ version }/quinjet-{ target-family }-{ target-arch }"`
adjusted to the existing artifact names, `pkg-fmt = "bin"`, and a Windows override for the
`.exe` suffix, then verify once with `cargo binstall --dry-run quinjet`.

### P1-5 (QJ-08): The parsers of untrusted Git output have no property tests or fuzzing

src/git/status.rs (`parse_porcelain_v2`), src/git/diff.rs (`parse_diff`, `parse_numstat`),
and src/git/history.rs (`parse_log`) parse bytes that arbitrary repositories control:
branch names, paths, and commit subjects are attacker-influenced. They have example-based
tests but no property tests and no fuzz targets, and quinjet has no fuzz/ directory.
[Testing Strategies](./patterns/testing-strategies.md) reserves property testing for exactly this shape, and the corpus agrees:
extras/deno/runtime/permissions/lib.rs proptests ordering invariants,
extras/nushell/crates/nu-parser/fuzz/fuzz_targets/parse.rs and
extras/ripgrep/fuzz/fuzz_targets/fuzz_glob.rs keep three-line libfuzzer targets in
workspace-excluded packages, and extras/meilisearch/crates/filter-parser/fuzz treats parse
errors as success and panics only on internal errors. Fix: add proptest as a dev-dependency
with never-panics and round-trip properties for the three parser modules, and a fuzz/
package with its own `[workspace]` table (so the main lint wall and lockfile are untouched)
holding one target per parser, with `cargo check --manifest-path fuzz/Cargo.toml` in ci.yml.

### P2-1 (QJ-09): Spawned git and gh processes have no time limit

`run_bounded_command` in src/git/github/mod.rs bounds output bytes but not wall time, so a
hung credential helper or a wedged `gh` blocks its worker lane forever (src/git/worker.rs).
Evidence: extras/starship/src/utils/mod.rs wraps every external command in `exec_timeout`
built on the process_control crate and degrades to a logged `None`. Fix: add
`process_control` and give `run_bounded_command` a `time_limit` with
`terminate_for_timeout`, surfacing the timeout as a normal `Failure` on the CLI and a toast
in the interface.

### P2-2 (QJ-10): No .gitattributes

The repository has no .gitattributes, so line endings depend on each contributor's autocrlf
and no diff drivers are declared. Evidence: extras/helix/.gitattributes sets `* text=auto`
with per-extension diff drivers; extras/starship/.gitattributes forces `eol=lf` on files
whose bytes matter. Fix: commit a .gitattributes with `* text=auto eol=lf`, `*.rs diff=rust`,
`*.toml diff=toml`, and binary markers for any future image assets.

### P2-3 (QJ-11): No .git-blame-ignore-revs

Formatting-only commits will eventually pollute blame. Evidence:
extras/zed/.git-blame-ignore-revs, honored automatically by GitHub. Fix: commit the file now
with a header explaining its use, and add revisions when a rustfmt or style migration lands.

### P2-4 (QJ-12): Blank issues are still enabled

.github/ISSUE_TEMPLATE has bug.yml and feature.yml but no config.yml, so the forms can be
bypassed. Evidence: extras/rustdesk/.github/ISSUE_TEMPLATE/config.yml sets
`blank_issues_enabled: false` and routes questions to Discussions;
extras/fd/.github/ISSUE_TEMPLATE and extras/ripgrep's config.yml do the same. Fix: add
.github/ISSUE_TEMPLATE/config.yml with `blank_issues_enabled: false` and a contact link to
the repository's Discussions.

### P2-5 (QJ-13): Almost no job sets timeout-minutes

Only .github/workflows/wiki.yml sets `timeout-minutes`; every other job inherits the 360
minute default, so a hung step burns six hours of runner time. Evidence: the [deno study](./studies/deno.md) item 8,
and extras/bevy/.github/workflows/ci.yml and extras/helix/.github/workflows/build.yml set
`timeout-minutes` on every job. Fix: add `timeout-minutes` (15 for lint-shaped jobs, 30 for
test and build jobs, 60 for mutants shards) across .github/workflows/.

### P2-6 (QJ-14): Dependabot has no cooldown

.github/dependabot.yml updates weekly with grouping but proposes releases published minutes
earlier, which is the supply-chain window [Dependencies, Releases, and Distribution](./patterns/dependencies-release-distribution.md) warns about. Evidence:
extras/fd/.github/dependabot.yml and extras/bevy/.github/dependabot.yml set
`cooldown: default-days: 7`. Fix: add the `cooldown` block to both ecosystems in
.github/dependabot.yml.

### P2-7 (QJ-15): `--version` carries no build metadata

There is no build.rs, so a dev build and a release build of 0.0.6 are indistinguishable in a
bug report. Evidence: extras/alacritty/alacritty/build.rs embeds the short commit hash into
the clap version string, extras/ripgrep/build.rs exposes it through `option_env!`, and
extras/gitui/build.rs honors `SOURCE_DATE_EPOCH` for reproducibility. Fix: add a build.rs
that runs `git rev-parse --short HEAD`, emits `cargo:rustc-env=QUINJET_BUILD_INFO=...`, and
wire `#[command(version = ...)]` in src/cli/mod.rs to include it. Note the lint wall: the
`[lints]` table applies to build scripts, so the `println!` directives need one scoped
`#[expect(clippy::print_stdout, reason = "cargo build-script directives")]`.

### P2-8 (QJ-16): No bug-report subcommand

Evidence: extras/gitui/src/bug_report.rs assembles version, OS, and compile-time information
with the bugreport crate; extras/starship ships `starship bug-report`. For quinjet the same
verb would also report `git --version` and whether `gh` authenticates, the two facts every
issue needs. Fix: a `quinjet bug-report` verb using the `bugreport` crate, emitted through
the existing `Emitter` so `--json` works, and a link to it from
.github/ISSUE_TEMPLATE/bug.yml.

### P2-9 (QJ-17): The subprocess-confinement rule lives in a workflow grep

hygiene.yml greps that `Command::new` appears only under src/git, src/cli, and src/main.rs.
That works but is invisible to local `cargo clippy` and editors. Evidence: [Deep Rust Language Idioms](./patterns/rust-language-idioms.md) closes
with "ban the hazards you have wrapped": extras/zed/clippy.toml bans
`std::process::Command::spawn` with a reason, and extras/starship/src/utils/mod.rs pairs the
ban with one sanctioned `#[allow]` site. Fix: add `std::process::Command::new` to
`disallowed-methods` in clippy.toml with a reason naming the sanctioned modules, put
`#[expect(clippy::disallowed_methods, reason = "...")]` on the few call sites in src/git and
src/cli, and keep or retire the grep.

### P2-10 (QJ-18): No benchmarks anywhere

There is no benches/ directory and no hyperfine script, so diff-rendering and startup
regressions are invisible. Evidence: extras/bat/tests/benchmarks/run-benchmarks.sh measures
startup with hyperfine; extras/nushell/benches/benchmarks.rs uses tango for paired runs;
[Testing Strategies](./patterns/testing-strategies.md) recommends criterion or divan in-process plus hyperfine end-to-end. Fix: one
criterion bench over `parse_diff` and `parse_porcelain_v2` with `[[bench]] harness = false`,
plus a scripts/bench.sh wrapping hyperfine on `quinjet status` in a fixture repository,
reported in deep.yml rather than gating CI.

### P2-11 (QJ-19): No third-party audit sharing via cargo-vet

cargo-deny, cargo-audit, osv, and dependency-review check advisories, but nothing asserts a
human audited the code of new dependencies. Evidence: extras/tauri/supply-chain/config.toml
imports the mozilla, google, and bytecode-alliance audit sets. Fix: `cargo vet init`, import
the same sets, commit supply-chain/, and add `cargo vet --locked` to security.yml. Optional
because quinjet's dependency tree is small and already tightly banned in deny.toml.

### P2-12 (QJ-20): src/app.rs and src/ui/mod.rs are outsized

src/app.rs is 6,694 lines (about 4,865 before its inline test module) and src/ui/mod.rs is
6,006 lines (about 4,770 before tests). [Formatting and Style](./patterns/formatting-and-style.md) advises keeping production modules
roughly under 2,000 to 3,000 lines unless they are deliberate single-source registries, and
[Project and Workspace Structure](./patterns/project-structure.md) shows the seam-based split in extras/helix (the helix-term commands/ satellite
directory). These files are cohesive state machines, so this is polish, not damage. Fix:
split src/app.rs along its existing regions (palette, prompts and modals, pull-request
state, toasts) into an app/ directory, and src/ui/mod.rs into sidebar, content, and overlay
modules, one file per PR to stay under the diff budget, recording any formatting-only
commit in .git-blame-ignore-revs (QJ-11).

## Summary

| Priority | Count | Gaps |
| --- | --- | --- |
| P0 | 3 | terminal-restoring panic hook; completions and man page; black-box binary tests |
| P1 | 5 | help and docs drift gate; parity inventory test; changelog discipline; binstall metadata; parser property tests and fuzzing |
| P2 | 12 | subprocess time limits; .gitattributes; .git-blame-ignore-revs; issue config.yml; job timeouts; dependabot cooldown; version build metadata; bug-report verb; disallowed-methods consolidation; benchmarks; cargo-vet; module splits |

The pattern across the corpus is clear: quinjet's static-analysis, CI, security, and release
machinery already exceed the eighteen reference repositories, often substantially. The gaps
cluster in exactly two places the wall cannot reach: what happens at the process boundary
(a panic mid-draw, a piped subcommand, a completion script, a hung child process) and what
keeps the two command surfaces and their documentation from drifting as the tool grows.
