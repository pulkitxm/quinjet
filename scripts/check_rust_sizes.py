#!/usr/bin/env python3
"""Fail when a tracked Rust source exceeds the repository line limit."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

MAX_LINES = 500


def line_count(source: str) -> int:
    """Count logical lines with or without a trailing newline."""
    return len(source.splitlines())


def tracked_rust_files(root: Path) -> list[Path]:
    """List the Rust files Git tracks."""
    listed = subprocess.run(
        ["git", "ls-files", "-z", "*.rs"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return [root / name for name in listed.split("\0") if name]


def repository_root() -> Path:
    """Return the working tree root."""
    return Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )


def selftest() -> int:
    """Prove line counting at the limit and across newline styles."""
    cases = [
        ("", 0),
        ("one", 1),
        ("one\n", 1),
        ("one\ntwo", 2),
        ("one\r\ntwo\r\n", 2),
        ("\n" * MAX_LINES, MAX_LINES),
    ]
    failures = 0
    for source, expected in cases:
        actual = line_count(source)
        if actual != expected:
            failures += 1
            print(
                f"selftest: expected {expected} lines, got {actual}",
                file=sys.stderr,
            )
    if failures:
        return 1
    print(f"check_rust_sizes: {len(cases)} selftest cases pass")
    return 0


def main(argv: list[str]) -> int:
    """Check every tracked Rust file against the line limit."""
    if "--selftest" in argv:
        return selftest()

    root = repository_root()
    files = tracked_rust_files(root)
    oversized = [
        (path.relative_to(root), lines)
        for path in files
        if (lines := line_count(path.read_text(encoding="utf-8"))) > MAX_LINES
    ]
    if oversized:
        for path, lines in oversized:
            print(f"{path}: {lines} lines, limit is {MAX_LINES}", file=sys.stderr)
        return 1

    print(f"check_rust_sizes: {len(files)} Rust file(s) within {MAX_LINES} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
