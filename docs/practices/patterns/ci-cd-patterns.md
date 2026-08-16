# CI/CD Patterns

Continuous integration is where a Rust project's engineering values become enforceable. Across the eighteen repositories studied here (rustdesk, tauri, deno, uv, zed, ripgrep, alacritty, bat, starship, meilisearch, ruff, bevy, helix, fd, nushell, tokio, gitui, clap), the CI systems range from a single 464-line file to 39-job generated pipelines, yet a surprisingly stable core of practices repeats. This chapter maps the consensus, the genuine disagreements, and the concrete mechanics worth copying.

## Consensus practices

Nearly every project in the set converges on the following, independent of size or domain.

**Trigger discipline: pull_request plus push-to-default plus workflow_dispatch.** The baseline trigger set appears everywhere from ripgrep (extras/ripgrep/.github/workflows/ci.yml) to fd (extras/fd/.github/workflows/CICD.yml). Docs-only changes are kept out of build lanes with path filters: rustdesk excludes `docs/**`, `README.md`, and packaging directories in extras/rustdesk/.github/workflows/flutter-ci.yml, and tauri goes further with per-crate `paths:` filters in extras/tauri/.github/workflows/test-core.yml.

**Concurrency groups that cancel superseded runs.** Fifteen of the eighteen define a `concurrency:` block. The dominant shape is the one in extras/clap/.github/workflows/ci.yml:

```yaml
concurrency:
  group: "${{ github.workflow }}-${{ github.ref }}"
  cancel-in-progress: true
```

Projects that also build main or run merge queues refine this so only pull request runs are cancellable, as in extras/helix/.github/workflows/build.yml:

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.head_ref || github.run_id }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
```

bevy uses the identical conditional in extras/bevy/.github/workflows/ci.yml. The exceptions are ripgrep, alacritty, bat, fd, and gitui, all of which simply let redundant runs finish.

**Warnings as errors, builds with --locked.** Whether via `RUSTFLAGS: "-D warnings"` at workflow env level (extras/meilisearch/.github/workflows/test-suite.yml, line 14) or clippy invocations like `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (extras/ruff/.github/workflows/ci.yaml, line 325), every project makes warnings fatal somewhere in CI, and every project with a committed lockfile builds `--locked`.

**fail-fast: false on matrices.** rustdesk, ripgrep, fd, gitui, and others disable fail-fast so one broken target does not hide the state of the rest (extras/gitui/.github/workflows/cd.yml, `fail-fast: false`).

**Least-privilege token permissions.** Sixteen of eighteen restrict the `GITHUB_TOKEN`. ripgrep documents the reasoning inline in extras/ripgrep/.github/workflows/ci.yml:

```yaml
# The section is needed to drop write-all permissions that are granted on
# `schedule` event. By specifying any permission explicitly all others are set
# to none. By using the principle of least privilege the damage a compromised
# workflow can do (because of an injection or compromised third party tool or
# action) is restricted.
permissions:
  # to fetch code (actions/checkout)
  contents: read
```

uv is the strictest: extras/uv/.github/workflows/ci.yml opens with `permissions: {}` and every checkout in the repository sets `persist-credentials: false` (130 occurrences under extras/uv/.github). ruff (57), bevy (39), and fd follow the same pattern.

**Scheduled work runs off the PR path.** Sixteen of eighteen carry at least one cron trigger. Only alacritty and fd (plus bat) have none at all.

**Tag-triggered releases that cross-check the version.** Every project that ships binaries triggers its release pipeline on `v*` tags and most verify the tag against the manifest before building, a pattern detailed later in this chapter.

## Workflow architecture: three camps

```text
Workflow architecture
|
+-- Monolith: one file is both CI and CD
|     bat   extras/bat/.github/workflows/CICD.yml   (464 lines, release steps ref-gated)
|     fd    extras/fd/.github/workflows/CICD.yml    (PR, push, tag, dispatch in one file)
|
+-- Orchestrator of reusable workflow_call units
|     uv        extras/uv/.github/workflows/ci.yml calls plan.yml, check-*.yml
|     rustdesk  thin trigger shells call flutter-build.yml (workflow_call, 2477 lines)
|     tauri     21 per-concern workflows, each path-filtered
|
+-- Generated workflows: YAML is a build artifact
      deno  TypeScript ci.ts emits ci.generated.yml, drift-checked
      zed   cargo xtask workflows emits YAML from Rust, drift-checked
```

The monolith camp optimizes for a single source of truth. bat's and fd's CICD.yml begin with a `crate_metadata` job that extracts name, version, and MSRV from `cargo metadata`, so the manifest drives both testing and packaging (extras/fd/.github/workflows/CICD.yml, the `crate_metadata` job). The cost is a long file where release logic and PR logic interleave behind `if: startsWith(github.ref, 'refs/tags/')` guards.

The orchestrator camp splits triggers from logic. rustdesk's extras/rustdesk/.github/workflows/flutter-ci.yml is nothing but a shell:

```yaml
jobs:
  run-ci:
    uses: ./.github/workflows/flutter-build.yml
    with:
      upload-artifact: false
```

The same reusable build workflow (extras/rustdesk/.github/workflows/flutter-build.yml, `on: workflow_call:`) is invoked by the PR shell, the nightly cron shell (flutter-nightly.yml), and the tag shell (flutter-tag.yml), so PRs, nightlies, and releases cannot drift apart. uv applies the same idea at finer granularity: extras/uv/.github/workflows/ci.yml is a pure dispatcher whose jobs are all `uses:` lines, gated by outputs of a change-detection workflow.

The generated camp treats YAML as untrustworthy at scale. deno's extras/deno/.github/workflows/ci.generated.yml opens with `# GENERATED BY ./ci.ts -- DO NOT DIRECTLY EDIT`; the 39-job pipeline, its cache keys, and its aggregation job are all emitted from typed TypeScript in extras/deno/.github/workflows/ci.ts. zed does the same from Rust: extras/zed/.github/workflows/run_tests.yml begins `# Generated from xtask::workflows::run_tests / # Rebuild with 'cargo xtask workflows'.` Both check the generated output for drift in CI, so hand edits fail the build. The payoff is loops, constants, and type checking for pipeline logic; the cost is a second toolchain contributors must learn before touching CI.

## Change detection as a first-class job

The largest repositories do not rely on GitHub's `paths:` filters alone, because a filtered-out workflow cannot satisfy a required check. Instead they run a cheap job that inspects the diff and gates everything else:

- uv's extras/uv/.github/workflows/plan.yml exposes 17 named outputs (`test-code`, `review-security`, `check-schema`, `build-release-binaries`, and so on) that every downstream job in ci.yml consumes via `needs.plan.outputs.*`.
- ruff's `determine_changes` job in extras/ruff/.github/workflows/ci.yaml feeds conditions like `if: ${{ needs.determine_changes.outputs.code == 'true' || github.ref == 'refs/heads/main' }}` (line 311).
- deno's `pre_build` job computes a docs-only fast path and deno_core change detection before the 39-job fan-out (extras/deno/.github/workflows/ci.generated.yml).
- zed's `orchestrate` job computes a nextest `rdeps()` filterset from changed packages, so tests run only for crates that transitively depend on the diff (extras/zed/.github/workflows/run_tests.yml, conditions on `needs.orchestrate.outputs.run_tests`).

Because the gate job always runs, branch protection can require it (or an aggregator over it) without the "skipped counts as passed" trap.

## Job matrices and OS coverage

Coverage philosophy splits by what the project ships:

- **CLI tools ship binaries for everything, so CI builds everything.** ripgrep's test matrix in extras/ripgrep/.github/workflows/ci.yml has 18 entries: pinned MSRV 1.96.0, stable, beta, nightly, musl, i686, aarch64, three armv7 variants, powerpc64, s390x, riscv64gc, macOS, and three Windows toolchains, with foreign architectures running the full test suite under qemu via a version-pinned `cross` binary. fd's release matrix in extras/fd/.github/workflows/CICD.yml lists 14 targets from `arm-unknown-linux-gnueabihf` on ubuntu-24.04 with cross to `aarch64-pc-windows-msvc` on windows-11-arm.
- **Libraries test the compiler and platform lattice.** tokio's extras/tokio/.github/workflows/ci.yml (1420 lines, 45 jobs) spans Linux, Windows, macOS, native ARM runners, FreeBSD VMs, illumos, wasm, and qemu cross-tests, all gated behind a cheap `basics` job. clap crosses OS with feature bundles (minimal, default, next) in extras/clap/.github/workflows/ci.yml.
- **Toolchain rows are part of the matrix.** gitui runs 3 OSes times nightly/stable/MSRV with `continue-on-error: ${{ matrix.rust == 'nightly' }}` (extras/gitui/.github/workflows/ci.yml, lines 18-22), so nightly breakage is visible but not blocking. nushell runs a daily beta-toolchain cron with the same tolerance (extras/nushell/.github/workflows/beta-test.yml).
- **Expensive platforms move off the PR path.** meilisearch's `test-macos` job in extras/meilisearch/.github/workflows/test-suite.yml runs only `if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'` (line 78), keeping macOS minutes out of every PR while still exercising the platform daily.

## Caching: three camps with real reasoning

**Camp 1: Swatinem/rust-cache with write gating (12 of 18).** The action is the default choice (rustdesk, tauri, uv, starship, meilisearch, ruff, helix, nushell, tokio, gitui, clap, and ruff again for wasm). The refinement that separates mature setups is restricting who writes the cache. ruff saves only from main (extras/ruff/.github/workflows/ci.yaml, line 319):

```yaml
- uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2.9.2
  with:
    save-if: ${{ github.ref == 'refs/heads/main' }}
```

tauri saves only from the one matrix leg whose artifacts other legs can reuse: `save-if: ${{ matrix.features.key == 'all' }}` with `key: ${{ matrix.platform.target }}` (extras/tauri/.github/workflows/test-core.yml, lines 93-96). gitui partitions by OS and toolchain with `shared-key: ${{ matrix.os }}-${{ env.cache-name }}-${{ matrix.rust }}` (extras/gitui/.github/workflows/ci.yml, line 32). bevy takes gating to its logical end: PRs use read-only `actions/cache/restore` (extras/bevy/.github/workflows/ci.yml, line 38, with the comment `# key won't match, will rely on restore-keys`), while a dedicated writer workflow, extras/bevy/.github/workflows/update-caches.yml, rebuilds caches on pushes to main and a nightly cron.

**Camp 2: custom caching.** deno builds its own on raw `actions/cache` (136 uses in extras/deno/.github/workflows/) with a bumpable `const cacheVersion = 123;` prefixed into every key (extras/deno/.github/workflows/ci.ts, line 21) and an explicit policy comment: "We force saving a new cache on every main run so that PRs can always be up to date with the freshest information." zed skips artifact caching in favor of a compiler cache, running sccache against a Cloudflare R2 bucket (`SCCACHE_BUCKET: sccache-zed` in extras/zed/.github/workflows/run_tests.yml, line 213). Second-layer caches also appear: helix caches built tree-sitter grammars keyed on `hashFiles('languages.toml')` with a manual bust version (extras/helix/.github/actions/rust-setup/action.yml), and rustdesk layers vcpkg binary caching over rust-cache in extras/rustdesk/.github/workflows/flutter-build.yml.

**Camp 3: deliberately no cache.** ripgrep, fd, bat, and alacritty cache nothing. Every build is from scratch and `--locked` against the committed lockfile (extras/bat/.github/workflows/CICD.yml, extras/fd/.github/workflows/CICD.yml). The reasoning: for a single-crate CLI the clean build is minutes, and a cold-start build is exactly what a packager or contributor experiences, so caching only hides breakage and adds a poisoning surface. This camp correlates strongly with the "small, stable, few dependencies" end of the spectrum.

## Action pinning by SHA

Ten of eighteen pin third-party actions to full 40-character commit SHAs with a human-readable version comment, the form seen in extras/uv/.github/workflows/ci.yml:

```yaml
- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
```

rustdesk (155 pinned uses), deno (385, emitted by the generator), uv (471), zed (393), starship, meilisearch, ruff, bevy, and nushell are all-in. The comment is not decoration: Renovate's `helpers:pinGitHubActionDigests` (zed) and dependabot's github-actions ecosystem (meilisearch, monthly with a 7-day cooldown) parse it to keep the SHA fresh.

The tag camp (tokio at 127 tag-pinned uses, clap, gitui, alacritty) rides major tags like `actions/checkout@v4`, accepting the risk on the grounds that they use few actions, mostly first-party ones. Two hybrid positions are worth noting: tauri pins by SHA only in workflows that hold elevated tokens while routine test workflows use tags, and ripgrep and bat pin exactly the release-critical actions by SHA (`actions/attest-build-provenance@977bb373... # v3.0.0` in extras/ripgrep/.github/workflows/release.yml, the winget publisher in bat) while everything else rides tags. That hybrid is a defensible minimum: the blast radius of a hijacked action is proportional to the token it can reach.

The hardened camp also lints the CI itself: uv runs zizmor as a reusable workflow uploading SARIF (extras/uv/.github/workflows/check-zizmor.yml), ruff runs zizmor and actionlint in extras/ruff/.github/workflows/ci.yaml, zed adds harden-runner egress auditing, and bevy scans workflows with CodeQL (extras/bevy/.github/workflows/security-static-analysis.yml).

## Merge queues and required checks

Five projects have adopted GitHub's merge queue via the `merge_group` trigger: zed (extras/zed/.github/workflows/run_tests.yml, line 9), bevy (extras/bevy/.github/workflows/ci.yml, line 7), helix (extras/helix/.github/workflows/build.yml), meilisearch (extras/meilisearch/.github/workflows/test-suite.yml, line 9), and zed's danger workflow. The sophistication is in what runs inside the queue: since the queue re-tests the merged result, redundant or slow legs are skipped there. meilisearch drops Windows from queue runs (`if: github.event_name != 'merge_group'` on `test-windows`, line 57 of test-suite.yml), and zed skips whole test jobs in the queue with `github.event_name != 'merge_group'` conditions (run_tests.yml, lines 194-374). The other thirteen projects rely on plain branch protection, reasoning that their merge rate does not yet produce the stale-green-check problem queues solve.

For required checks themselves, the standout consensus among large projects is the **single aggregation gate**: one job that `needs:` everything and is the only check branch protection requires, so adding a CI job never requires touching repository settings. Six repositories implement it. clap's is the tersest (extras/clap/.github/workflows/ci.yml, lines 22-32):

```yaml
  ci:
    permissions:
      contents: none
    name: CI
    needs: [test, shell-integration, shell-integration-nu, check, ui, minimal-versions, lockfile, docs, rustfmt, clippy, cffconvert]
    runs-on: ubuntu-latest
    if: "always()"
    steps:
      - name: Failed
        run: exit 1
        if: "contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') || contains(needs.*.result, 'skipped')"
```

bat compresses the check into jq: `jq --exit-status 'all(.result == "success")' <<< '${{ toJson(needs) }}'` (extras/bat/.github/workflows/CICD.yml, line 32) and pairs it with a meta-test, tests/github-actions.rs, that parses the workflow YAML to keep the `needs:` list complete. uv's `required-checks-passed` (extras/uv/.github/workflows/ci.yml, line 294), zed's `tests_pass` (run_tests.yml, line 902), and deno's `ci-status` (ci.generated.yml, line 7588) are the same idea at scale. Note the details that make it correct: `if: always()` so the gate runs even when a dependency fails, and treating `skipped` and `cancelled` as failures so a path filter cannot silently green the build. tokio inverts the topology with the same goal: a cheap `basics` job (fmt, clippy, docs, minrust) must pass before 40+ expensive jobs start (`needs: basics` throughout extras/tokio/.github/workflows/ci.yml, lines 45-57), saving compute on obviously broken PRs.

## Scheduled jobs

Crons carry four distinct workloads across the set:

1. **Advisory audits decoupled from commits.** A new RUSTSEC advisory should surface between merges, not only when someone touches Cargo.toml. tokio runs cargo-deny on both manifest-path pushes and a daily 2 AM cron (extras/tokio/.github/workflows/audit.yml), tauri audits daily, and starship softens the failure mode with `continue-on-error: ${{ matrix.checks == 'advisories' }}` so "the sudden announcement of a new advisory" cannot redden unrelated PRs (extras/starship/.github/workflows/security-audit.yml, lines 20-21).
2. **Nightly builds and toolchain canaries.** gitui rebuilds all platforms nightly and uploads to S3 (extras/gitui/.github/workflows/nightly.yml), nushell publishes tagged nightlies from a synced repo (extras/nushell/.github/workflows/nightly-build.yml), and clap moves beta/nightly and latest-deps testing entirely off the PR path into a monthly cron (extras/clap/.github/workflows/rust-next.yml, `cron: '3 3 3 * *'`).
3. **Flake and fuzz hunting.** meilisearch runs cargo-flaky at 100 iterations every day at 4 AM (extras/meilisearch/.github/workflows/flaky-tests.yml) and a stateful indexing fuzzer (fuzzer-indexing.yml); ruff fuzzes daily (extras/ruff/.github/workflows/daily_fuzz.yaml); deno runs Node.js's own compatibility suite on a weekday cron (extras/deno/.github/workflows/node_compat_test.generated.yml).
4. **Scheduled maintenance that opens PRs.** rustdesk's extras/rustdesk/.github/workflows/update-webpki-roots.yml opens a reviewable cargo-update PR on a schedule under a non-cancelling group (`group: update-webpki-roots`, `cancel-in-progress: false`), and bevy's update-caches.yml refreshes CI caches nightly so PR restores stay warm.

## Release pipelines, signing, and provenance

The release consensus has four load-bearing parts.

**Verify the tag against the manifest before building anything.** ripgrep inlines it (extras/ripgrep/.github/workflows/release.yml, lines 30-40):

```yaml
- name: Check that tag version and Cargo.toml version are the same
  shell: bash
  run: |
    if ! grep -q "version = \"$VERSION\"" Cargo.toml; then
      echo "version does not match Cargo.toml" >&2
      exit 1
    fi
- name: Create GitHub release
  run: gh release create $VERSION --draft --verify-tag --title $VERSION
```

meilisearch scripts it as extras/meilisearch/.github/scripts/check-release.sh, which validates the tag format and matches it against both Cargo.toml and Cargo.lock, and every publish job gates on it.

**Keep releases draft until every artifact exists.** ripgrep creates the release `--draft`, starship keeps releases draft until all 13 target artifacts and checksums upload, and alacritty uploads each platform's artifact to a draft via a small script (extras/alacritty/.github/workflows/upload_asset.sh, called from extras/alacritty/.github/workflows/release.yml) so a human publishes only after every OS job finishes.

**Sign or attest what you ship.** `actions/attest-build-provenance` appears in ripgrep (release.yml, line 288, SHA-pinned), fd (extras/fd/.github/workflows/CICD.yml, gated on `refs/tags/v[0-9]`), and helix, which adds a preview mode so the release pipeline is testable from PRs: `uses: actions/attest-build-provenance@v4` with `if: env.preview == 'false'` (extras/helix/.github/workflows/release.yml, lines 267-271). starship publishes to crates.io with OIDC trusted publishing instead of a stored token (`permissions: id-token: write` plus `rust-lang/crates-io-auth-action` in extras/starship/.github/workflows/release.yml, lines 331-342). rustdesk attaches a Syft-generated CycloneDX SBOM to every release (`syft dir:. -o cyclonedx-json=rustdesk.sbom.json` in extras/rustdesk/.github/workflows/flutter-build.yml, lines 56-73). meilisearch signs its multi-arch Docker publishing path with OIDC (extras/meilisearch/.github/workflows/publish-docker-images.yml).

**Or outsource the whole pipeline.** uv and ruff hand release engineering to cargo-dist: extras/ruff/.github/workflows/release.yml opens with `# This file was autogenerated by dist: https://axodotdev.github.io/cargo-dist` and dist-workspace.toml (extras/ruff/dist-workspace.toml, `cargo-dist-version = "0.31.0"`) declares the 18-target matrix, installers, and attestations declaratively. ruff adds a dispatch-triggered release with a two-person approval environment on top.

## Comparison table

| Repo | Action pinning | Cargo caching | Merge queue | Aggregator gate | Scheduled jobs | Release signing / provenance |
|---|---|---|---|---|---|---|
| rustdesk | SHA + comment (155) | rust-cache + vcpkg + clear-cache workflow | no | no | nightly build, scheduled dep-update PR | Syft SBOM, secret-gated code signing |
| tauri | tags, SHA for elevated tokens | rust-cache, save-if on all-features leg | no | no | daily audit + cargo-vet | covector publish behind 3-OS suite |
| deno | SHA (generated, 385) | custom actions/cache, cacheVersion const, save on main | no | ci-status | weekday Node compat, daily crons | OIDC (id-token) publish jobs |
| uv | SHA (471), permissions {} | rust-cache, save-if gating | no | required-checks-passed | daily cron | cargo-dist, 18 targets, attestations |
| zed | SHA (393), harden-runner | sccache to R2 | yes, heavy jobs skipped | tests_pass | nightly builds every few hours | release on v* tags, drafted notes |
| ripgrep | tags, SHA for attest | none, deliberate | no | no | nightly cron CI | tag==version check, attest-build-provenance |
| alacritty | tags (checkout only) | none | no | no | none (sourcehut nightly fmt) | draft release via upload_asset.sh |
| bat | tags, SHA for winget | none, --locked | no | all-jobs (jq) | none | ref-gated release matrix in CICD.yml |
| starship | SHA (56) | rust-cache | no | no | daily security audit | release-please, OIDC crates.io, SignPath |
| meilisearch | SHA (108) | rust-cache by feature matrix | yes, Windows skipped | no | daily flaky hunt, fuzzer, macOS suite | check-release.sh gate, OIDC Docker |
| ruff | SHA (259), zizmor | rust-cache, save-if main | no | via determine_changes gating | daily fuzz, scheduled reports | cargo-dist, approval-gated, attestations |
| bevy | SHA (128), CodeQL | restore-only in PRs, writer workflow | yes | no | nightly cache refresh, daily cron | docs deploy only, crates via cargo-release |
| helix | mixed (10 SHA, 15 tag) | rust-cache via composite + grammar cache | yes | no | nightly cron | attest-build-provenance, preview mode |
| fd | mixed, persist-credentials false | none, --locked | no | no | none | attest gated on version tags, 14 targets |
| nushell | SHA (40) | rust-cache (single use) | no | no | nightly build, daily beta test, audit | SHA256SUMS, winget, nushell release scripts |
| tokio | tags (127) | rust-cache (49 uses) | no | basics gate (inverse) | daily cargo-deny | crates.io only (library) |
| gitui | tags (25) | rust-cache by os+toolchain | no | no | two nightly crons to S3 | cd.yml on tags, contents: write only |
| clap | tags (37) | rust-cache (12) | no | ci (needs + always()) | monthly rust-next + audit | cargo-release replacements + tag workflow |

## Exemplary excerpts

**One reusable build, three trigger shells** (extras/rustdesk/.github/workflows/flutter-build.yml):

```yaml
name: Build the flutter version of the RustDesk

on:
  workflow_call:
    inputs:
      upload-artifact:
        type: boolean
        default: true
```

**A gate job before the expensive fan-out** (extras/tokio/.github/workflows/ci.yml, lines 44-57):

```yaml
  # Basic actions that must pass before we kick off more expensive tests.
  basics:
    name: basic checks
    runs-on: ubuntu-latest
    needs:
      - clippy
      - fmt
      - docs
      - minrust
```

**Read-only caches for pull requests** (extras/bevy/.github/workflows/ci.yml, lines 38-44):

```yaml
- uses: actions/cache/restore@27d5ce7f107fe9357f9df03efb73ab90386fccae # v5.0.5
  with:
    # key won't match, will rely on restore-keys
    key: ${{ runner.os }}-stable--${{ hashFiles('**/Cargo.toml') }}-
    # See .github/workflows/update-caches.yml for how keys are generated
    restore-keys: |
      ${{ runner.os }}-stable--${{ hashFiles('**/Cargo.toml') }}-
```

**Advisory noise kept out of PR status** (extras/starship/.github/workflows/security-audit.yml, lines 20-21):

```yaml
    # Prevent sudden announcement of a new advisory from failing ci:
    continue-on-error: ${{ matrix.checks == 'advisories' }}
```

**A composite action as the shared CI entry point** (extras/helix/.github/actions/rust-setup/action.yml):

```yaml
name: Rust setup
description: Install a Rust toolchain and warm the cargo + tree-sitter grammar caches.
runs:
  using: composite
  steps:
    - uses: dtolnay/rust-toolchain@3c5f7ea28cd621ae0bf5283f0e981fb97b8a7af9 # master
    - uses: Swatinem/rust-cache@c19371144df3bb44fab255c43d04cbc2ab54d1c4 # v2.9.1
      with:
        shared-key: ${{ inputs.cache-key }}
```

## What a new Rust project should do

- [ ] Trigger CI on `pull_request`, push to the default branch, and `workflow_dispatch`; add `paths-ignore` for docs the way extras/rustdesk/.github/workflows/flutter-ci.yml does.
- [ ] Add a workflow-level `concurrency:` group keyed on workflow plus ref, with `cancel-in-progress` conditional on `github.event_name == 'pull_request'` (extras/helix/.github/workflows/build.yml).
- [ ] Set top-level `permissions: contents: read` (or `permissions: {}` with per-job escalation) and `persist-credentials: false` on every checkout, following extras/uv/.github/workflows/ci.yml and extras/ripgrep/.github/workflows/ci.yml.
- [ ] Pin every third-party action to a full commit SHA with a `# vX.Y.Z` comment, and let Renovate or a github-actions dependabot ecosystem refresh the pins; at minimum pin anything reachable from an elevated token, as ripgrep and tauri do.
- [ ] Lint the CI itself: zizmor and actionlint jobs modeled on extras/uv/.github/workflows/check-zizmor.yml and extras/ruff/.github/workflows/ci.yaml.
- [ ] Build a matrix over ubuntu, windows, and macos plus a pinned-MSRV row read from `Cargo.toml` via `cargo metadata`, with `fail-fast: false` (extras/fd/.github/workflows/CICD.yml, extras/gitui/.github/workflows/ci.yml). Add beta or nightly rows only with `continue-on-error`.
- [ ] Use Swatinem/rust-cache with `save-if` restricted to the default branch (extras/ruff/.github/workflows/ci.yaml, line 319) and keys partitioned by OS, toolchain, and feature set; skip caching entirely only if a cold build is under a few minutes and you value reproducibility more.
- [ ] Create a single always-running aggregation job (`needs` everything, `if: always()`, fail on `failure`, `cancelled`, or unexpected `skipped`) and make it the only required check, copying extras/clap/.github/workflows/ci.yml or extras/bat/.github/workflows/CICD.yml.
- [ ] If PR throughput warrants a merge queue, add `merge_group:` to the trigger list and skip redundant heavy legs inside the queue, as extras/meilisearch/.github/workflows/test-suite.yml does for Windows.
- [ ] Add scheduled jobs for the work that should not wait for a commit: a daily cargo-deny or rustsec audit (extras/tokio/.github/workflows/audit.yml), a nightly or monthly beta-toolchain canary (extras/clap/.github/workflows/rust-next.yml), and flake or fuzz hunting once the suite is large (extras/meilisearch/.github/workflows/flaky-tests.yml).
- [ ] Trigger releases only on `v*` tags, verify tag equals manifest version before building (extras/ripgrep/.github/workflows/release.yml, extras/meilisearch/.github/scripts/check-release.sh), and keep the release draft until all artifacts and checksums are attached.
- [ ] Attest release artifacts with `actions/attest-build-provenance` gated on version tags (extras/fd/.github/workflows/CICD.yml, extras/helix/.github/workflows/release.yml), publish to registries with OIDC trusted publishing instead of stored tokens (extras/starship/.github/workflows/release.yml), and consider attaching an SBOM (extras/rustdesk/.github/workflows/flutter-build.yml).
- [ ] Give the release workflow a preview mode runnable from a PR, as extras/helix/.github/workflows/release.yml does, so the pipeline is tested before the tag exists.
- [ ] When the pipeline outgrows one file, split trigger shells from a `workflow_call` core (extras/rustdesk/.github/workflows/flutter-ci.yml); when it outgrows hand-written YAML, generate it and drift-check the output (extras/deno/.github/workflows/ci.ts, extras/zed/.github/workflows/run_tests.yml).
