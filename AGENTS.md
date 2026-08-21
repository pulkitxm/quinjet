# Working on Quinjet

Quinjet is a single-crate Rust 2024 binary: a keyboard-first Git interface for
the terminal, built on ratatui, crossterm and clap. This file is the contract
for contributors changing it.

## The one rule that shapes the codebase

Every user-visible repository or GitHub operation in the terminal interface is
also reachable from a command-line verb. Repository reads and mutations use the
`cli::Command` vocabulary executed by `cli::Session`. Browser opening uses the
same `cli::open_url` helper from both faces. Presentation state such as focus,
scrolling, folding, filtering, and mouse capture remains specific to the terminal
interface.

This is enforced for mutations, not merely documented. One macro declaration
in `src/cli/mod.rs` generates the exhaustive `GitOperation` route match and one
fixture per variant. Adding a variant without a route fails to compile, and the
test resolves every route in the real clap command tree.

When you add an operation, do all of it:

1. Add the variant to `GitOperation` in `src/git/mod.rs`, or to `Command` in `src/cli/command.rs` for a read.
2. Execute it in `cli::Session` in `src/cli/command.rs`.
3. Render its `Outcome` as plain text in `src/cli/render.rs`.
4. Add the subcommand to `Verb` in `src/cli/mod.rs`, and add its pattern, fixture, and path to `operation_routes!` when it is a `GitOperation`.
5. Reach it from the terminal in `src/app.rs`.
6. Write its reference page under `docs/cli/`, linked from the group's `README.md`.

## Layout

```text
src/
  main.rs          argv dispatch, terminal loop, panic hook, terminal guard
  cli/
    command.rs     the command vocabulary, outcomes, and the session
    mod.rs         the subcommand tree, exit codes, the --json emitter
    render.rs      plain-text renderings, no terminal and no color
    update.rs      verified release lookup and executable replacement
    watch.rs       the non-interactive refresh loop
  git/             argv construction, status, history, diff, worker, github/
  ui/              viewport-only rendering, mouse hit map
  app.rs           focus, selection, modal, command and generation state
  state.rs         recently opened project paths
  theme.rs         selectable palettes and semantic syntax colors
docs/cli/          one page per verb, generated into the wiki
docs/practices/    how widely used Rust projects are engineered
```

`ARCHITECTURE.md` holds the responsiveness invariants. Read it before changing
the worker, the caches, or anything that touches a generation.

## House rules

- No comments in tracked files. Only exact machine-read directives are accepted.
  Use attributes for Rust and clap documentation metadata, and let names and
  structure carry the meaning. `scripts/check_comments.py` checks every tracked
  file and rejects unclassified text formats.
- Never use the em-dash character, in code, docs, or commit messages.
- Prefer established crates over hand-rolled implementations.
- Do not preserve backward compatibility for its own sake; choose the simplest
  implementation that meets the current requirement.

## The lint wall

`unsafe_code` is forbidden. Clippy runs with `all`, `pedantic`, `nursery` and
`cargo` denied, plus a large restriction set: `unwrap_used`, `expect_used`,
`panic`, `indexing_slicing`, `print_stdout`, `print_stderr`, `exit`, `todo`,
`unreachable` and more. Tests get the usual escapes through `clippy.toml`.

Expect to meet it. Some consequences worth knowing before you write code:

- Return `Result` and use `anyhow::Context`; do not unwrap or panic.
- Write to `io::stdout()` through the `Emitter`, never `println!`.
- `drop(x)` is how this codebase discards a `Result` it cannot use. For a
  `Copy` result such as `fmt::Error`, restructure instead; both `drop` and
  `let _ =` are refused.
- Build strings with `push_str`, not `push_str(&format!(..))` and not
  `format!` over an iterator.
- When a lint is genuinely wrong, write `#[expect(lint, reason = "...")]`
  scoped as tightly as possible. Bare `#[allow]` is denied, and so is
  `#[expect]` without a reason.

## Verifying

```bash
make ci-fast
make ci
make deep
```

`make ci` is the broad local gate. GitHub also runs platform matrices,
cross-target builds, installer tests, coverage, security scans, and other
workflow-only jobs that the local target does not reproduce.

`make tools` installs the cargo-based checkers once; `make tools-deep`
installs the expensive ones. At minimum, before pushing:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
```

Documentation is checked by `markdownlint-cli2` and `typos`. The repository also
has a separate generated-wiki link check for changes that affect that output.

## Tests

- Unit tests live beside the code they cover, in `#[cfg(test)]` modules.
- `tests/cli.rs` runs the built binary through argv in scratch repositories.
  Anything about argument parsing, exit codes, or the shape of `--json`
  belongs there rather than in a unit test.
- Process tests remove repository-affecting Git environment variables, disable
  system configuration, and point global configuration at the null device.
- Completion coverage runs all five generators outside a repository and checks
  the bash result with `bash -n`. Manual coverage also runs outside a repository
  and verifies a nested page's full command path and inherited global options.
- Destructive process tests prove previews do not mutate, then prove `--yes`
  performs discard, cherry-pick, and revert.

## Commits and pull requests

Conventional commit subjects (`feat:`, `fix:`, `docs:`, `ci:`, `test:`,
`chore:`), written as what changed and why, not as a list of files. Commit at
every logical checkpoint rather than landing one large diff. Keep a pull
request reviewable; split by concern when it grows past roughly two thousand
lines. Squash merge, then delete the branch.
