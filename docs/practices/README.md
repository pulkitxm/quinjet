# Rust Practices

How the most widely used Rust codebases are engineered, distilled from a direct
study of eighteen repositories chosen by GitHub star count, and what that
corpus implies for Quinjet itself.

Each repository was cloned and read directly: manifests, formatting and lint
configuration, CI pipelines, test suites, source idioms, documentation, and
release machinery. Paths cited as `extras/<repo>/<file>` refer to those local
clones, which are ignored by Git; the same file exists at the same path in the
upstream repository.

## Contents

- [Studies](./studies/README.md): one chapter per repository, eighteen in all.
- [Patterns](./patterns/README.md): nine cross-cutting syntheses of what the
  corpus agrees on, where it splits, and why.

- [Gap Analysis](./gap-analysis.md): Quinjet audited against everything the
  study found, with completed recommendations and remaining gaps tracked.

- [Rust Dump](./rust-dump.md): the whole reference bound into one file for
  reading straight through or searching in one place.

## The corpus

| Repository | Stars | Study |
|---|---|---|
| [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | 120,919 | [rustdesk](./studies/rustdesk.md) |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | 110,245 | [tauri](./studies/tauri.md) |
| [denoland/deno](https://github.com/denoland/deno) | 108,251 | [deno](./studies/deno.md) |
| [astral-sh/uv](https://github.com/astral-sh/uv) | 88,771 | [uv](./studies/uv.md) |
| [zed-industries/zed](https://github.com/zed-industries/zed) | 88,670 | [zed](./studies/zed.md) |
| [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | 67,319 | [ripgrep](./studies/ripgrep.md) |
| [alacritty/alacritty](https://github.com/alacritty/alacritty) | 65,390 | [alacritty](./studies/alacritty.md) |
| [sharkdp/bat](https://github.com/sharkdp/bat) | 60,188 | [bat](./studies/bat.md) |
| [starship/starship](https://github.com/starship/starship) | 59,420 | [starship](./studies/starship.md) |
| [meilisearch/meilisearch](https://github.com/meilisearch/meilisearch) | 58,979 | [meilisearch](./studies/meilisearch.md) |
| [astral-sh/ruff](https://github.com/astral-sh/ruff) | 49,222 | [ruff](./studies/ruff.md) |
| [bevyengine/bevy](https://github.com/bevyengine/bevy) | 47,648 | [bevy](./studies/bevy.md) |
| [helix-editor/helix](https://github.com/helix-editor/helix) | 45,833 | [helix](./studies/helix.md) |
| [sharkdp/fd](https://github.com/sharkdp/fd) | 44,095 | [fd](./studies/fd.md) |
| [nushell/nushell](https://github.com/nushell/nushell) | 40,272 | [nushell](./studies/nushell.md) |
| [tokio-rs/tokio](https://github.com/tokio-rs/tokio) | 32,930 | [tokio](./studies/tokio.md) |
| [gitui-org/gitui](https://github.com/gitui-org/gitui) | 22,396 | [gitui](./studies/gitui.md) |
| [clap-rs/clap](https://github.com/clap-rs/clap) | 16,634 | [clap](./studies/clap.md) |

Star counts were recorded in August 2026.

## How to read this

Start with the [patterns](./patterns/README.md) if you want the conclusions:
each one ends with a checklist a new Rust project can apply directly. Reach
into a [study](./studies/README.md) when you want the full context behind a
citation, and read the [gap analysis](./gap-analysis.md) to see the corpus
turned into a concrete status and prioritized plan for this repository.
