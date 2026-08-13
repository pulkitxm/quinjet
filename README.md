# Quinjet

A fast, keyboard-first Git source-control interface for the terminal, written in Rust.

> Quinjet is under active development.

## Goals

- Live working-tree, index, branch, and repository refresh
- VS Code-inspired source-control workflows
- Syntax-highlighted diffs and commit history
- Mouse support without sacrificing complete keyboard control
- Responsive UI: Git and filesystem work stay off the render thread

## Development

```bash
cargo run -- /path/to/repository
```

Run `cargo test` for tests and `cargo clippy --all-targets --all-features -- -D warnings` for linting.
