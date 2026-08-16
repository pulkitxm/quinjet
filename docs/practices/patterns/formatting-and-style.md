# Formatting and Style Across the Rust Ecosystem

Formatting is the least glamorous dimension of engineering practice and the one with the highest
consensus. Across the eighteen repositories studied here (rustdesk, tauri, deno, uv, zed, ripgrep,
alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap), every
single project machine-formats its Rust code and gates it in CI. The interesting variation is not
whether to format but how much configuration to allow, whether nightly-only rustfmt options are
worth the toolchain cost, and how far mechanical style enforcement extends beyond `.rs` files into
TOML, Markdown, YAML, and prose.

## Consensus practices

Four practices appear in effectively all eighteen projects.

**1. rustfmt is the only Rust formatter, and CI checks it.** No project uses an alternative
formatter or leaves formatting to convention. The enforcement command is nearly always
`cargo fmt --all -- --check` or a close variant. ripgrep's job is representative
(extras/ripgrep/.github/workflows/ci.yml):

```yaml
  rustfmt:
    runs-on: ubuntu-latest
    steps:
    ...
        components: rustfmt
    - name: Check formatting
      run: cargo fmt --all --check
```

tokio is the one project that avoids `cargo fmt` itself, for a documented reason
(extras/tokio/.github/workflows/ci.yml):

```yaml
      - name: "rustfmt --check"
        # Workaround for rust-lang/cargo#7732
        run: |
          if ! rustfmt --check --edition 2021 $(git ls-files '*.rs'); then
            printf "Please run \`rustfmt --edition 2021 \$(git ls-files '*.rs')\` to fix rustfmt errors.\nSee CONTRIBUTING.md for more details.\n" >&2
            exit 1
          fi
```

**2. The formatting config, when present, is tiny and version-pinned.** Most configs are one to
three lines, and the dominant content is not a style preference at all: it is an edition or
style_edition pin so that a toolchain upgrade cannot silently reformat the tree. uv, zed, and ruff
all ship the identical two-line file (extras/uv/rustfmt.toml, extras/zed/rustfmt.toml,
extras/ruff/rustfmt.toml):

```toml
edition = "2024"
style_edition = "2024"
```

bevy pins in the other direction, holding style_edition back to avoid churn
(extras/bevy/rustfmt.toml):

```toml
use_field_init_shorthand = true
newline_style = "Unix"
style_edition = "2021"
```

**3. Formatting-only commits are erased from blame.** zed and ruff both commit a
`.git-blame-ignore-revs` file listing bulk-reformat commits. zed documents the intent in the file
itself (extras/zed/.git-blame-ignore-revs):

```text
# This file consists of a list of commits that should be ignored for
# `git blame` purposes. This is useful for ignoring commits that only
# changed whitespace / indentation / formatting, but did not change
# the underlying syntax tree.
```

**4. Spelling of identifiers and docs is machine-checked.** Eight of the eighteen repositories
carry a `typos` configuration (extras/uv/_typos.toml, extras/zed/typos.toml,
extras/starship/typos.toml, extras/ruff/_typos.toml, extras/bevy/typos.toml,
extras/nushell/typos.toml, extras/gitui/typos.toml, extras/clap/typos.toml). tokio goes further
with cargo-spellcheck and a 328-line custom dictionary (extras/tokio/spellcheck.dic, configured in
extras/tokio/spellcheck.toml with `extra_dictionaries = ["spellcheck.dic"]`).

## The two rustfmt camps

### The defaults camp (12 of 18)

The majority position is explicit: rustfmt defaults, zero style overrides, and often a config file
whose only job is to say so. Three projects ship a file that is nothing but a comment or literally
empty:

- extras/bat/rustfmt.toml and extras/fd/rustfmt.toml each contain exactly `# Defaults are used`.
- extras/helix/rustfmt.toml is a zero-byte file.
- extras/starship/.rustfmt.toml spells out why an empty file is better than no file:

```toml
# This file intentionally left almost blank
#
# The empty `rustfmt.toml` makes rustfmt use the default configuration,
# overriding any which may be found in the contributor's home or parent
# folders.
```

That comment names the real function of an empty config: it is a firewall against a contributor's
`~/.rustfmt.toml`. helix pairs its empty file with a toolchain pin so the rustfmt binary itself is
also fixed (extras/helix/rust-toolchain.toml):

```toml
[toolchain]
channel = "1.90.0"
components = ["rustfmt", "rust-src", "clippy"]
```

tokio, clap, and rustdesk have no rustfmt config file at the repository root at all; nushell's
config is the single line `edition = "2024"` (extras/nushell/rustfmt.toml); uv, zed, and ruff pin
editions only; bevy is defaults plus two stable one-liners. The reasoning on this side is
consistent across CONTRIBUTING files and config comments: defaults mean zero onboarding cost, no
debate surface, no nightly requirement, and no reformat churn when contributors' local versions
differ. rustdesk shows how the camp handles a genuine local need without a global rule: only one
library crate opts into one option (extras/rustdesk/libs/enigo/rustfmt.toml is exactly
`wrap_comments = true`).

### The custom camp (6 of 18)

Six projects deliberately diverge, each for a different articulated reason.

**Density**: ripgrep compresses aggressively (extras/ripgrep/rustfmt.toml):

```toml
max_width = 79
use_small_heuristics = "max"
edition = "2024"
```

Both options are stable, so ripgrep gets a distinctive dense style with a plain stable toolchain.
gitui goes even narrower, and is the only project in the set that uses hard tabs
(extras/gitui/rustfmt.toml):

```toml
max_width = 70
hard_tabs = true
newline_style = "Unix"
```

gitui backs the tab decision with an editorconfig so non-rustfmt editors agree
(extras/gitui/.editorconfig):

```ini
root = true
[*.rs]
indent_style = tab
```

**Ecosystem alignment**: tauri and deno both come from mixed Rust and TypeScript codebases and pull
Rust toward web conventions: 2-space indentation and a narrower or explicit width. tauri writes out
every choice, including redundant defaults, so nothing is implicit
(extras/tauri/rustfmt.toml):

```toml
max_width = 100
hard_tabs = false
tab_spaces = 2
newline_style = "Unix"
...
force_explicit_abi = true
```

deno keeps its Rust config to three lines (extras/deno/.rustfmt.toml: `max_width = 80`,
`tab_spaces = 2`, `edition = "2024"`) and layers the rest through dprint, described below.

**Maximal opinion**: alacritty is the far end of the spectrum, with fifteen options including many
unstable ones such as `format_strings`, `normalize_comments`, `wrap_comments`,
`reorder_impl_items`, and `imports_granularity = "Module"` (extras/alacritty/rustfmt.toml).
meilisearch sits nearby with `unstable_features = true`, `use_small_heuristics = "max"`, and the
two import options (extras/meilisearch/.rustfmt.toml).

## Nightly-only formatting options

The custom camp splits again on how it pays for unstable options.

- **Pay openly with a nightly CI job.** alacritty's sourcehut build installs nightly rustfmt just
  to format (extras/alacritty/.builds/linux.yml):

```yaml
  - rustfmt: |
      cd alacritty
      rustup toolchain install nightly -c rustfmt
      cargo +nightly fmt -- --check
```

- **Format with nightly locally, check with stable in CI.** meilisearch's fmt job runs
  `cargo fmt --all -- --check` on a pinned stable 1.91.1 toolchain
  (extras/meilisearch/.github/workflows/test-suite.yml). Stable rustfmt warns and ignores the
  unstable keys, and because the default import behavior is Preserve, already-grouped imports pass
  the stable check untouched. The unstable style is therefore maintained by convention plus
  occasional nightly runs, not enforced per PR.

- **Refuse to pay, and document the deferral.** bevy keeps its wishlist in comments
  (extras/bevy/rustfmt.toml):

```toml
# The following lines may be uncommented on nightly Rust.
# Once these features have stabilized, they should be added to the always-enabled options above.
# unstable_features = true
# imports_granularity = "Crate"
# normalize_comments = true

# these options seem poorly implemented and cause churn, so, try to avoid them
# wrap_comments = true
# comment_width = 100
```

## Import grouping and ordering

Import layout is the single most wanted unstable feature. Four projects configure it; the rest
accept rustfmt's default alphabetical reordering within whatever groups the author wrote
(`reorder_imports` is on by default, and tauri restates it explicitly).

- deno wants one item per `use` and three groups (std, external, crate), and injects the options
  per invocation through dprint's exec plugin rather than the config file
  (extras/deno/.dprint.json):

```json
    "commands": [{
      "command": "rustfmt --config imports_granularity=item --config group_imports=StdExternalCrate",
      "exts": ["rs"],
      "cacheKeyFiles": [
        "rust-toolchain.toml",
        ".rustfmt.toml"
      ]
    }]
```

- meilisearch chooses `imports_granularity = "Module"` with the same `StdExternalCrate` grouping
  (extras/meilisearch/.rustfmt.toml).
- alacritty chooses `imports_granularity = "Module"` without grouping
  (extras/alacritty/rustfmt.toml).
- bevy would choose `imports_granularity = "Crate"` if it were stable (commented block above).

The lesson: item-level granularity (deno) optimizes for conflict-free diffs, module-level
(alacritty, meilisearch) optimizes for compact headers, and `StdExternalCrate` grouping is the
uncontested choice whenever grouping is configured at all.

## editorconfig

Six of eighteen repositories commit a `.editorconfig`: tauri, deno, uv, alacritty, ruff, and gitui.
Its role is to govern the files rustfmt never touches. uv's is the most instructive
(extras/uv/.editorconfig):

```ini
[*]
charset = utf-8
trim_trailing_whitespace = true
end_of_line = lf
indent_style = space
insert_final_newline = true
indent_size = 2

[*.{rs,py,pyi}]
indent_size = 4

[*.snap]
trim_trailing_whitespace = false

[*.md]
max_line_length = 100
```

Two details recur across these files. First, the base indent is 2 spaces for config and web files
with a 4-space override for Rust, matching rustfmt so editors and formatter never disagree
(extras/ruff/.editorconfig does the same). Second, snapshot and golden files are exempted from
whitespace fixing, because trailing whitespace in a captured output is data: uv exempts `*.snap`
and even one specific test file (`crates/uv/tests/help.rs`), and deno unsets the rules for `*.out`
expectation files and vendored Node tests (extras/deno/.editorconfig).

## TOML formatting

TOML manifests are the second most formatted file type, with three tools in play.

- **taplo** is the mainstream choice. tauri runs `taplo fmt --check --diff` as a dedicated CI job
  (extras/tauri/.github/workflows/fmt.yml), and bevy installs a pinned taplo 0.10.0 binary and runs
  the same command with a fix-it hint on failure (extras/bevy/.github/workflows/ci.yml). starship
  uses taplo as a validator rather than a formatter, linting preset files against a schema:
  `taplo lint --schema "file://${GITHUB_WORKSPACE}/.github/config-schema.json" docs/public/presets/toml/*.toml`
  (extras/starship/.github/workflows/format-workflow.yml).
- **tombi** is gitui's alternative, and its config is a model of a justified override
  (extras/gitui/tombi.toml):

```toml
# Keep dependency inline tables on a single line. Multi-line inline tables are
# TOML 1.1 syntax that Cargo on our MSRV (rust 1.88) rejects with
# "invalid inline table", so tombi must not expand them.
[format.rules]
line-width = 220
```

- **dprint's TOML plugin** covers deno and starship as part of their umbrella formatter
  (extras/deno/.dprint.json plugin list includes `toml-0.7.0.wasm`; extras/starship/.dprint.json
  declares a `"toml": {}` section).

## Markdown and YAML linting

No repository writes YAML style rules by hand; instead, three patterns cover prose and config
files.

- **One umbrella formatter.** deno's `.dprint.json` formats TypeScript, JSON, Markdown, TOML, and
  YAML (via the `pretty_yaml` plugin with `"quotes": "preferSingle"`) and shells out to rustfmt for
  `.rs`, so `deno run tools/format.js --check` is the entire formatting gate
  (extras/deno/tools/format.js). starship's dprint config formats Markdown at
  `"lineWidth": 100` (extras/starship/.dprint.json).
- **Prettier for the web-adjacent files.** tauri runs Prettier over JS/TS/MD with a commented
  `.prettierignore` (extras/tauri/.prettierignore explains, for example, that change files are
  hand-written and an IIFE script must not be formatted). zed pins the exact version in a script,
  `PRETTIER_VERSION=3.5.0` (extras/zed/script/prettier), with a one-key config
  `{ "printWidth": 120 }` (extras/zed/.prettierrc). ruff runs Prettier over YAML only, through
  pre-commit (extras/ruff/.pre-commit-config.yaml: `- id: prettier` with `types: [yaml]`).
- **Dedicated Markdown linters.** ruff layers mdformat (with mkdocs and footnote plugins) and
  markdownlint-cli in a priority-ordered pre-commit pipeline
  (extras/ruff/.pre-commit-config.yaml). bevy runs markdownlint through super-linter with a small
  policy file that disables the line-length rule and allowlists `<details>` and `<summary>`
  (extras/bevy/.github/linters/.markdown-lint.yml).

Workflow YAML gets correctness linting rather than style linting: actionlint (ruff via pre-commit
with a shellcheck integration, zed with extras/zed/.github/actionlint.yml for custom runner
labels), check-jsonschema's `check-github-workflows` hook (ruff), and zizmor for security auditing
(ruff, zed, uv). No project in the set uses yamllint.

## Naming conventions

None of the eighteen projects restates Rust's RFC 430 naming rules, because rustc's built-in
`non_snake_case`, `non_camel_case_types`, and `non_upper_case_globals` lints already enforce them.
What projects do write down is vocabulary:

- uv's STYLE.md legislates terminology down to identifier casing (extras/uv/STYLE.md):

```text
2. Use "pre-release", not "prerelease" (except in code, in which case: use `Prerelease`, not
   `PreRelease`; and `prerelease`, not `pre_release`).
```

- uv and ruff teach clippy the correct casing of domain words through `doc-valid-idents`
  (extras/uv/clippy.toml begins `doc-valid-idents = ["PyPI", "PubGrub", "PyPy", "CPython", ...]`),
  so doc comments cannot silently miscase product names.
- Workspace-wide crate prefixes act as a naming convention at the package level: `uv-*` (70
  crates), `bevy_*` (extras/bevy/crates), `helix-*` (extras/helix), `nu-*` (extras/nushell/crates),
  `tauri-*` (extras/tauri/crates). The prefix makes ownership and layering visible in every `use`
  statement.
- typos and cargo-spellcheck close the loop by rejecting misspelled identifiers and doc words, with
  every exception justified: zed's typos.toml carries a 118-line exclusion list and nushell's uses
  regex ignores for box-drawing characters in TUI fixtures.

## File and module size norms

No project enforces a maximum file length with tooling. The observed norm is that production
modules stay in the low thousands of lines and the outliers are either tests or deliberate
single-source-of-truth registries:

- The largest files are overwhelmingly tests: extras/zed/crates/editor/src/editor_tests.rs (43,169
  lines), extras/uv/crates/uv/tests/lock/lock.rs (38,196), extras/deno/tests/integration/lsp_tests.rs
  (22,700), extras/meilisearch/crates/meilisearch/tests/search/multi/proxy.rs (9,712).
- The largest intentional production file is extras/ripgrep/crates/core/flags/defs.rs at 8,161
  lines: every CLI flag as a unit struct in one file, because help text, man page, and completions
  all generate from that single registry and splitting it would scatter the source of truth.
- Projects that keep files small do it by crate granularity, not by file-length rules: zed's
  largest non-test production files sit inside a 250-crate workspace, tokio's biggest source file
  is 2,699 lines, gitui's is 1,959, and starship's is 2,332. fd shows the single-crate version of
  the same discipline: a flat 5k-line src/ with subdirectories only for real subsystems.

The practical norm to extract: keep a production module under roughly 2,000 to 3,000 lines, allow
generated-style registries and test files to grow without limit, and reach for a new module or
crate rather than a file-length lint.

## Comparison table: rustfmt posture

| Repository  | Config file                         | Stance   | Notable settings                                       | Nightly rustfmt needed |
|-------------|-------------------------------------|----------|--------------------------------------------------------|------------------------|
| rustdesk    | none at root (one lib crate only)   | defaults | enigo crate sets `wrap_comments = true`                | no                     |
| tauri       | rustfmt.toml                        | custom   | 2-space, width 100, `force_explicit_abi`               | no                     |
| deno        | .rustfmt.toml + dprint exec flags   | custom   | width 80, 2-space; imports via `--config` flags        | for import options     |
| uv          | rustfmt.toml                        | defaults | edition + style_edition 2024 only                      | no                     |
| zed         | rustfmt.toml                        | defaults | edition + style_edition 2024 only                      | no                     |
| ripgrep     | rustfmt.toml                        | custom   | width 79, `use_small_heuristics = "max"`               | no                     |
| alacritty   | rustfmt.toml                        | custom   | 15 options incl. wrap/normalize comments, Module imports | yes (CI installs it) |
| bat         | rustfmt.toml                        | defaults | `# Defaults are used`                                  | no                     |
| starship    | .rustfmt.toml                       | defaults | intentionally blank, blocks home-dir configs           | no                     |
| meilisearch | .rustfmt.toml                       | custom   | `unstable_features`, Module + StdExternalCrate imports | locally; stable CI     |
| ruff        | rustfmt.toml                        | defaults | edition + style_edition 2024 only                      | no                     |
| bevy        | rustfmt.toml                        | defaults | style_edition 2021 pin; nightly wishlist in comments   | no                     |
| helix       | rustfmt.toml                        | defaults | empty file; rustfmt pinned via rust-toolchain.toml     | no                     |
| fd          | rustfmt.toml                        | defaults | `# Defaults are used`                                  | no                     |
| nushell     | rustfmt.toml                        | defaults | `edition = "2024"` only; fmt enforced by git hook too  | no                     |
| tokio       | none                                | defaults | `rustfmt --check --edition 2021` over git ls-files     | no                     |
| gitui       | rustfmt.toml                        | custom   | width 70, `hard_tabs = true`                           | no                     |
| clap        | none                                | defaults | `cargo fmt --check` on pinned stable                   | no                     |

## Comparison table: non-Rust style enforcement

| Repository  | .editorconfig | TOML formatting            | Markdown / YAML tooling                                  |
|-------------|---------------|----------------------------|----------------------------------------------------------|
| rustdesk    | no            | none                       | Dart analyzer only; none for md/yaml                     |
| tauri       | yes           | taplo fmt --check in CI    | Prettier for JS/TS/MD with commented ignore file         |
| deno        | yes           | dprint TOML plugin         | dprint markdown + pretty_yaml plugins                    |
| uv          | yes           | none                       | Prettier in checks; md width 100 via editorconfig; zizmor |
| zed         | no            | none                       | Prettier 3.5.0 pinned in script; actionlint + zizmor     |
| ripgrep     | no            | none                       | none                                                     |
| alacritty   | yes           | none (editorconfig covers) | none; scdoc man pages compiled as a docs gate            |
| bat         | no            | none                       | none                                                     |
| starship    | no            | dprint TOML + taplo lint   | dprint markdown at lineWidth 100                         |
| meilisearch | no            | none                       | none                                                     |
| ruff        | yes           | none                       | mdformat + markdownlint; Prettier for YAML; actionlint   |
| bevy        | no            | taplo fmt --check in CI    | super-linter markdownlint with policy file               |
| helix       | no            | none                       | none; generated docs drift-checked instead               |
| fd          | no            | none                       | none                                                     |
| nushell     | no            | none                       | typos with regex ignores for TUI artifacts               |
| tokio       | no            | none                       | cargo-spellcheck with 328-line dictionary                |
| gitui       | yes           | tombi with justified width | none                                                     |
| clap        | no            | pre-commit toml/yaml checks | committed (commit lint) + typos                         |

## What a new Rust project should do

- [ ] Commit a rustfmt.toml even if you want defaults. Make it explicit like bat's
      `# Defaults are used` (extras/bat/rustfmt.toml) or starship's commented blank file, so a
      contributor's home config can never leak in.
- [ ] Pin `edition` and `style_edition` in that file (extras/uv/rustfmt.toml pattern) so a
      toolchain bump cannot reformat the tree; when it must, record the bulk commit in
      `.git-blame-ignore-revs` (extras/zed/.git-blame-ignore-revs pattern).
- [ ] Enforce with `cargo fmt --all -- --check` in CI on a pinned toolchain that installs the
      `rustfmt` component; optionally mirror it in a versioned git hook like
      extras/nushell/.githooks/pre-commit.
- [ ] Skip nightly-only options at first. If you want import grouping later, choose
      `group_imports = "StdExternalCrate"` (the unanimous choice where configured) and decide
      openly how to pay: a nightly fmt CI job (alacritty), per-invocation `--config` flags (deno),
      or convention plus a stable check (meilisearch).
- [ ] Add a `.editorconfig` for the files rustfmt does not own: LF, final newline, trimmed
      whitespace, 2-space base indent with a 4-space `[*.rs]` override, and explicit exemptions for
      snapshot or golden files (extras/uv/.editorconfig pattern).
- [ ] Format TOML with taplo (`taplo fmt --check --diff` as in extras/tauri/.github/workflows/fmt.yml);
      if a formatter choice interacts with your MSRV, write the reason into the config the way
      extras/gitui/tombi.toml does.
- [ ] Pick one tool for Markdown and pin its version: dprint if you want an umbrella formatter,
      Prettier via a pinned script (extras/zed/script/prettier), or mdformat plus markdownlint in
      pre-commit (extras/ruff/.pre-commit-config.yaml). Disable the Markdown line-length rule or
      set it deliberately; do not leave it to defaults.
- [ ] Lint workflow YAML for correctness, not style: actionlint with a config for custom runner
      labels (extras/zed/.github/actionlint.yml) plus zizmor for security.
- [ ] Add `typos` with a curated exception file from day one, and `doc-valid-idents` entries in
      clippy.toml for every product name your docs will use (extras/uv/clippy.toml pattern).
- [ ] Write naming and terminology rules only where the compiler cannot: a short STYLE.md for
      user-facing wording and identifier vocabulary (extras/uv/STYLE.md), and consistent crate
      prefixes (`project-*`) once you split into a workspace.
- [ ] Do not add a file-length lint. Keep production modules roughly under 2,000 to 3,000 lines by
      splitting modules and crates, and accept large files only when they are tests or a deliberate
      single source of truth like extras/ripgrep/crates/core/flags/defs.rs.
