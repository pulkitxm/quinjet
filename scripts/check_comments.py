#!/usr/bin/env python3
"""Fail when Rust sources carry non-doc comments.

Doc comments (`///`, `//!`, `/** */`, `/*! */`) stay: clap renders them as
`--help` text and rustdoc renders them as documentation. Everything else has to
earn its place through names and structure instead. A short allow list keeps
comments that a tool actually reads.
"""

from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

DIRECTIVES = (
    re.compile(r"^SPDX-[A-Za-z-]+:"),
    re.compile(r"^rustfmt::skip\b"),
    re.compile(r"^grcov-excl-(start|stop|line)\b", re.IGNORECASE),
    re.compile(r"^coverage:(ignore|off|on)\b"),
    re.compile(r"^cargo-deny\b"),
    re.compile(r"^@generated\b"),
)


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    text: str

    def __str__(self) -> str:
        return f"{self.path}:{self.line}: {self.text}"


def comments(source: str) -> list[tuple[int, str, bool]]:
    """Return (line, text, is_doc) for every comment outside string literals."""
    return [(line, text, is_doc) for _, _, line, text, is_doc in spans(source)]


def spans(source: str) -> list[tuple[int, int, int, str, bool]]:
    """Return (start, stop, line, text, is_doc) for every comment in the source."""
    found: list[tuple[int, int, int, str, bool]] = []
    i = 0
    line = 1
    end = len(source)
    while i < end:
        char = source[i]
        if char == "\n":
            line += 1
            i += 1
        elif char == '"':
            i, line = skip_string(source, i, line)
        elif char == "'":
            i, line = skip_char_or_lifetime(source, i, line)
        elif char in "rbc" and (raw := raw_string_start(source, i)) is not None:
            i, line = skip_raw_string(source, raw, line)
        elif source.startswith("//", i):
            start = i
            stop = source.find("\n", i)
            stop = end if stop == -1 else stop
            body = source[start:stop]
            is_doc = body.startswith("///") or body.startswith("//!")
            found.append((start, stop, line, body.strip(), is_doc))
            i = stop
        elif source.startswith("/*", i):
            start = i
            start_line = line
            i, line, body = skip_block_comment(source, i, line)
            is_doc = body.startswith("/**") or body.startswith("/*!")
            found.append((start, i, start_line, body.strip().splitlines()[0], is_doc))
        else:
            i += 1
    return found


def strip(source: str) -> str:
    """Remove every comment the checker would report, leaving the code intact."""
    doomed = [
        (start, stop)
        for start, stop, _, text, is_doc in spans(source)
        if not is_doc and not allowed(text)
    ]
    out = source
    for start, stop in reversed(doomed):
        head = out.rfind("\n", 0, start) + 1
        before = out[head:start]
        after = out[stop:]
        if before.strip() == "" and after.startswith("\n"):
            out = out[:head] + after[1:]
        elif before.strip() == "":
            out = out[:head] + after
        else:
            out = out[:start].rstrip(" \t") + after
    return out


def skip_string(source: str, i: int, line: int) -> tuple[int, int]:
    i += 1
    while i < len(source):
        char = source[i]
        if char == "\\":
            i += 2
            continue
        if char == "\n":
            line += 1
        elif char == '"':
            return i + 1, line
        i += 1
    return i, line


def skip_char_or_lifetime(source: str, i: int, line: int) -> tuple[int, int]:
    literal = re.match(r"'(\\.|[^\\'])'", source[i : i + 6])
    if literal:
        return i + literal.end(), line
    return i + 1, line


def raw_string_start(source: str, i: int) -> tuple[int, int] | None:
    match = re.match(r'(?:b|c)?r(#*)"', source[i : i + 16])
    if not match:
        return None
    return i + match.end(), len(match.group(1))


def skip_raw_string(source: str, raw: tuple[int, int], line: int) -> tuple[int, int]:
    start, hashes = raw
    terminator = '"' + "#" * hashes
    stop = source.find(terminator, start)
    stop = len(source) if stop == -1 else stop + len(terminator)
    return stop, line + source.count("\n", start, stop)


def skip_block_comment(source: str, i: int, line: int) -> tuple[int, int, str]:
    start = i
    depth = 0
    while i < len(source):
        if source.startswith("/*", i):
            depth += 1
            i += 2
        elif source.startswith("*/", i):
            depth -= 1
            i += 2
            if depth == 0:
                break
        else:
            if source[i] == "\n":
                line += 1
            i += 1
    return i, line, source[start:i]


def allowed(text: str) -> bool:
    body = text.lstrip("/").lstrip("*").strip()
    return any(pattern.search(body) for pattern in DIRECTIVES)


def scan(path: Path, source: str) -> list[Finding]:
    return [
        Finding(str(path), line, text)
        for line, text, is_doc in comments(source)
        if not is_doc and not allowed(text)
    ]


def tracked_rust_files(root: Path) -> list[Path]:
    listed = subprocess.run(
        ["git", "ls-files", "-z", "*.rs"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return [root / name for name in listed.split("\0") if name]


def repository_root() -> Path:
    return Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )


def rewrite() -> int:
    root = repository_root()
    changed = 0
    for path in tracked_rust_files(root):
        source = path.read_text(encoding="utf-8")
        stripped = strip(source)
        if stripped != source:
            path.write_text(stripped, encoding="utf-8")
            changed += 1
    print(f"check_comments: stripped comments from {changed} file(s)")
    return 0


def selftest() -> int:
    cases: list[tuple[str, int]] = [
        ("fn main() {}\n", 0),
        ("// plain\nfn main() {}\n", 1),
        ("/// doc\nfn main() {}\n", 0),
        ("//! module doc\n", 0),
        ('let url = "https://example.com";\n', 0),
        ('let raw = r#"// not a comment"#;\n', 0),
        ("let quote = '\"'; // trailing\n", 1),
        ("let tick: &'a str = x;\n", 0),
        ("/* block */\n", 1),
        ("/*! doc block */\n", 0),
        ("/* outer /* nested */ still */\n", 1),
        ("// SPDX-License-Identifier: MIT\n", 0),
        ("// rustfmt::skip\n", 0),
        ('let s = "// fake";\n// real\n', 1),
    ]
    strips: list[tuple[str, str]] = [
        ("fn a() {\n    // gone\n    let x = 1;\n}\n", "fn a() {\n    let x = 1;\n}\n"),
        ("let x = 1; // trailing\n", "let x = 1;\n"),
        ("/// doc\nfn a() {}\n", "/// doc\nfn a() {}\n"),
        ('let s = "// keep";\n', 'let s = "// keep";\n'),
        ("// SPDX-License-Identifier: MIT\n", "// SPDX-License-Identifier: MIT\n"),
        ("fn a() {}\n/* block */\nfn b() {}\n", "fn a() {}\nfn b() {}\n"),
    ]

    failures = 0
    for source, expected in cases:
        actual = len(scan(Path("<selftest>"), source))
        if actual != expected:
            failures += 1
            print(f"selftest: expected {expected}, got {actual} for {source!r}", file=sys.stderr)
    for source, expected_source in strips:
        actual_source = strip(source)
        if actual_source != expected_source:
            failures += 1
            print(
                f"selftest: strip produced {actual_source!r}, expected {expected_source!r}",
                file=sys.stderr,
            )
    if failures:
        return 1
    print(f"check_comments: {len(cases) + len(strips)} selftest cases pass")
    return 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()

    if "--strip" in argv:
        return rewrite()

    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )

    findings: list[Finding] = []
    files = tracked_rust_files(root)
    for path in files:
        findings.extend(scan(path.relative_to(root), path.read_text(encoding="utf-8")))

    if findings:
        for finding in findings:
            print(finding, file=sys.stderr)
        print(f"{len(findings)} disallowed comment(s) in {len(files)} file(s)", file=sys.stderr)
        return 1

    print(f"check_comments: {len(files)} Rust file(s) clean")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
