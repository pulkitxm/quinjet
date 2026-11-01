# Launch gallery

Screenshots and a screen recording of Quinjet 0.0.80, prepared for the Product
Hunt launch on 4 November 2026. Nothing here is generated or mocked up: every
frame is the real binary running.

| File | What it shows | Where it was recorded |
| --- | --- | --- |
| `01-changes.png` | Changes view with a side-by-side syntax-highlighted diff | Synthetic repository |
| `02-pr-checks.png` | A pull request's metadata, description and conversation beside all 72 check runs | This repository, PR #142 |
| `03-ci-log.png` | One check run's steps folded open, with each step's log and duration | This repository, PR #142 |
| `04-history.png` | Branch-scoped history with commit details and a lazily loaded patch | Synthetic repository |
| `05-unified.png` | Unified diff mode over index-first collapsed file headers | Synthetic repository |
| `landing-page.png` | The whole of quinjet.pulkit.page as it deploys | Rendered from `site/` |

## The synthetic repository

Local views are recorded against a generated repository rather than real work,
so no private code appears in published material. It is a fictional usage
analytics monorepo with TypeScript, Rust, Python and Go sources, eleven
commits across three authors, two side branches, and a working tree holding
staged, unstaged, untracked, renamed and deleted paths at once. That mixture is
what makes the Changes view show every resource group in one frame.

Pull request views need real check runs and a real conversation, so those two
frames use this repository's own merged pull request.

## Reproducing

Recorded headlessly with [VHS](https://github.com/charmbracelet/vhs) so the
frames are exact rather than photographed from a terminal:

- 2800 by 1600 pixels, JetBrains Mono Nerd Font at 30 points, no window chrome
- Terminal palette set to the Quinjet dark palette so the edges match the interface
- `quinjet tui --theme quinjet --appearance dark`
- Stills are the last settled frame of a recording, not a timed screenshot,
  which avoids catching the interface mid-repaint
- Published copies are downsampled to 2080 pixels wide and quantized

A Nerd Font is required. Without one the file icons render as replacement
boxes.

## The recording

There is also a screen recording of the same workflow, which is deliberately
not tracked: it is two megabytes, and the repository rejects tracked files over
five hundred kilobytes. Compressing it far enough to fit destroys the legibility
of terminal text, which is the only thing it exists to show. It is regenerated
from a VHS tape rather than kept, and a local copy lives under `extras/`,
which this repository ignores.
