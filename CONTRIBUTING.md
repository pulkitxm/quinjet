# Contributing to Quinjet

Thank you for helping improve Quinjet. Bug reports, design feedback, documentation, tests, and code are welcome.

## Before You Start

- Search existing issues and pull requests before opening a duplicate.
- For a substantial feature or architectural change, open an issue first so the approach can be discussed.
- Keep changes focused. Separate unrelated fixes into separate pull requests.
- Never include credentials, private repository data, or generated build artifacts.

## Development Setup

Quinjet requires Rust 1.88 or newer and Git.

```bash
git clone https://github.com/pulkitxm/quinjet.git
cd quinjet
cargo run -- /path/to/a/test/repository
```

The `extras/` directory is ignored and is only for local reference repositories or experiments.

## Required Checks

CI runs formatting, linting, tests, documentation, MSRV, feature powerset,
cross-target builds, coverage, packaging, installer smoke tests, repository
hygiene, spelling, workflow linting, dependency auditing, and generated-wiki
checks. The broad local targets are:

```bash
make tools
make ci
make ci-fast
make tools-deep
make deep
```

`make ci` does not reproduce every GitHub-only gate. Platform matrices,
cross-target builds, installer smoke tests, coverage, security scans, and other
workflow jobs still run in GitHub.

The deep checks also run weekly, and on demand when a pull request is labeled
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
- Put every user-visible repository or GitHub operation in `src/cli`. The terminal interface is a caller of that layer, never a second implementation. Focus, scrolling, folding, filtering, and other presentation state do not need verbs.
- Update `README.md`, the in-app help, and `docs/cli/` when user-facing behavior changes. A new or changed flag that is not in `docs/cli/` is an incomplete change.

`tests/cli.rs` runs the built binary in isolated scratch directories. Keep
repository-affecting Git environment variables scrubbed there. Add process
coverage for argument parsing, output, exit codes, confirmation gates, and
metadata generators. Completion tests cover bash, zsh, fish, elvish, and
PowerShell, validate bash syntax with `bash -n`, and manual tests verify nested
command paths and inherited global options.

## Pull Requests

A good pull request includes:

1. A concise description of the problem and solution.
2. Screenshots or terminal recordings for visual changes when useful.
3. Tests or an explanation of why automated testing is impractical.
4. Notes about compatibility, performance, and destructive behavior.

Maintainers may request changes before merge. By contributing, you agree that your contribution is licensed under the repository's MIT License.

## Documentation

`docs/` is the source of the [project wiki](https://github.com/pulkitxm/quinjet/wiki). One Markdown file becomes one wiki page, a directory becomes a group whose `README.md` is the group's own page, and the order that `README.md` lists its pages in is the order the sidebar shows them.

```bash
python3 scripts/sync_wiki.py --check      # every page builds and every link resolves
python3 scripts/sync_wiki.py              # write .wiki-build/ and read it locally
```

Edit the docs in this repository, never the wiki: a push to `main` overwrites every wiki page from `docs/`. Links between pages are relative (`./verb.md`, `../conventions.md`) and are rewritten to wiki slugs during the sync, so a link that resolves to no file, or an anchor that matches no heading, fails the hygiene workflow.

## Reporting Security Issues

Do not publish exploitable security issues in a public issue. Use GitHub's private security advisory flow for this repository instead.

## Releases

Releases are automated. A push to `main` that touches `src/`, `Cargo.toml`, `Cargo.lock`, or `README.md` bumps the patch version, tags it, publishes the crate to crates.io, and attaches binaries to a GitHub release. Contributors should not bump the version themselves; a maintainer edits it directly when a release needs a minor or major bump.
