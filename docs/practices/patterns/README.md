# Patterns

Nine cross-cutting syntheses. Each one names the consensus the eighteen
repositories share, the camps where they split and the reasoning on each side,
a comparison across the whole corpus, and a closing checklist a new Rust
project can apply directly.

## Chapters

- [Formatting and Style](./formatting-and-style.md): rustfmt camps, nightly
  options, editorconfig, and the formatters around Rust.

- [Lints and Static Analysis](./lints-and-static-analysis.md): lint tables,
  deny versus warn philosophy, clippy.toml knobs, and supply-chain scanners.

- [CI CD Patterns](./ci-cd-patterns.md): workflow architecture, matrices,
  caching, pinning, hardening, and release pipelines.

- [Project Structure](./project-structure.md): single crate versus workspace,
  module conventions, and where large files get split.

- [Testing Strategies](./testing-strategies.md): real-binary harnesses,
  snapshots, property tests, fuzzing, and coverage.

- [Error Handling and API Design](./error-handling-and-api-design.md): error
  types, exit codes, panic policy, and API discipline.

- [Rust Language Idioms](./rust-language-idioms.md): zero-copy, newtypes,
  concurrency selection, macros, and unsafe policy.

- [Dependencies, Releases, and Distribution](./dependencies-release-distribution.md):
  dependency hygiene, MSRV, changelogs, and shipping binaries.

- [Documentation Practices](./documentation-practices.md): rustdoc gates,
  manuals, templates, and generated references.
