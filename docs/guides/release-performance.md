# Release Size and Interactive Latency

Quinjet remains one portable executable with the complete command tree, syntax catalog,
image decoders, PDF previews, and terminal graphics protocols. Release optimization
measures executable bytes, installation work, process startup, first paint, and steady
rendering independently.

## Reference baseline

The frozen comparison source is
[`fda4cf1`](https://github.com/pulkitxm/quinjet/commit/fda4cf1eb8c19b22bf0394ef3f216e5072c09a70),
version `0.0.69`. Its release profile already selects one code-generation unit, thin LTO,
aborting panics, and symbol stripping. The implicit optimization level is `3`.

The published [v0.0.69 release](https://github.com/pulkitxm/quinjet/releases/tag/v0.0.69)
provides a distribution reference:

| Target | Published executable bytes |
| --- | ---: |
| Linux x86_64, musl | 11,507,240 |
| Linux ARM64, musl | 9,647,128 |
| macOS x86_64 | 9,158,152 |
| macOS ARM64 | 8,089,184 |
| Windows x86_64, MSVC | 9,249,792 |

Published artifacts and locally rebuilt artifacts have different compiler and audit
provenance. Before/after conclusions use two builds of the frozen source and candidate
on the same runner, with the same toolchain and release recipe.

The aspirational size target is strictly less than `5_000_000` executable bytes.
Compressed download size is a separate measurement. An executable of exactly
`5_000_000` bytes fails that target, even when its archive fits comfortably below it.

Native comparisons currently miss that target on all five platforms. The workflow also
enforces separate target-specific regression limits, set about four to five percent above
the measured full-feature release sizes:

| Target | Strict regression limit, executable bytes |
| --- | ---: |
| Linux x86_64, musl | 8,000,000 |
| Linux ARM64, musl | 7,000,000 |
| macOS x86_64 | 6,400,000 |
| macOS ARM64 | 5,500,000 |
| Windows x86_64, MSVC | 6,500,000 |

Passing a regression limit does not mean the five-megabyte target was achieved. Both
limits and the actual size appear in every report.

## Build choices

The release profile uses `opt-level = "z"` and fat LTO. Whole-program linking removes
unused implementations across crate boundaries. Size-oriented optimization substantially
reduces generated code, but also disables loop vectorization, so render, search, and
media workloads require measurement alongside byte counts.

Eight controlled builds of the frozen source compare optimization levels and LTO on
macOS ARM64 with Rust `1.98.1` and cargo-auditable `0.7.7`. Every build retains the same
features, one code-generation unit, aborting panics, and symbol stripping:

| Optimization level | LTO | Executable bytes | Deterministic gzip bytes |
| --- | --- | ---: | ---: |
| `3` | thin | 8,022,416 | 4,276,285 |
| `3` | fat | 7,605,312 | 4,140,248 |
| `2` | thin | 7,561,632 | 4,099,290 |
| `2` | fat | 7,225,472 | 3,988,763 |
| `s` | thin | 7,578,512 | 3,876,289 |
| `s` | fat | 6,351,440 | 3,575,049 |
| `z` | thin | 6,799,200 | 3,738,999 |
| `z` | fat | 5,243,024 | 3,268,365 |

The smallest profile reduces executable bytes by 34.6 percent before code or dependency
changes. These are local compiler experiments, rather than final five-platform results.

Linux x86_64 distribution builds pack relative relocations with GNU ld's
`-z pack-relative-relocs`. Rust `1.93` or newer supplies the musl static-PIE support needed
to start those executables. The release build checks this requirement before linking.
The flag is scoped to that distribution build, so ordinary source installations retain
the Rust `1.88` minimum and other targets keep their native linker behavior.

Runtime dependency features match the product's actual use:

- Syntect loads compiled grammars and themes. HTML generation and YAML/plist grammar
  loading are omitted.
- Ratatui keeps the crossterm backend, layout cache, and underline colors. Unused calendar
  and macro features are omitted.
- Indicatif's in-memory terminal is a test dependency feature.
- Image retains PNG, JPEG, GIF, WebP, BMP, ICO, TIFF, TGA, PNM, and QOI decoding.
- Two-face retains its complete fancy-regex syntax catalog.
- Auditable dependency metadata remains embedded in distribution executables.

Feature removal can shorten compilation without reducing a linked executable. The
linker already discards unused syntax/theme bundles and unreachable library code.
The complete two-face syntax asset occupies `959_011` bytes, while the linked default
theme asset occupies `5_035` bytes.

The panic hook continues restoring terminal state before invoking the standard panic
reporter. Immediate-abort configurations that bypass the hook would violate this
behavior.

## Work kept off the critical path

Terminal entry pushes keyboard enhancement flags without first negotiating support.
Supporting terminals provide CSI-u input; other terminals retain their ordinary key
encoding. Matching teardown and the panic hook restore terminal state.

Image protocol selection uses terminal environment inference and explicit
`QUINJET_IMAGE_PROTOCOL=kitty`, `iterm2`, `sixel`, or `halfblocks` overrides. Font size comes
from the terminal window ioctl when available, with a 10-by-20-pixel fallback. These paths
do not query terminal input.

Inside tmux, passthrough setup runs once through a separate bounded CLI process. A stalled
tmux command is terminated and reaped after a 500-millisecond setup budget. Image encoding
uses the library's protocol constructors without repeating that subprocess during startup
or background preparation.

Explicit `QUINJET_IMAGE_PROTOCOL=auto` retains query-based discovery on unidentified
terminals. A short-lived helper owns terminal input during negotiation and is reaped before
the normal event reader starts. The parent's 500-millisecond deadline starts before spawning
the helper, including process startup in the budget. Termination and reaping can add cleanup
time. The helper uses the library's capability parser without spawning descendants or
changing terminal modes. Failed or timed-out discovery falls back to environment inference
and window geometry.
Successful discovery also supplies the queried font size. This opt-in negotiation cost is
measured separately from immediate native selection.

Human CLI diff rendering skips syntax grammar initialization and regex compilation.
The shared command/session path retains the same text and media previews. JSON and
terminal-interface sessions retain semantic syntax spans. Search requests compile one
matcher and reuse its searcher across files and commit bodies, preserving Unicode,
line-oriented matching, invalid-pattern literal fallback, and UTF-16 BOM decoding.

Native image resizing and protocol encoding run on one lazy background worker. Rendering
shows halfblocks immediately, then repaints when the current native payload is ready.
One coalesced request batch and four pending preparations bound the work. The cache retains
the currently visible encodings plus four offscreen outputs, so a fifth visible image can
also become native without repeated encoding. Raster identity, protocol, dimensions, and
cancellation tickets reject obsolete results after scrolling, resizing, or changing the
displayed document.

## Reproducing executable and startup measurements

The [native comparison workflow](../../.github/workflows/performance.yml) builds the
same five targets as the release workflow. Its production-equivalent command is:

```bash
cargo auditable build --release --locked --target "$TARGET"
```

The Linux x86_64 candidate additionally uses
`RUSTFLAGS='-C link-arg=-Wl,-z,pack-relative-relocs'`. Its baseline has explicitly empty
flags. The report records both flag values, and the packed ELF must contain `DT_RELR`
without a dynamic-library dependency or interpreter.

It records both revisions, `rustc -Vv`, Cargo version, target, runner, dependency metadata,
the render harness digest, and build wall times. Locked sources are fetched before
timing compilation into empty per-revision target directories. Executables are copied
before test builds can activate
development-only dependency features or overwrite build outputs.

Cargo-auditable `0.7.7` records a dev-feature-unified metadata graph. Audit verification
therefore checks the actual normal/build release tree separately, validates dependency
identities and edges, and reports metadata-only entries explicitly. A package appearing
in that metadata is not sufficient evidence that its code is linked.

The [process benchmark](../../scripts/benchmark_release.py) can also run locally:

```bash
python3 scripts/benchmark_release.py \
  --baseline ./baseline-quinjet \
  --candidate ./candidate-quinjet \
  --samples 21 \
  --output comparison.json
```

It creates a new synthetic repository containing 128 modified Rust files, one local
commit, and no remotes. Each executable receives an isolated home, cache, state, Git
configuration, and completion installation. No existing repository content enters the
fixture or public results.

Measurements include:

1. Exact executable bytes, SHA-256, and deterministic gzip level-nine bytes.
2. Fresh-home first-use `--version`, including automatic shell integration.
3. Already-initialized `--version`, `--help`, and `capabilities`.
4. JSON status, a complete 128-file plain-text diff, and a 128-file contents regex search.
5. A 160-by-45 first frame with an explicit Kitty protocol and dark mode. The synthetic
   terminal answers keyboard enhancement, cursor, Kitty graphics, primary attributes,
   pixel geometry, and the terminating device-status query.
6. The same Kitty selection with System appearance and no capability replies.
7. Explicit `auto` on an unidentified terminal, with Sixel capability replies in dark mode.
8. Explicit `auto` with dark mode and no capability replies.

Baseline and candidate invocations alternate order. Each sample launches a fresh process
with warm OS caches. Reports include median, nearest-rank p95, minimum, and sample count.
CLI output must match byte-for-byte. Windows records process measurements without the
POSIX PTY cases. PTY runs verify input-driven clean exit and original terminal-mode
restoration, draining output and retrying quit input for at most three seconds after the
timed frame. Both executables are staged under the same filename so clap's generated
usage text compares the command interface rather than artifact labels.

The synthetic shell is Bash on every platform, including Windows. Fresh-home first use
includes that integration, rather than PowerShell profile discovery. Executable directories
come first on `PATH`, so shortcut setup stays inside the fixture. Hosted installer tests
separately exercise Windows PowerShell integration.

Use `--budget 5000000` to require the strict aspirational byte target. Always preserve the
actual measured result when that target is missed. Run final latency comparisons after
compilers, tests, and other resource-intensive jobs finish.

## Reproducing shell installation measurements

The [installer benchmark](../../scripts/benchmark_install.py) runs the actual shell scripts
and release executables in fresh synthetic homes:

```bash
git show fda4cf1:install.sh > baseline-install.sh
python3 scripts/benchmark_install.py \
  --baseline ./baseline-quinjet \
  --candidate ./candidate-quinjet \
  --baseline-installer ./baseline-install.sh \
  --asset quinjet-macos-aarch64 \
  --samples 21 \
  --output install-comparison.json
```

Choose the published asset for the native host. The transport copies local mock release
assets through a synthetic curl executable, without contacting GitHub or introducing
network delay. Timings include checksum verification, staged binary installation, shell
completion installation, and shortcut creation. Each run verifies the installed completion
file and immediately executes the `q` shortcut. Reports record executable and installer
digests, asset-request order, median, and p95.

Pinned installation downloads checksums and the executable. Latest installation first
resolves one release tag, then downloads both assets from that immutable tag. The extra
HEAD request prevents mismatched assets across a concurrent release. These measurements
describe local installer overhead, rather than end-to-end internet installation latency.
The installers download raw executables; deterministic gzip sizes are compression
comparisons, rather than their actual transfer sizes.

## Reproducing viewport measurements

The ignored release benchmark renders synthetic documents through Ratatui's TestBackend:

```bash
cargo test --release --locked --bin quinjet \
  ui::tests::performance::release_render_timings \
  -- --ignored --exact --nocapture --test-threads=1
```

The workflow copies the identical harness into the frozen baseline and registers its test
module. Both versions render a 160-by-45 viewport over 100, 1,000, and 4,000 collapsed
files, plus a position deep into 100,000 expanded rows. Each case reports first draw and
31 steady draws. This isolates layout and rendering from Git, terminal transport,
filesystem watching, and network latency.

File-header boundaries are built lazily once per document replacement. Fold eligibility
uses cached counts, sticky headers use binary search, and unfiltered list selections
avoid allocation. First-draw costs include cache creation; steady draws reuse it.

## Research and constraints

The investigation distinguishes practical production settings from target-specific
experiments:

| Technique | Constraint |
| --- | --- |
| Additional asset compression | Outer zlib saves only 91,410 bytes from an already lazily compressed syntax pack and adds decompression work. |
| Anyhow without its standard-library feature | A matched trial saves only 128 bytes and changes error/backtrace behavior, so the feature remains enabled. |
| Narrower regex features | Cargo feature unification is additive; a direct dependency cannot subtract features enabled by grep-regex or fancy-regex. |
| RGBA8 conversion before image resizing | A matched trial grows the executable by 16 bytes. Native protocol code still links dynamic resizing, while early conversion changes 16-bit and HDR colors and adds a full-resolution allocation. |
| Removing image codecs or grammars | Changes supported functionality and the comparison workload. |
| RELR relocation packing | Adopted only for Linux x86_64 distribution builds after native static-PIE and audit verification; other targets have different relocation layouts. |
| Identical-code folding | Must be measured per linker; optimized Windows builds already request reference elimination and folding. |
| PGO | Requires representative training per native target and matching LLVM, source, profile, and features. |
| BOLT | Applies to suitable ELF executables, rather than the macOS and Windows distribution formats. |
| Executable packing | Adds launch decompression and does not provide a uniform five-platform release strategy. |
| Rebuilding the standard library | Requires a more complex nightly toolchain and native validation. |
| Dynamic dependencies or host-specific CPUs | Change the portable single-executable distribution contract. |

Linked-code attribution matters more than lockfile package counts. Cargo-bloat provides
approximate crate/function attribution; native section tools reveal constants,
relocations, unwind data, and padding. An analysis build retaining symbols must remain
separate from the stripped executable used for byte and latency comparisons.

The RGBA8 experiment included 108 generated comparisons over all ten pixel
representations, color annotations, format round-trips, transparency, and aspect ratios.
Eight-bit previews matched, but every resized 16-bit and floating-point representation
changed. An HDR TIFF checkerboard changed every output pixel, with a maximum channel
difference of 128. Resizing before conversion preserves the current preview behavior.

References:

- [Cargo release profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Package overrides and generic code](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides-and-generics)
- [Rust Performance Book build configuration](https://nnethercote.github.io/perf-book/build-configuration.html)
- [Minimum-sized Rust techniques and platform constraints](https://github.com/johnthagen/min-sized-rust)
- [Cargo feature unification](https://doc.rust-lang.org/cargo/reference/features.html#feature-unification)
- [Syntect features](https://docs.rs/crate/syntect/5.3.0/features)
- [Image DynamicImage behavior](https://docs.rs/image/0.25.10/image/enum.DynamicImage.html)
- [Rust profile-guided optimization](https://doc.rust-lang.org/rustc/profile-guided-optimization.html)
- [GNU linker options](https://sourceware.org/binutils/docs/ld/Options.html)
- [musl release history](https://musl.libc.org/releases.html)
- [Rust 1.93 bundled-musl update](https://blog.rust-lang.org/2026/01/22/Rust-1.93.0/)
- [BOLT prerequisites](https://github.com/llvm/llvm-project/blob/main/bolt/README.md)
