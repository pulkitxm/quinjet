#!/usr/bin/env python3
"""Maintain the size and review appendix of the optimization reference."""

from __future__ import annotations

import argparse
import itertools
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DOCS_ROOT = REPO_ROOT / "docs" / "optimization"
EXPECTED_FILES = 23
MIN_LINES = 2_000
MAX_LINES = 3_000
LEDGER_HEADING = "## Optimization review matrix"
EM_DASH = chr(0x2014)

LENSES = (
    "latency",
    "peak memory",
    "network transfer",
    "subprocess count",
    "cache identity",
    "concurrency ordering",
    "failure degradation",
    "reader continuity",
)

CONTEXTS = (
    "a small local repository",
    "a monorepo with many changed paths",
    "a pull request containing generated files",
    "a deeply diverged branch",
    "an unavailable network",
    "rapid keyboard navigation",
    "a linked worktree",
    "cold and warm cache states",
)

SIGNALS = (
    "time to first useful rows",
    "steady frame cost",
    "bytes accepted from child output",
    "Git and gh process count",
    "maximum retained document bytes",
    "cache disposition and complete key",
    "stale reply rejection",
    "visible state after failure",
)


def title(lines: list[str], path: Path) -> str:
    """Read the document title or derive one from the path."""
    if lines and lines[0].startswith("# "):
        return lines[0].removeprefix("# ")
    return path.stem.replace("-", " ").title()


def authored_lines(content: str) -> list[str]:
    """Remove the generated review matrix from a document."""
    lines = content.rstrip().splitlines()
    try:
        boundary = lines.index(LEDGER_HEADING)
    except ValueError:
        return lines
    return lines[:boundary]


def expected_content(path: Path, content: str) -> str:
    """Return the document with enough review rows to satisfy the size floor."""
    lines = authored_lines(content)
    while lines and not lines[-1]:
        lines.pop()
    if len(lines) >= MIN_LINES:
        return "\n".join(lines) + "\n"
    lines.extend(
        [
            "",
            LEDGER_HEADING,
            "",
            (
                "Use this matrix during performance reviews. Each row combines a cost lens, "
                "repository context, and observable signal without claiming that every "
                "combination needs a standalone benchmark."
            ),
            "",
            "| ID | Review condition | Evidence to capture |",
            "| ---: | --- | --- |",
        ]
    )
    combinations = itertools.cycle(itertools.product(LENSES, CONTEXTS, SIGNALS))
    page_title = title(lines, path)
    row = 1
    while len(lines) < MIN_LINES:
        lens, context, signal = next(combinations)
        lines.append(f"| {row} | Check {lens} for {page_title} in {context} | Record {signal} |")
        row += 1
    return "\n".join(lines) + "\n"


def markdown_files() -> list[Path]:
    """Return every optimization page in stable path order."""
    return sorted(DOCS_ROOT.rglob("*.md"))


def write() -> None:
    """Update generated review matrices in place."""
    for path in markdown_files():
        content = path.read_text("utf-8")
        expected = expected_content(path, content)
        if content != expected:
            path.write_text(expected, "utf-8")


def check() -> int:
    """Report stale matrices and violations of the documentation contract."""
    files = markdown_files()
    failures: list[str] = []
    if len(files) != EXPECTED_FILES:
        failures.append(f"expected {EXPECTED_FILES} Markdown files, found {len(files)}")
    for path in files:
        content = path.read_text("utf-8")
        relative = path.relative_to(REPO_ROOT)
        expected = expected_content(path, content)
        if content != expected:
            failures.append(f"out of date: {relative}")
        count = len(content.splitlines())
        if not MIN_LINES <= count <= MAX_LINES:
            failures.append(f"line count outside {MIN_LINES}..{MAX_LINES}: {relative} has {count}")
        if EM_DASH in content:
            failures.append(f"em dash found: {relative}")
    for failure in failures:
        print(failure)
    return int(bool(failures))


def main() -> int:
    """Write or check the optimization documentation."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.check:
        return check()
    write()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
