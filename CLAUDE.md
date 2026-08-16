# Claude notes for Quinjet

Read [AGENTS.md](./AGENTS.md) first. It holds the contract: the one-surface
rule, the layout, the house rules, the lint wall, and how to verify. This file
only adds what is specific to working here through Claude Code.

## Before you edit

- Ask where the work should happen. This repository is usually developed in
  worktrees beside the main checkout, and several already exist.
- The `extras/` directory is ignored by Git and holds reference clones and
  scratch research. Keep experiments there, never in the tracked tree.

## While you work

- Commit at every logical checkpoint, not once at the end. Each commit should
  build and say what changed in that step.
- Expect the lint wall to reject the first shape of new code. That is working
  as intended: read the lint, then restructure. Do not reach for `#[allow]`,
  and do not relax a lint repository-wide to make one call site compile.
- Do not write `//` comments. The build fails on them, and a doc comment or a
  better name is what the reviewer wanted anyway.
- Never use the em-dash character, and leave no AI attribution in commits,
  branches, code, or docs.

## Which checks to run when

Cheap, run often:

```bash
cargo fmt --all && cargo clippy --all-targets --all-features --locked
cargo test --all-features --locked
```

Before opening a pull request, `make ci` runs the full set, but several of its
steps need tools from `make tools` and take minutes. When only documentation
changed, these three are the ones that matter:

```bash
python3 scripts/sync_wiki.py --check
npx --yes markdownlint-cli2
typos
```

`make deep` is for a deliberate session, not a routine change: miri,
sanitizers, mutants and minimal-version resolution together take a long time.

## Documentation

`docs/` is the source; the GitHub wiki is generated from it on every push to
`main` by `scripts/sync_wiki.py`. Never edit the wiki directly. A new page has
to be linked from its section `README.md`, or the sidebar will not show it.

`docs/practices/` is a reference study of eighteen widely used Rust projects,
with a gap analysis of this repository at
[docs/practices/gap-analysis.md](./docs/practices/gap-analysis.md). When you
are deciding how something ought to be done here, that page usually already
has an answer and cites who does it that way.
