# Contributing to Quinjet

Thank you for helping improve Quinjet. Bug reports, design feedback, documentation, tests, and code are welcome.

## Before You Start

- Search existing issues and pull requests before opening a duplicate.
- For a substantial feature or architectural change, open an issue first so the approach can be discussed.
- Keep changes focused. Separate unrelated fixes into separate pull requests.
- Never include credentials, private repository data, or generated build artifacts.

## Development Setup

Quinjet requires stable Rust and Git.

```bash
git clone https://github.com/pulkitxm/quinjet.git
cd quinjet
cargo run -- /path/to/a/test/repository
```

The `extras/` directory is ignored and is only for local reference repositories or experiments.

## Required Checks

CI runs formatting, linting, tests, documentation, MSRV, feature powerset, cross-target
builds, coverage, packaging, installer smoke tests, repository hygiene, spelling, workflow
linting, and dependency auditing. The same set runs locally:

```bash
make tools      # once, installs the cargo-based checkers
make ci         # everything CI runs on a pull request
make ci-fast    # format, lint, test, comments, secrets
make tools-deep # once, installs the expensive checkers
make deep       # miri, sanitizers, cargo-careful, mutants, minimal versions, udeps, bloat
```

The deep checks also run weekly, and on demand when a pull request is labelled
`deep-check`.

At minimum, run this before submitting a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
python3 scripts/check_comments.py
python3 scripts/check_secrets.py
```

Lints are strict on purpose: the clippy `pedantic`, `nursery` and `cargo` groups are denied
along with a broad restriction set, so `unwrap`, `expect`, `panic`, indexing, and printing
to stdout all fail the build. Reach for a scoped `#[expect(lint, reason = "...")]` when a
lint is genuinely wrong for a piece of code, rather than relaxing it repository-wide.

Comments are checked too. Doc comments (`///`, `//!`) stay because clap and rustdoc render
them; ordinary `//` comments fail the build, so let names and structure carry the meaning.

Add focused tests for behavior changes, especially status parsing, destructive Git operations, input editing, scrolling, and pane geometry.

## Code Guidelines

- Keep terminal input and rendering independent from Git subprocess latency.
- Do not execute user-derived commands through a shell; pass Git arguments directly.
- Preserve generation checks and coalescing for asynchronous reads.
- Keep destructive operations explicit and confirmed.
- Ensure every action remains keyboard-accessible even when mouse support is added.
- Avoid unbounded output, queues, histories, or caches.
- Prefer small, typed domain changes over UI-specific Git logic.
- Update `README.md` and in-app help when user-facing behavior changes.

## Pull Requests

A good pull request includes:

1. A concise description of the problem and solution.
2. Screenshots or terminal recordings for visual changes when useful.
3. Tests or an explanation of why automated testing is impractical.
4. Notes about compatibility, performance, and destructive behavior.

Maintainers may request changes before merge. By contributing, you agree that your contribution is licensed under the repository's MIT License.

## Reporting Security Issues

Do not publish exploitable security issues in a public issue. Use GitHub's private security advisory flow for this repository instead.

## Releases

Releases are automated. A push to `main` that touches `src/`, `Cargo.toml`, `Cargo.lock`, or `README.md` bumps the patch version, tags it, publishes the crate to crates.io, and attaches binaries to a GitHub release. Contributors should not bump the version themselves; a maintainer edits it directly when a release needs a minor or major bump.
