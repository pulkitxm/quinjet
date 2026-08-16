# Documentation Practices

Documentation in mature Rust projects is not a single artifact. It is a layered system:
rustdoc on the API surface, a user manual somewhere (in rustdoc, in an mdbook, in a docs
site, or in plain Markdown), a README that acts as the front door, contributor process
docs, and templates that shape every issue and pull request. Across the eighteen
repositories studied here (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat,
starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap), the striking
result is how much of this system is mechanically enforced: doc lints, rustdoc warning
gates, drift checks on generated pages, and CI jobs that reject a PR without a changelog
entry. Documentation that is not checked by a machine decays; these projects know it.

A composite of the documentation surface these repositories converge on:

```text
repo/
|-- README.md                  front door: pitch, install, quickstart
|-- CHANGELOG.md               human-facing history, often CI-enforced
|-- CONTRIBUTING.md            process: build, test, changelog, PR rules
|-- ARCHITECTURE.md            (minority) crate map and design overview
|-- docs/ or book/             manual sources: mdbook, mkdocs, or plain md
|-- .github/
|   |-- ISSUE_TEMPLATE/
|   |   |-- bug_report.yml     structured form with required fields
|   |   `-- config.yml         routes questions to Discussions
|   `-- PULL_REQUEST_TEMPLATE.md
`-- crates/*/src/lib.rs        //! crate docs, doc lints, doc tests
```

## Consensus practices

**Every project ships a CONTRIBUTING document, even when it is tiny.** All eighteen have
one: at the root (extras/uv/CONTRIBUTING.md, extras/ripgrep/CONTRIBUTING.md,
extras/fd/CONTRIBUTING.md, extras/tokio/CONTRIBUTING.md and eleven more), under .github/
(extras/tauri/.github/CONTRIBUTING.md, extras/deno/.github/CONTRIBUTING.md), or inside
the docs tree (extras/helix/docs/CONTRIBUTING.md, plus translated variants such as
extras/rustdesk/docs/CONTRIBUTING-DE.md). Size varies enormously: ripgrep's is eight
lines that mostly defer to a policy file, while uv's opens by teaching contributors how
to pick an issue ("We label issues that we think are a good opportunity for subsequent
contributions as `help wanted`", extras/uv/CONTRIBUTING.md). The consensus is on the
file's existence and its role as the canonical answer to "how do I get a change merged",
not on its length.

**Issue templates plus a config.yml router are universal among projects that take bug
reports, and the routing target is Discussions.** Sixteen of eighteen have an
ISSUE_TEMPLATE directory; alacritty ships none at all, and helix omits the router.
The router pattern is identical everywhere: keep the tracker for actionable reports and
push questions elsewhere. From extras/ripgrep/.github/ISSUE_TEMPLATE/config.yml:

```yaml
blank_issues_enabled: true
contact_links:
  - name: Ask a question
    about: |
      You've come to seek help or want to discuss something related to ripgrep.
    url: https://github.com/BurntSushi/ripgrep/discussions/new
```

**Structured issue forms front-load triage work onto the reporter.** The strongest forms
make the reporter confirm they read the docs before the submit button works. ripgrep's
form lists known non-bugs, then requires a checkbox
(extras/ripgrep/.github/ISSUE_TEMPLATE/bug_report.yml):

```yaml
  - type: checkboxes
    id: issue-not-common
    attributes:
      label: Please tick this box to confirm you have reviewed the above.
      options:
        - label: I have a different issue.
          required: true
```

fd does the same against its README's troubleshooting section and requires the output of
`fd --version` (extras/fd/.github/ISSUE_TEMPLATE/bug_report.yaml). rustdesk, zed, and
starship additionally set `blank_issues_enabled: false` so every issue goes through a
form (extras/rustdesk/.github/ISSUE_TEMPLATE/config.yml).

**The README is a front door, not the manual.** Nearly every README follows the same
arc: pitch, screenshot or demo, install, quickstart, then links out to deeper docs.
ripgrep's README heading list is the archetype: CHANGELOG, documentation quick links,
screenshot, "Why should I use ripgrep?", "Why shouldn't I use ripgrep?", installation,
building, running tests (extras/ripgrep/README.md). Projects with global audiences
maintain translated READMEs: rustdesk links twenty-five language variants from its
README header into docs/ (extras/rustdesk/README.md), and bat keeps README-ja, -ko,
-ru, -zh under extras/bat/doc/.

**Library crates get doc lints; the docs build itself is a CI gate.** Wherever a crate
is meant to be consumed as a library, `missing_docs` appears in some strictness, and six
projects compile rustdoc with warnings denied in CI. ripgrep's matcher crate carries
`#![deny(missing_docs)]` (extras/ripgrep/crates/matcher/src/lib.rs, line 37), and its CI
runs rustdoc with `RUSTDOCFLAGS: -D warnings` (extras/ripgrep/.github/workflows/ci.yml).
The same gate appears in extras/clap/.github/workflows/ci.yml,
extras/bat/.github/workflows/CICD.yml, extras/helix/.github/workflows/build.yml,
extras/ruff/.github/workflows/ci.yaml, and extras/tokio/.github/workflows/ci.yml (which
adds `--cfg docsrs --cfg tokio_unstable` so unstable-feature docs are checked too).

**Generated documentation is committed and drift-checked.** When docs are derived from
code (keymaps, CLI references, config schemas), the generator runs in CI and any diff
fails the build. helix regenerates its mdbook's generated pages and fails with an
actionable message (extras/helix/.github/workflows/build.yml):

```yaml
      - name: Check uncommitted documentation changes
        if: always()
        run: |
          git diff
          git diff-files --quiet \
            || (echo "Run 'cargo xtask docgen', commit the changes and push again" \
            && exit 1)
```

The generated pages live at extras/helix/book/src/generated/ (lang-support.md,
static-cmd.md, typable-cmd.md), produced by extras/helix/xtask/src/main.rs. uv guards
its settings and environment-variable references with `generate-all --mode check`,
starship drift-checks its config schema, and bevy regenerates templated doc pages the
same way.

**The changelog is documentation with a process behind it.** bat enforces it
mechanically: a workflow diffs CHANGELOG.md against the base branch and greps the added
lines for the PR number and submitter
(extras/bat/.github/workflows/require-changelog-for-PRs.yml):

```yaml
          ADDED=$(git diff -U0 "origin/${PR_BASE}" HEAD -- CHANGELOG.md | grep -P '^\+[^\+].+$')
          echo "Added lines in CHANGELOG.md:"
          echo "$ADDED"
          echo "Grepping for PR info (see CONTRIBUTING.md):"
          grep "#${PR_NUMBER}\\b.*${PR_SUBMITTER}\\b" <<< "$ADDED"
```

The policy side lives in extras/bat/CONTRIBUTING.md ("Keeping the `CHANGELOG.md` file
up-to-date makes the release process much easier"), with matching guidance in
extras/fd/CONTRIBUTING.md, gitui's CI job that extracts release notes from the changelog
on every PR, and ripgrep's standing TBD section in extras/ripgrep/CHANGELOG.md.

## Divergent camps

**Where the user manual lives.** This is the deepest split, and it tracks the audience.

- Camp 1, rustdoc is the manual: clap and tokio. clap compiles its tutorial, cookbook,
  and FAQ as rustdoc modules under extras/clap/src/ (`_tutorial.rs`, `_cookbook/`, `_faq.rs`,
  `_derive/`), so every documentation example is a doc test that runs in CI. tokio's
  entire user-facing story is crate docs (extras/tokio/tokio/src/lib.rs), backed by a
  spellcheck dictionary at extras/tokio/spellcheck.dic. Reasoning: for a library, the
  API reference is where users already are, and doc tests make every example
  self-verifying.
- Camp 2, mdbook in-repo: helix and zed. extras/helix/book/book.toml deploys to a
  custom domain with per-page edit links (`edit-url-template =
  "https://github.com/helix-editor/helix/edit/master/book/{path}"`), and zed wraps the
  mdbook HTML renderer with a Rust post-processor (extras/zed/docs/book.toml,
  `command = "cargo run -p docs_preprocessor -- postprocess"`) deployed by
  extras/zed/.github/workflows/deploy_docs.yml. Reasoning: docs versioned with the
  code, reviewable in the same PR, and buildable by the Rust toolchain contributors
  already have.
- Camp 3, a non-Rust static site generator in-repo: uv and ruff use mkdocs-material
  (extras/uv/mkdocs.yml, extras/ruff/mkdocs.template.yml), starship uses vitepress with
  crowdin-managed locale directories (extras/starship/docs/.vitepress/config.mts,
  extras/starship/crowdin.yml). Reasoning: polished product sites, search, theming, and
  translation pipelines that mdbook does not offer.
- Camp 4, plain Markdown in the repo: ripgrep keeps a 1,025-line GUIDE.md and a
  1,063-line FAQ.md at the root, fd keeps the whole manual in README.md ("How to use"
  with fifteen subsections), gitui splits by topic into KEY_CONFIG.md, THEMES.md, and
  FAQ.md at the root, and bat uses doc/ (assets.md, alternatives.md). Reasoning: zero
  build step, zero hosting, and GitHub renders it fine for a single-binary tool.
- Camp 5, the manual lives outside the repo: deno (docs.deno.com), tauri (tauri.app),
  meilisearch, rustdesk, and nushell all point users at separate documentation repos or
  sites, keeping only contributor and process docs in-tree (for nushell, the in-tree
  remainder is extras/nushell/devdocs/ with FAQ.md, HOWTOS.md, PLATFORM_SUPPORT.md,
  rust_style.md). alacritty is its own sub-camp: the manual is five scdoc man pages
  under extras/alacritty/extra/man/ (alacritty.1.scd, alacritty.5.scd,
  alacritty-bindings.5.scd), compiled in CI so broken docs fail the build. Reasoning:
  product-scale docs teams, or in alacritty's case the conviction that a terminal
  emulator's manual belongs in man.

**How strict the missing_docs lint should be.** Four levels are all represented.
gitui forbids it in its library crate (extras/gitui/asyncgit/src/lib.rs, line 11:
`#![forbid(missing_docs)]`). ripgrep denies it in every published crate
(extras/ripgrep/crates/matcher/src/lib.rs). tokio, tauri, and bevy warn: tokio via
`#![warn(missing_debug_implementations, missing_docs, rust_2018_idioms,
unreachable_pub)]` in extras/tokio/tokio/src/lib.rs, tauri via
`#![warn(missing_docs, rust_2018_idioms)]` at line 55 of
extras/tauri/crates/tauri/src/lib.rs, and bevy workspace-wide with
`missing_docs = "warn"` at line 84 of extras/bevy/Cargo.toml, escalated to an error by
`-D warnings` in CI. Application-shaped projects (fd, bat, alacritty, helix, deno,
starship) simply omit it. The reasoning split is clean: the lint pays for itself exactly
when strangers consume the API through docs.rs; forcing doc comments onto internal
binary modules produces boilerplate, not documentation.

**ARCHITECTURE.md: written or skipped.** Only three projects maintain a dedicated
architecture document: tauri at the root (extras/tauri/ARCHITECTURE.md, which opens with
"What Tauri is NOT" before naming each crate's role), helix at
extras/helix/docs/architecture.md (a crate table: "helix-core: Core editing primitives,
functional." then "This document contains a high-level overview of Helix internals"),
and deno, which goes furthest with both extras/deno/doc/architecture.md and a
directory-by-directory extras/deno/doc/codebase-map.md ("A directory-by-directory tour
of the repository, plus the files worth reading first"). The rest either skip it or
substitute narrower dev docs: bevy's extras/bevy/docs/ holds cargo_features.md,
profiling.md, debugging.md, and linters.md; meilisearch versions its process rules in
extras/meilisearch/documentation/ (release.md, versioning-policy.md,
experimental-features.md). The skip camp's implicit argument is visible in ripgrep:
architecture rationale lives in crate docs instead, where it cannot go stale silently
because rustdoc builds it (see the Matcher design discussion below). The write camp's
argument is scale: at 25 to 250 crates, newcomers need a map before an API reference.

**YAML issue forms versus Markdown templates.** Nine projects use YAML forms with typed
fields and `required: true` validation (ripgrep, fd, helix, rustdesk, zed, uv, ruff,
clap, nushell, plus tauri's bug_report.yml). Seven still use free-form Markdown
templates (deno, bat, starship, meilisearch, bevy, tokio, gitui), and alacritty uses
nothing. The forms camp gets machine-enforced version strings and checkboxes; the
Markdown camp keeps friction low and trusts triage. The trend line is one-directional:
every recently overhauled template in this set is a YAML form.

**README as crate docs versus hand-written crate docs.** One camp makes the README the
crate documentation with `#![doc = include_str!("../README.md")]`, which also compiles
and runs the README's code fences as doc tests: meilisearch's utility crates
(extras/meilisearch/crates/permissive-json-pointer/src/lib.rs, line 1) and several
nushell crates (extras/nushell/crates/nu-command/src/lib.rs) do this. The other camp
writes crate docs by hand and keeps the README separate but honest: clap injects the
README into a hidden struct only under `cfg(doctest)` so its examples are tested without
polluting the docs (excerpt below), and tokio's crate docs are independent prose. The
first approach guarantees a single source of truth; the second acknowledges that a good
README and good API docs address different readers.

## Comparison across the eighteen repositories

| Repository | User manual home | Architecture doc | CONTRIBUTING | missing_docs | rustdoc -D warnings CI | Issue templates | PR template |
|---|---|---|---|---|---|---|---|
| rustdesk | external site | none | docs/ + 20 translations | one lib crate | no | YAML form, blank off | no |
| tauri | external site | ARCHITECTURE.md | .github/ | warn in lib crates | no | YAML + md mix | yes, guidelines |
| deno | external site | doc/architecture.md + codebase-map.md | .github/ | no | doctest flags only | Markdown | yes |
| uv | mkdocs in docs/ | none (STYLE.md instead) | root | few lib crates | no | YAML forms | yes |
| zed | mdbook in docs/ | none | root | gpui and lib crates | doctest job | YAML forms, blank off | yes |
| ripgrep | GUIDE.md + FAQ.md in repo | in crate docs | root, 8 lines | deny, all lib crates | yes, private items too | YAML form | no |
| alacritty | scdoc man pages | none | root | no | no | none | yes |
| bat | README + doc/ | none | root | no | yes | Markdown | no |
| starship | vitepress in docs/, crowdin | none | root | no | no | Markdown, blank off | yes |
| meilisearch | external site | documentation/ (process) | root | no | no | Markdown | yes |
| ruff | mkdocs in docs/ | none | root, large | rare | yes | YAML forms | yes |
| bevy | rustdoc + external learn site | docs/ dev guides | root, pointer to site | warn, workspace-wide | deployed docs build | Markdown incl. docs_improvement | yes, Objective/Solution |
| helix | mdbook in book/ | docs/architecture.md | docs/ | no | yes | YAML form | no |
| fd | README is the manual | none | root | no | no | YAML form | no |
| nushell | external book + devdocs/ | devdocs/ | root | rare | no | YAML forms | yes, release notes |
| tokio | rustdoc is the manual | none | root, large | warn, every crate | yes, plus cargo test --doc | Markdown | yes |
| gitui | topic files at root | none | root | forbid in asyncgit | no | Markdown | yes, checklist |
| clap | rustdoc modules (`_tutorial`, `_faq`) | none | root + per-crate | warn | yes | YAML forms | yes |

## Exemplary excerpts

**Design rationale as crate docs, ripgrep.** The Matcher crate's docs explain not just
what the API is but why it is shaped that way, so the architecture discussion is built
and link-checked on every CI run (extras/ripgrep/crates/matcher/src/lib.rs):

```rust
/*!
This crate provides an interface for regular expressions, with a focus on line
oriented search. ...
A key design decision made in this crate is the use of *internal iteration*,
or otherwise known as the "push" model of searching. In this paradigm,
implementations of the `Matcher` trait will drive search and execute callbacks
provided by the caller when a match is found.
*/
```

**Doc tests hardened at the crate root, tokio.** Doc examples are compiled with
warnings denied so samples cannot rot, and docs.rs metadata is pinned in the manifest
(extras/tokio/tokio/src/lib.rs and extras/tokio/tokio/Cargo.toml):

```rust
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
```

```toml
[package.metadata.docs.rs]
all-features = true
# enable unstable features in the documentation
rustdoc-args = ["--cfg", "docsrs", "--cfg", "tokio_unstable"]
```

**README examples as doc tests without README-as-docs, clap.** The README is attached
to a hidden struct only when doc tests run (extras/clap/src/lib.rs, lines 108 to 110):

```rust
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
```

**Help text as a committed doc artifact, bat.** `--help` output is snapshotted into
doc files and asserted by integration tests, so the CLI reference in the repo can never
drift from the binary (extras/bat/tests/integration_tests.rs):

```rust
fn long_help() {
    test_help("--help", "../doc/long-help.txt");
}
```

**An error catalog compiled as documentation, bevy.** Runtime error codes are Markdown
files attached to marker types with `#[doc = include_str!]`, so the catalog is rendered
by rustdoc and its examples are doc-tested (extras/bevy/errors/src/lib.rs):

```rust
//! Definitions of Bevy's error codes that might occur at runtime.
#[doc = include_str!("../B0001.md")]
pub struct B0001;
```

**A PR template that teaches, tauri.** The template shows good and bad PR titles and
points to the change-file requirement
(extras/tauri/.github/PULL_REQUEST_TEMPLATE.md): "Examples of good title:
fix(windows): fix race condition in event loop ... 3. If this change requires a new
version, then add a change file in `.changes` directory". nushell's template goes
further and harvests a "User-facing changes (Release notes)" section nearly verbatim
into the release blog (extras/nushell/.github/pull_request_template.md), while gitui's
is a four-item checklist ending in "I ran `make check` without errors" and "I added an
appropriate item to the changelog" (extras/gitui/.github/PULL_REQUEST_TEMPLATE.md).

**A documentation style contract, uv.** STYLE.md pins wording, punctuation, and
formatting rules for user-facing text ("Use backticks to escape: commands, code
expressions, package names, and file paths", extras/uv/STYLE.md), and clippy's
`doc-valid-idents` list in extras/uv/clippy.toml (also extras/ruff/clippy.toml) keeps
product names like "PyPI" and "CPython" spelled correctly inside doc comments.

## What a new Rust project should do

- [ ] Write a README as a front door: pitch, screenshot or demo, install, quickstart, links to deeper docs; keep the manual elsewhere once it outgrows a screen or two.
- [ ] Add a CONTRIBUTING.md covering build, test, changelog policy, and PR expectations; even a short one beats none.
- [ ] Put `#![deny(missing_docs)]` (or at least `warn`) on every crate meant to be consumed as a library; skip it for internal binary modules.
- [ ] Gate the docs build in CI with `RUSTDOCFLAGS="-D warnings"`, including `--document-private-items` for internal-doc hygiene.
- [ ] Run doc tests in CI (`cargo test --doc`) and harden them with `#![doc(test(attr(deny(warnings))))]` so examples cannot rot.
- [ ] Compile the README's examples: either `#![doc = include_str!("../README.md")]` for a docs-are-the-README crate, or a `#[cfg(doctest)]` ReadmeDoctests struct.
- [ ] Write an ARCHITECTURE.md (or docs/architecture.md) with a crate table and data-flow overview once the workspace passes a handful of crates; add a codebase map when it passes a dozen.
- [ ] Choose one manual home deliberately: rustdoc modules for a library, mdbook in-repo for a tool, mkdocs or similar for a product site; do not split the manual across all three.
- [ ] Generate every derivable doc (CLI reference, keymap, config schema) from code, commit the output, and fail CI on drift with a message naming the regeneration command.
- [ ] Use YAML issue forms with required version fields and a "I read the docs" checkbox, plus a config.yml routing questions to Discussions.
- [ ] Add a PR template that asks for the issue link, testing done, and a changelog or release-notes entry; keep it short enough that people actually fill it in.
- [ ] Enforce the changelog mechanically: a CI job that requires an added CHANGELOG line referencing the PR, with documented exemptions for non-user-facing changes.
- [ ] Snapshot `--help` output into a committed doc file asserted by a test.
- [ ] Pin `[package.metadata.docs.rs]` (all-features, cfg flags) so docs.rs renders what you intend.
- [ ] Add a docs style contract (STYLE.md) and `doc-valid-idents` entries for product names once user-facing prose accumulates.
