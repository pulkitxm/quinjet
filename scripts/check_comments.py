#!/usr/bin/env python3
"""Reject comments in every Git-tracked file that supports comment syntax."""

from __future__ import annotations

import io
import re
import subprocess
import sys
import tokenize
from dataclasses import dataclass
from pathlib import Path

DIRECTIVES = (
    re.compile(r"^SPDX-[A-Za-z-]+:"),
    re.compile(r"^rustfmt::skip\b"),
    re.compile(r"^grcov-excl-(start|stop|line)\b", re.IGNORECASE),
    re.compile(r"^coverage:(ignore|off|on)\b"),
    re.compile(r"^cargo-deny\b"),
    re.compile(r"^@generated\b"),
    re.compile(r"^nosemgrep\b"),
    re.compile(r"^shellcheck\s+disable="),
    re.compile(r"^editorconfig-checker-(disable|enable)$"),
    re.compile(r"^yaml-language-server:\s+\$schema="),
    re.compile(r"^zizmor:\s+ignore\["),
    re.compile(r"^swift-tools-version:"),
    re.compile(r"^swiftlint:"),
    re.compile(r"^swift-format"),
    re.compile(r"^biome-ignore\b"),
    re.compile(r"^@ts-[a-z-]+\b"),
    re.compile(r"^eslint-[a-z-]+\b"),
)

HASH_SUFFIXES = {".ps1", ".psd1", ".rb", ".sh", ".toml", ".yaml", ".yml"}
HASH_NAMES = {".editorconfig", ".gitignore", "CODEOWNERS", "Cargo.lock", "Makefile"}
PLAIN_SUFFIXES = {".asc"}
PLAIN_NAMES = {"CNAME", "LICENSE"}


@dataclass(frozen=True)
class Comment:
    """One comment token and its character range."""

    start: int
    stop: int
    line: int
    text: str
    replacement: str | None = None


@dataclass(frozen=True)
class Finding:
    """One rejected comment, ready to print."""

    path: str
    line: int
    text: str

    def __str__(self) -> str:
        """Render the finding the way a compiler would."""
        return f"{self.path}:{self.line}: {self.text}"


@dataclass
class HashStringState:
    """Quoting state shared across lines in hash-comment formats."""

    quote: str = ""
    triple: str = ""
    escaped: bool = False


def allowed(comment: Comment) -> bool:
    """Say whether a comment carries syntax consumed by a tool."""
    if comment.text.startswith("#!/"):
        return True
    body = comment.text.lstrip("/<#;:!*").strip()
    return any(pattern.search(body) for pattern in DIRECTIVES)


def skip_string(source: str, i: int, line: int) -> tuple[int, int]:
    """Skip a Rust double-quoted string literal."""
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
    """Skip a Rust character literal, or step over a lifetime tick."""
    literal = re.match(r"'(\\.|[^\\'])'", source[i : i + 6])
    if literal:
        return i + literal.end(), line
    return i + 1, line


def raw_string_start(source: str, i: int) -> tuple[int, int] | None:
    """Return where a Rust raw string body starts and its fence width."""
    match = re.match(r'(?:b|c)?r(#*)"', source[i : i + 16])
    if not match:
        return None
    return i + match.end(), len(match.group(1))


def skip_raw_string(source: str, raw: tuple[int, int], line: int) -> tuple[int, int]:
    """Skip a Rust raw string literal."""
    start, hashes = raw
    terminator = '"' + "#" * hashes
    stop = source.find(terminator, start)
    stop = len(source) if stop == -1 else stop + len(terminator)
    return stop, line + source.count("\n", start, stop)


def skip_block_comment(source: str, i: int, line: int) -> tuple[int, int, str]:
    """Skip a nested C-style block comment."""
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


def doc_attribute(text: str) -> str | None:
    """Convert Rust line documentation into its attribute representation."""
    if text.startswith("///"):
        body = text[3:].replace("\\", "\\\\").replace('"', '\\"')
        return f'#[doc = "{body}"]'
    if text.startswith("//!"):
        body = text[3:].replace("\\", "\\\\").replace('"', '\\"')
        return f'#![doc = "{body}"]'
    return None


def c_comments(source: str, *, preserve_rust_docs: bool) -> list[Comment]:
    """Find C-style comments outside Rust-compatible strings."""
    found: list[Comment] = []
    i = 0
    line = 1
    while i < len(source):
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
            stop = len(source) if stop == -1 else stop
            text = source[start:stop]
            replacement = doc_attribute(text) if preserve_rust_docs else None
            found.append(Comment(start, stop, line, text.strip(), replacement))
            i = stop
        elif source.startswith("/*", i):
            start = i
            start_line = line
            i, line, body = skip_block_comment(source, i, line)
            found.append(Comment(start, i, start_line, body.strip().splitlines()[0]))
        else:
            i += 1
    return found


def line_offsets(source: str) -> list[int]:
    """Return the character offset of every line start."""
    offsets = [0]
    offsets.extend(match.end() for match in re.finditer("\n", source))
    return offsets


def python_comments(source: str) -> list[Comment]:
    """Find Python comments with the standard tokenizer."""
    offsets = line_offsets(source)
    found: list[Comment] = []
    for token in tokenize.generate_tokens(io.StringIO(source).readline):
        if token.type != tokenize.COMMENT:
            continue
        start_line, start_column = token.start
        stop_line, stop_column = token.end
        start = offsets[start_line - 1] + start_column
        stop = offsets[stop_line - 1] + stop_column
        found.append(Comment(start, stop, start_line, token.string.strip()))
    return found


def advance_hash_string(source: str, i: int, state: HashStringState) -> int:
    """Advance through one character of a quoted hash-language string."""
    char = source[i]
    advanced = i
    if state.triple:
        advanced = i + 1
        if source.startswith(state.triple, i):
            advanced = i + len(state.triple)
            state.triple = ""
    elif state.quote:
        advanced = i + 1
        if state.escaped:
            state.escaped = False
        elif char == "\\" and state.quote == '"':
            state.escaped = True
        elif char == state.quote:
            state.quote = ""
    elif source.startswith(('"""', "'''"), i):
        advanced = i + 3
        state.triple = source[i : i + 3]
    elif char in "'\"":
        advanced = i + 1
        state.quote = char
    return advanced


def hash_comments(source: str) -> list[Comment]:
    """Find hash comments outside strings in line-oriented formats."""
    found: list[Comment] = []
    state = HashStringState()
    line = 1
    line_start = 0
    i = 0
    while i < len(source):
        char = source[i]
        if char == "\n":
            line += 1
            line_start = i + 1
            state.escaped = False
            i += 1
            continue
        advanced = advance_hash_string(source, i, state)
        if advanced != i:
            i = advanced
            continue
        if char == "#" and (i == line_start or source[i - 1].isspace()):
            stop = source.find("\n", i)
            stop = len(source) if stop == -1 else stop
            found.append(Comment(i, stop, line, source[i:stop].strip()))
            i = stop
            continue
        i += 1
    return found


def semicolon_comments(source: str) -> list[Comment]:
    """Find full-line semicolon comments in EditorConfig files."""
    found: list[Comment] = []
    offset = 0
    for line, text in enumerate(source.splitlines(keepends=True), start=1):
        column = len(text) - len(text.lstrip())
        if text[column:].startswith(";"):
            content = text.rstrip("\r\n")
            stop = offset + len(content)
            found.append(Comment(offset + column, stop, line, content[column:].strip()))
        offset += len(text)
    return found


def powershell_blocks(source: str) -> list[Comment]:
    """Find PowerShell block comments."""
    found: list[Comment] = []
    i = 0
    while (start := source.find("<#", i)) != -1:
        stop_marker = source.find("#>", start + 2)
        stop = len(source) if stop_marker == -1 else stop_marker + 2
        line = source.count("\n", 0, start) + 1
        found.append(Comment(start, stop, line, source[start:stop].strip().splitlines()[0]))
        i = stop
    return found


def ruby_blocks(source: str) -> list[Comment]:
    """Find Ruby block comments."""
    pattern = re.compile(r"(?ms)^=begin(?:\s.*)?$.*?^=end(?:\s.*)?$")
    return [
        Comment(match.start(), match.end(), source.count("\n", 0, match.start()) + 1, "=begin")
        for match in pattern.finditer(source)
    ]


def html_comments(source: str) -> list[Comment]:
    """Find HTML comments in Markdown."""
    found: list[Comment] = []
    i = 0
    while (start := source.find("<!--", i)) != -1:
        marker = source.find("-->", start + 4)
        stop = len(source) if marker == -1 else marker + 3
        line = source.count("\n", 0, start) + 1
        found.append(Comment(start, stop, line, source[start:stop].strip().splitlines()[0]))
        i = stop
    return found


def comments_for(path: Path, source: str) -> list[Comment]:
    """Select the strict scanner for a tracked file."""
    suffix = path.suffix
    if suffix == ".rs":
        return c_comments(source, preserve_rust_docs=True)
    if suffix == ".py":
        return python_comments(source)
    if suffix == ".json":
        return c_comments(source, preserve_rust_docs=False)
    if suffix == ".md":
        return html_comments(source)
    if suffix in HASH_SUFFIXES or path.name in HASH_NAMES:
        found = hash_comments(source)
        if path.name == ".editorconfig":
            found.extend(semicolon_comments(source))
        if suffix in {".ps1", ".psd1"}:
            blocks = powershell_blocks(source)
            found = [
                comment
                for comment in found
                if not any(block.start <= comment.start < block.stop for block in blocks)
            ]
            found.extend(blocks)
        if suffix == ".rb":
            found.extend(ruby_blocks(source))
        return sorted(found, key=lambda comment: comment.start)
    if suffix in PLAIN_SUFFIXES or path.name in PLAIN_NAMES:
        return []
    message = f"unclassified tracked text file: {path}"
    raise ValueError(message)


def tracked_files(root: Path) -> list[Path]:
    """List every file tracked by Git."""
    listed = subprocess.run(
        ["git", "ls-files", "-z"],
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


def read_text(path: Path) -> str | None:
    """Read text while recognizing tracked binary files."""
    data = path.read_bytes()
    if b"\0" in data:
        return None
    return data.decode("utf-8")


def strip_comments(source: str, comments: list[Comment]) -> str:
    """Remove rejected comments and preserve replacement attributes."""
    out = source
    for comment in reversed(comments):
        if allowed(comment):
            continue
        if comment.replacement is not None:
            out = out[: comment.start] + comment.replacement + out[comment.stop :]
            continue
        head = out.rfind("\n", 0, comment.start) + 1
        before = out[head : comment.start]
        after = out[comment.stop :]
        if before.strip() == "" and after.startswith("\n"):
            out = out[:head] + after[1:]
        elif before.strip() == "":
            out = out[:head] + after
        else:
            out = out[: comment.start].rstrip(" \t") + after
    return out


def scan(path: Path, source: str) -> list[Finding]:
    """Report every rejected comment in one file."""
    return [
        Finding(str(path), comment.line, comment.text)
        for comment in comments_for(path, source)
        if not allowed(comment)
    ]


def rewrite() -> int:
    """Strip rejected comments from every tracked text file."""
    root = repository_root()
    changed = 0
    for path in tracked_files(root):
        source = read_text(path)
        if source is None:
            continue
        stripped = strip_comments(source, comments_for(path.relative_to(root), source))
        if stripped != source:
            path.write_text(stripped, encoding="utf-8")
            changed += 1
    print(f"check_comments: stripped comments from {changed} file(s)")
    return 0


def selftest() -> int:
    """Prove every scanner and rewrite behavior on known input."""
    cases: list[tuple[Path, str, int]] = [
        (Path("file.rs"), "fn main() {}\n", 0),
        (Path("file.rs"), "// plain\nfn main() {}\n", 1),
        (Path("file.rs"), "/// docs\nfn main() {}\n", 1),
        (Path("file.rs"), 'let url = "https://example.com";\n', 0),
        (Path("file.rs"), 'let raw = r#"// not a comment"#;\n', 0),
        (Path("file.rs"), "/* outer /* nested */ still */\n", 1),
        (Path("file.py"), "#!/usr/bin/env python3\nvalue = '# text' # gone\n", 1),
        (Path("file.sh"), "#!/bin/sh\nvalue=${name#prefix}\n# gone\n", 1),
        (Path("file.yml"), "key: '# text'\nkey: value # gone\n", 1),
        (Path("file.toml"), 'key = "# text" # gone\n', 1),
        (Path(".editorconfig"), "root = true\n; gone\n", 1),
        (Path("file.ps1"), "<# gone #>\n", 1),
        (Path("file.rb"), "=begin\ngone\n=end\n", 1),
        (Path("file.md"), "text\n<!-- gone -->\n", 1),
        (Path("file.json"), '{"url":"https://example.com"}\n', 0),
        (Path("file.rs"), "// nosemgrep: rule\n", 0),
    ]
    rewrites: list[tuple[Path, str, str]] = [
        (Path("file.rs"), "fn a() {\n    // gone\n}\n", "fn a() {\n}\n"),
        (Path("file.rs"), "/// docs\nfn a() {}\n", '#[doc = " docs"]\nfn a() {}\n'),
        (Path("file.py"), "value = 1 # gone\n", "value = 1\n"),
        (Path("file.yml"), "key: value # gone\n", "key: value\n"),
        (Path("file.md"), "before\n<!-- gone -->\nafter\n", "before\nafter\n"),
        (Path("file.rs"), "// nosemgrep: rule\n", "// nosemgrep: rule\n"),
    ]
    failures = 0
    for path, source, expected in cases:
        actual = len(scan(path, source))
        if actual != expected:
            failures += 1
            print(f"selftest: expected {expected}, got {actual} for {path}", file=sys.stderr)
    for path, source, expected in rewrites:
        actual = strip_comments(source, comments_for(path, source))
        if actual != expected:
            failures += 1
            print(f"selftest: rewrite mismatch for {path}: {actual!r}", file=sys.stderr)
    if failures:
        return 1
    print(f"check_comments: {len(cases) + len(rewrites)} selftest cases pass")
    return 0


def main(argv: list[str]) -> int:
    """Run the checker, stripper, or selftest."""
    if "--selftest" in argv:
        return selftest()
    if "--strip" in argv:
        return rewrite()

    root = repository_root()
    files = tracked_files(root)
    findings: list[Finding] = []
    text_count = 0
    for path in files:
        source = read_text(path)
        if source is None:
            continue
        text_count += 1
        relative = path.relative_to(root)
        try:
            findings.extend(scan(relative, source))
        except (SyntaxError, tokenize.TokenError, UnicodeDecodeError, ValueError) as error:
            print(f"check_comments: {error}", file=sys.stderr)
            return 1

    if findings:
        for finding in findings:
            print(finding, file=sys.stderr)
        message = f"{len(findings)} disallowed comment(s) in {text_count} tracked text file(s)"
        print(message, file=sys.stderr)
        return 1

    print(f"check_comments: {text_count} tracked text file(s) clean")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
