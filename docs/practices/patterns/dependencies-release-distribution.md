# Dependencies, Releases, and Distribution

This chapter synthesizes how eighteen mature Rust codebases manage the full lifecycle
from dependency intake to shipped binary: lockfile policy, minimal feature selection,
MSRV declaration and verification, automated update bots, changelog discipline,
release automation, binary distribution matrices, and the packaging of shell
completions and man pages. The projects span applications (ripgrep, fd, bat, gitui,
alacritty, helix, nushell, starship, zed, rustdesk), services (meilisearch, deno, uv,
ruff), frameworks (bevy, tauri, tokio), and a library with a CLI-shaped test surface
(clap). Where they agree, the agreement is strong evidence; where they split, the
split almost always tracks whether the artifact is a binary or a library.

## Consensus practices

**Commit the lockfile, build with `--locked`.** Sixteen of eighteen repositories
commit `Cargo.lock`. The two that do not, tokio and bevy, are libraries that must
prove they build against fresh resolutions (`extras/tokio/.gitignore` line 2 and
`extras/bevy/.gitignore` line 17 both list `Cargo.lock`). Application projects treat
the lockfile as the release manifest: `extras/bat/.github/workflows/CICD.yml`
contains twenty `--locked` invocations and fd's release workflow uses
`cargo build --profile $(PROFILE) --locked` in `extras/fd/Makefile`. Meilisearch goes
further and refuses to tag a release unless the lockfile agrees with the manifest
(see the excerpt from `check-release.sh` below).

**Declare MSRV in exactly one place and read it back mechanically.** Fifteen
projects declare `rust-version` in `Cargo.toml` (rustdesk 1.75, tauri 1.90, uv
1.95.0, ripgrep 1.96, alacritty 1.85.0, bat 1.88, starship 1.95, ruff 1.95, bevy
1.96.0, helix 1.90, fd 1.90.0, nushell 1.95.0, tokio 1.71 in
`extras/tokio/tokio/Cargo.toml`, gitui 1.88, clap 1.85). Deno, meilisearch, and zed
instead pin the whole toolchain in `rust-toolchain.toml` (1.95.0, 1.91.1, and 1.97.1
respectively) and treat the pin as the support statement. The consensus refinement
is that CI must never hardcode a second copy of the number: it extracts the value
from the manifest, so a bump is a one-line change.

**Run an update bot with a cooldown.** Fifteen projects run dependabot or Renovate.
The distinctive shared idea is a deliberate delay before adopting a new release, to
let yanked or malicious versions surface. fd sets `cooldown: default-days: 7` in
`extras/fd/.github/dependabot.yml`, meilisearch does the same, tauri sets
`"minimumReleaseAge": "3 days"` in `extras/tauri/renovate.json`, and starship sets
`minimumReleaseAge: '4 days'` in `extras/starship/.github/renovate.json5`.

**Trim features and write down why.** Nearly every manifest disables default
features on heavy dependencies and carries a comment explaining the trim or the pin.
`extras/starship/Cargo.toml` line 43 reads
`# default feature restriction addresses https://github.com/starship/starship/issues/4251`
above its `gix` dependency; `extras/deno/Cargo.toml` line 292 pins
`reqwest = { version = "=0.12.5", ... } # pinned because of https://github.com/seanmonstar/reqwest/pull/1955`;
`extras/meilisearch/crates/meilisearch/Cargo.toml` line 118 explains
`# fixed version due to format breakages in v1.40` above `insta = { version = "=1.39.0" }`.

**Generate completions and man pages from the CLI definition, never by hand.** Every
CLI in the set derives its completion scripts and man page from the same source that
parses arguments: ripgrep from its `Flag` trait registry
(`extras/ripgrep/crates/core/flags/complete/`), bat in its build script
(`extras/bat/build/application.rs`), fd via a hidden `--gen-completions` flag driven
by `extras/fd/Makefile`, starship and uv as subcommands
(`uv generate-shell-completion` is smoke tested by eval in
`extras/uv/.github/workflows/test-smoke.yml` line 43), alacritty and helix from
checked-in files kept honest by tests and packaging metadata.

**Gate the release on tag-equals-manifest.** ripgrep, meilisearch, and gitui all
refuse to build release artifacts when the pushed tag disagrees with `Cargo.toml`.
This one check prevents the classic failure of shipping a binary whose `--version`
does not match its release page.

## Divergent camps

### Lockfile policy: applications commit, libraries do not, clap does both

The application camp (all sixteen binaries and services) commits `Cargo.lock` and
passes `--locked` everywhere, making CI and releases reproducible. The library camp
(tokio, bevy) gitignores it so CI always resolves fresh, catching breakage in the
version ranges the library actually publishes. clap occupies a third position: it is
a library but commits its lockfile anyway and adds a CI job that fails when the
committed lockfile drifts from a fresh resolution, in
`extras/clap/.github/workflows/ci.yml`:

```yaml
  lockfile:
    runs-on: ubuntu-latest
    steps:
    ...
    - name: "Is lockfile updated?"
      run: cargo update --workspace --locked
```

Both library repos compensate for the missing lockfile with `-Z minimal-versions`
jobs (`extras/tokio/.github/workflows/ci.yml` line 794, and clap runs
`cargo +nightly generate-lockfile -Z minimal-versions` at line 192 of its ci.yml),
proving that the lower bounds in their version requirements are honest.

### MSRV verification: five extraction styles, one principle

Every project that verifies MSRV in CI derives the toolchain from the manifest, but
the mechanism varies:

- **cargo metadata plus jq** (fd, bat, bevy). `extras/fd/.github/workflows/CICD.yml`
  line 34: `cargo metadata --no-deps --format-version 1 | jq -r '"msrv=" + .packages[0].rust_version' | tee -a $GITHUB_OUTPUT`.
- **A TOML-reading action** (ruff). `extras/ruff/.github/workflows/ci.yaml` line 530
  uses `SebRollen/toml-action` to read `workspace.package.rust-version`.
- **Shell grep** (alacritty). `extras/alacritty/.github/workflows/ci.yml` line 24:
  `rustup default $(cat Cargo.toml | grep "rust-version" | sed 's/.*"\(.*\)".*/\1/')`.
- **A consistency gate between two declarations** (nushell).
  `extras/nushell/.github/workflows/check-msrv.nu` compares
  `rust-toolchain.toml`'s channel against `workspace.package.rust-version` and exits
  1 on mismatch.
- **A named constant** (tokio: `rust_min: '1.71'` in ci.yml; gitui and clap: a
  literal matrix row, with clap's annotated `rust: "1.85"  # MSRV` so a Renovate
  regex manager can bump it).

Two projects also write the policy down. Helix documents in
`extras/helix/docs/CONTRIBUTING.md`:

```markdown
Helix keeps an intentionally low MSRV for the sake of easy building and packaging
downstream. We follow [Firefox's MSRV policy]. Once Firefox's MSRV increases we
may bump ours as well, but be sure to check that popular distributions like Ubuntu
package the new MSRV version.
```

uv encodes its rolling policy directly in the bot config,
`extras/uv/.github/renovate.json5`:

```json5
      commitMessageTopic: "MSRV",
      // We have a rolling support policy for the MSRV
      // 2 releases back * 6 weeks per release * 7 days per week + 1
      minimumReleaseAge: "85 days",
```

ripgrep adds a per-crate wrinkle: the workspace pins 1.96, but the reusable library
crates keep an older floor for downstream consumers
(`extras/ripgrep/crates/globset/Cargo.toml` and `crates/ignore/Cargo.toml` both
declare `rust-version = "1.88"`).

### Update bots: dependabot for cadence, Renovate for policy

Ten repositories use dependabot (rustdesk, bat, meilisearch, bevy, helix, fd,
nushell, tokio, gitui, and bat again for submodules); six use Renovate (tauri, uv,
zed, starship, ruff, clap); ripgrep, alacritty, and deno use neither and instead
fold `cargo update` review into a release checklist
(`extras/ripgrep/RELEASE-CHECKLIST.md`: "Run `cargo update` and review dependency
updates. Commit updated `Cargo.lock`."). The Renovate camp chooses it for expressive
policy: tauri disables `oxc_*` crates "because of MSRV and PR spam" and groups all
windows-rs crates in `extras/tauri/renovate.json`; uv and zed use custom regex
managers to bump tool versions embedded inside workflow `run:` steps; clap's
Renovate bumps the pinned lint toolchain via a `# STABLE` comment. The dependabot
camp values simplicity plus grouping: gitui groups cargo updates into rolling minor
and patch PRs, helix groups minor and patch weekly, and rustdesk points dependabot
at a git submodule daily (`extras/rustdesk/.github/dependabot.yml`,
`package-ecosystem: "gitsubmodule"`).

### Changelog discipline: hand-written, machine-stamped, or externalized

Three camps exist:

1. **Keep-a-changelog by hand, enforced by CI.** gitui's `CHANGELOG.md` opens with
   the Keep a Changelog preamble and CI extracts the release notes on every PR via
   `ffurrer2/extract-release-notes` (`extras/gitui/.github/workflows/ci.yml` line
   334), so a malformed changelog fails before the release. bat goes further with
   `extras/bat/.github/workflows/require-changelog-for-PRs.yml`, which diffs
   `CHANGELOG.md` against the base branch and greps the added lines for the PR
   number and submitter. fd keeps a permanent `# Unreleased` section at the top with
   per-entry credits; ripgrep keeps a standing `TBD` section
   (`extras/ripgrep/CHANGELOG.md`: "Unreleased changes. Release notes have not yet
   been written."); alacritty states its section ordering rule in the file header:
   "The sections should follow the order `Packaging`, `Added`, `Changed`, `Fixed`
   and `Removed`."
2. **Machine-stamped from structured inputs.** clap uses cargo-release
   `pre-release-replacements` in `extras/clap/Cargo.toml` to rewrite `Unreleased`
   headers, compare links, `CITATION.cff`, and even a doc link in `src/lib.rs` at
   tag time. tauri collects per-PR change files under `extras/tauri/.changes/` with
   a tag taxonomy (`feat`, `bug`, `sec`, `breaking`, ...) defined in
   `.changes/config.json`, and covector assembles per-crate changelogs from them.
   starship generates its changelog from conventional commits via release-please
   (`extras/starship/release-please-config.json`, with `"draft": true`).
3. **No changelog file at all.** deno, meilisearch, zed, and nushell write release
   notes outside the repo (release pages or a blog); nushell harvests PR-template
   release-notes sections nearly verbatim into the release blog, and tokio keeps
   per-crate changelogs such as `extras/tokio/tokio/CHANGELOG.md` rather than a
   root file.

Notably, none of the eighteen uses git-cliff; the projects that want generated
changelogs choose tools coupled to their release automation (release-please,
covector, cargo-release) so the changelog and the version bump cannot drift apart.

### Release automation: three levels of delegation

- **Fully delegated: cargo-dist.** uv and ruff describe their entire release
  pipeline in `dist-workspace.toml` (18 targets each, shell and powershell
  installers, `.tar.gz`/`.zip` archives) and let dist generate the workflows. ruff
  layers governance on top in `extras/ruff/dist-workspace.toml`:

  ```toml
  # Whether CI should trigger releases with dispatches instead of tag pushes
  dispatch-releases = true
  # Whether to enable GitHub Attestations
  github-attestations = true
  ```

  plus a two-person approval environment documented in
  `extras/ruff/.github/workflows/release.yml`: "This environment requires a
  2-factor approval, i.e., the workflow must be approved by another team member."
  uv pairs dist with a `release-prepare.yml` dispatch workflow that runs
  `scripts/release.sh` to open the version-bump PR.
- **Version management delegated, artifacts hand-rolled.** starship lets
  release-please cut the tag and changelog, then a hand-written 13-target matrix
  builds artifacts, publishes to crates.io with OIDC trusted publishing
  (`id-token: write` in `extras/starship/.github/workflows/release.yml`), and only
  flips the draft flag once every artifact and checksum is uploaded
  (`gh release edit ... --draft=false`). tauri's covector and bevy's cargo-release
  (`extras/bevy/.github/workflows/post-release.yml`) sit in this camp too.
- **Fully hand-rolled workflows.** ripgrep, fd, bat, gitui, alacritty, helix,
  meilisearch, nushell, zed, and rustdesk write their own tag-triggered matrix.
  The best of these encode the same safety rails dist gives for free: ripgrep's
  `extras/ripgrep/.github/workflows/release.yml` verifies the tag first:

  ```yaml
      - name: Check that tag version and Cargo.toml version are the same
        shell: bash
        run: |
          if ! grep -q "version = \"$VERSION\"" Cargo.toml; then
            echo "version does not match Cargo.toml" >&2
            exit 1
          fi
  ```

  and meilisearch scripts it in `extras/meilisearch/.github/scripts/check-release.sh`,
  checking both `Cargo.toml` and `Cargo.lock` against `GITHUB_REF`. helix adds a
  preview mode so the release workflow itself can be exercised from a PR without
  tagging (`extras/helix/.github/workflows/release.yml`:
  `preview: ${{ !startsWith(github.ref, 'refs/tags/') || github.repository != 'helix-editor/helix' }}`).
  Version bumps are scripted even here: `extras/fd/scripts/version-bump.sh`,
  `extras/rustdesk/res/bump.sh` (which seds the version across spec files, PKGBUILD,
  pubspec, workflows, and flatpak manifests, then runs `cargo run` to refresh the
  lockfile), and meilisearch's `update-cargo-toml-version.yml` dispatch workflow.

### Binary distribution matrices and supply-chain proof

Release matrices cluster around 13 to 18 targets. fd's matrix in
`extras/fd/.github/workflows/CICD.yml` is representative: 14 targets spanning
gnu/musl Linux (x86_64, i686, aarch64, arm hard-float), both macOS architectures,
and three Windows toolchains including `aarch64-pc-windows-msvc` on `windows-11-arm`,
with `use-cross: true` rows pinned to cross v0.2.5. ripgrep builds 14 targets with a
dedicated `release-lto` profile and generates docs under qemu for foreign
architectures. meilisearch multiplies 6 platforms by 2 editions
(`edition: [community, enterprise]` in
`extras/meilisearch/.github/workflows/publish-release-assets.yml`). Provenance is
now table stakes: fd runs `actions/attest` gated to version tags, helix and ripgrep
use `actions/attest-build-provenance`, ruff enables `github-attestations` in dist,
and rustdesk attaches a Syft CycloneDX SBOM. Prebuilt-binary installs are served by
`[package.metadata.binstall]` tables in fd, nushell, and tauri's CLI crate
(`extras/fd/Cargo.toml`):

```toml
[package.metadata.binstall]
pkg-url = "{ repo }/releases/download/v{ version }/{ name }-v{ version }-{ target }.{ archive-format }"
bin-dir = "{ bin }-v{ version }-{ target }/{ bin }{ binary-ext }"
pkg-fmt = "tgz"
```

### Completions and man pages: generate, verify, package

The strongest pattern is a closed loop: one source of truth generates the artifacts,
a test or CI diff proves the committed copies match, and packaging installs them
into system directories. ripgrep's release job runs the built binary itself:

```yaml
        "$BIN" --generate complete-bash > "$ARCHIVE/complete/rg.bash"
        "$BIN" --generate complete-fish > "$ARCHIVE/complete/rg.fish"
        "$BIN" --generate complete-powershell > "$ARCHIVE/complete/_rg.ps1"
        "$BIN" --generate complete-zsh > "$ARCHIVE/complete/_rg"
        "$BIN" --generate man > "$ARCHIVE/doc/rg.1"
```

(`extras/ripgrep/.github/workflows/release.yml`), producing an archive shaped like:

```text
ripgrep-<version>-<target>/
|-- rg
|-- complete/
|   |-- rg.bash
|   |-- rg.fish
|   |-- _rg
|   `-- _rg.ps1
`-- doc/
    `-- rg.1
```

alacritty checks in its completions but pins them with a unit test at
`extras/alacritty/alacritty/src/cli.rs` line 539 that byte-compares
`extra/completions/*` against `clap_complete` output, and writes its five man pages
in scdoc (`extras/alacritty/extra/man/alacritty.1.scd` and friends), compiled as a
CI docs gate. bat renders both at build time from templates in
`extras/bat/build/application.rs` (`gen_man_and_comp`, with
`cargo:rerun-if-changed=assets/manual/` hooks). helix ships completions through
distro packaging metadata in `extras/helix/helix-term/Cargo.toml`:

```toml
  { source = "../contrib/completion/hx.bash", dest = "/usr/share/bash-completion/completions/hx", mode = "644" },
  { source = "../contrib/completion/hx.fish", dest = "/usr/share/fish/vendor_completions.d/hx.fish", mode = "644" },
```

fd hides the generator behind a default `completions` feature and a Makefile that
also carries the Debian `fdfind` rename variants. starship adds
`clap_complete_nushell` beside `clap_complete` so all six major shells are covered
from one derive.

## Comparison table

| Repository | Cargo.lock | MSRV source | MSRV CI verification | Update bot | Changelog | Release automation |
|---|---|---|---|---|---|---|
| rustdesk | committed | rust-version 1.75 | none found | dependabot (submodule, daily) | none in repo | hand-rolled tag workflows, res/bump.sh |
| tauri | committed | rust-version 1.90 | exact-MSRV toolchain job | Renovate, 3-day age | covector .changes files | covector version-or-publish |
| deno | committed | toolchain pin 1.95.0 | toolchain pin is the check | none | none in repo | scripted, generated workflows |
| uv | committed | rust-version 1.95.0 | pinned toolchain, Renovate-managed | Renovate, 85-day MSRV age | CHANGELOG.md | cargo-dist plus release-prepare dispatch |
| zed | committed | toolchain pin 1.97.1 | toolchain pin is the check | Renovate, weekly | none in repo | hand-rolled v* tag workflow plus nightly |
| ripgrep | committed | rust-version 1.96 (libs 1.88) | pinned MSRV matrix row | none, checklist-driven | manual, standing TBD section | hand-rolled, tag-vs-manifest gate |
| alacritty | committed | rust-version 1.85.0 | grep from Cargo.toml | none | Keep a Changelog, ordered sections | hand-rolled, draft release, human publish |
| bat | committed | rust-version 1.88 | cargo metadata + jq | dependabot, monthly | Keep a Changelog, PR-enforced | hand-rolled single CICD.yml |
| starship | committed | rust-version 1.95 | none dedicated | Renovate, 4-day age | release-please generated | release-please plus 13-target matrix, OIDC publish |
| meilisearch | committed | toolchain pin 1.91.1 | pin mirrored in every job | dependabot, 7-day cooldown | none in repo | hand-rolled, check-release.sh gate, 6x2 matrix |
| ruff | committed | rust-version 1.95 | toml-action read, build on it | Renovate | CHANGELOG.md | cargo-dist, dispatch releases, 2-person gate |
| bevy | not committed | rust-version 1.96.0 | cargo metadata + jq | dependabot | _release-content drafts | cargo-release post-release bump PRs |
| helix | committed | rust-version 1.90 + toolchain | env.MSRV, written Firefox policy | dependabot, grouped weekly | CHANGELOG.md | hand-rolled with preview mode, attestation |
| fd | committed | rust-version 1.90.0 | cargo metadata + jq, clippy+test on MSRV | dependabot, 7-day cooldown | manual, permanent Unreleased | hand-rolled 14-target CICD.yml, attest |
| nushell | committed | rust-version 1.95.0 + toolchain | check-msrv.nu consistency gate | dependabot | none in repo (release blog) | nushell release scripts, WiX, winget |
| tokio | not committed | rust-version 1.71 (per crate) | rust_min env matrix job | dependabot (actions) | per-crate CHANGELOG.md | manual, checklist above version field |
| gitui | committed | rust-version 1.88 (3 places) | literal MSRV matrix row | dependabot, grouped | Keep a Changelog, CI-extracted | hand-rolled cd.yml, homebrew bump |
| clap | committed (library) | rust-version 1.85 | msrv matrix row, minimal-versions, lockfile job | Renovate | cargo-release replacements | cargo-release plus tag notes workflow |

## Exemplary excerpts

**The tag-vs-lockfile release gate**, `extras/meilisearch/.github/scripts/check-release.sh`:

```bash
check_tag() {
    local expected=$1
    local actual=$2
    local filename=$3

    if [[ $actual != $expected ]]; then
        echo >&2 "Error: the current tag does not match the version in $filename: found $actual, expected $expected"
        return 1
    fi
}
```

**MSRV consistency as a hard CI failure**, `extras/nushell/.github/workflows/check-msrv.nu`:

```nu
let toolchain_spec = open rust-toolchain.toml | get toolchain.channel
let msrv_spec = open Cargo.toml | get workspace.package.rust-version

if $toolchain_spec != $msrv_spec {
    print -e "Mismatching rust compiler versions specified in `Cargo.toml` and `rust-toolchain.toml`"
    exit 1
}
```

**Changelog entries as a merge requirement**, `extras/bat/.github/workflows/require-changelog-for-PRs.yml`:

```yaml
      - name: Search for added line in changelog
        run: |
          ADDED=$(git diff -U0 "origin/${PR_BASE}" HEAD -- CHANGELOG.md | grep -P '^\+[^\+].+$')
          grep "#${PR_NUMBER}\\b.*${PR_SUBMITTER}\\b" <<< "$ADDED"
```

**Release mechanics encoded next to the version they release**, `extras/tokio/tokio/Cargo.toml`:

```toml
# When releasing to crates.io:
# - Remove path dependencies (if any)
# - Update doc url
#   - README.md
# - Update CHANGELOG.md.
# - Create "v1.x.y" git tag.
version = "1.53.1"
```

**Facade crates locked in lockstep**, `extras/clap/Cargo.toml`:

```toml
clap_builder = { path = "./clap_builder", version = "=4.6.6", default-features = false }
clap_derive = { path = "./clap_derive", version = "=4.6.4", optional = true }
```

The exact `=` pins guarantee that a `clap` release can never resolve against a
mismatched builder or derive crate, which is the entire point of a facade split.

## What a new Rust project should do

- [ ] Commit `Cargo.lock` for any binary and pass `--locked` to every CI and release cargo invocation; for a library, either gitignore it and add a `-Z minimal-versions` job, or commit it with a `cargo update --workspace --locked` freshness job like clap.
- [ ] Declare `rust-version` once in `Cargo.toml` and have CI extract it (cargo metadata + jq, or a TOML-reading action) to install exactly that toolchain; run clippy and the test suite on it, not just `cargo check`.
- [ ] If you also keep a `rust-toolchain.toml`, add a consistency gate that fails CI when it disagrees with `rust-version`, as nushell does.
- [ ] Write the MSRV bump policy down (rolling window or an external anchor like Firefox's) so bumps are boring.
- [ ] Turn on dependabot or Renovate with a cooldown of 3 to 7 days, group noisy crate families, and let the bot also bump tool versions embedded in workflows via regex managers.
- [ ] Disable default features on heavy dependencies and annotate every pin, trim, or exact version with a comment linking to the issue that forced it.
- [ ] Add unused-dependency detection (cargo-shear, cargo-machete, or cargo-udeps) and a cargo-deny or cargo-audit job with a justified ignore list.
- [ ] Keep a Keep-a-Changelog file with a permanent Unreleased section and enforce entries mechanically per PR, or adopt release automation that generates the changelog from structured inputs; do not do both by hand.
- [ ] Prefer cargo-dist for a standalone binary: one `dist-workspace.toml` buys the target matrix, installers, checksums, and attestations; consider `dispatch-releases` and an approval-gated environment for release control.
- [ ] If hand-rolling the release workflow, gate every job on a tag-equals-`Cargo.toml` (and ideally `Cargo.lock`) check, keep releases draft until all artifacts and checksums upload, and add a preview mode so the workflow is testable from a PR.
- [ ] Script the version bump (one shell script that touches every embedding file and refreshes the lockfile) and keep a committed release checklist.
- [ ] Build at least: gnu and musl Linux on x86_64 and aarch64, both macOS architectures, and x86_64 MSVC Windows; pin cross by version for foreign targets.
- [ ] Sign artifacts with build provenance attestation and publish checksums; publish to crates.io with OIDC trusted publishing instead of a stored token.
- [ ] Generate shell completions and the man page from the clap definition (subcommand or hidden flag), verify the committed copies against generated output in a test, smoke test completions by eval-ing them in a real shell, and package them into release archives and distro metadata.
- [ ] Add `[package.metadata.binstall]` matching the release artifact naming so `cargo binstall` works on day one.
