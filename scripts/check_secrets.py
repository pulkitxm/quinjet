#!/usr/bin/env python3
"""Fail when a tracked file looks like it carries a credential."""

from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

SELF = "scripts/check_secrets.py"

PLACEHOLDER = re.compile(
    r"""(?ix)
    XXXX | EXAMPLE | PLACEHOLDER | REDACTED | CHANGEME | DUMMY | \*{3}
    | <[A-Za-z_-]+> | \.\.\. | \$\{ | \{\{
    | : (?: secret | password | passwd | pass | token | deploy-key ) @
    """
)


@dataclass(frozen=True)
class Rule:
    name: str
    pattern: re.Pattern[str]
    placeholders_ok: bool = True


RULES = (
    Rule("private key block", re.compile(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----"), False),
    Rule("github token", re.compile(r"\bgh[pousr]_[A-Za-z0-9]{20,}")),
    Rule("github fine-grained token", re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}")),
    Rule("aws access key id", re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b")),
    Rule("slack token", re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}")),
    Rule("google api key", re.compile(r"\bAIza[0-9A-Za-z_-]{35}\b")),
    Rule("openai key", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}")),
    Rule("anthropic key", re.compile(r"\bsk-ant-[A-Za-z0-9_-]{20,}")),
    Rule("crates.io token", re.compile(r"\bcio[A-Za-z0-9]{28,}\b")),
    Rule("npm token", re.compile(r"\bnpm_[A-Za-z0-9]{30,}\b")),
    Rule("stripe key", re.compile(r"\b[sr]k_live_[A-Za-z0-9]{20,}\b")),
    Rule(
        "generic secret assignment",
        re.compile(
            r"""(?ix)
            \b(?:api[_-]?key|secret|password|passwd|token|private[_-]?key)
            \s*["']?\s*[:=]\s*["'][A-Za-z0-9+/_-]{24,}={0,2}["']
            """
        ),
    ),
    Rule("url with inline credentials", re.compile(r"\b[a-z][a-z0-9+.-]*://[^/\s:@]+:[^/\s:@]+@")),
)

SKIP_SUFFIXES = (".png", ".jpg", ".jpeg", ".gif", ".ico", ".pdf", ".woff", ".woff2", ".zip")


def scan(text: str, name: str) -> list[str]:
    if "\0" in text:
        return []
    findings = []
    for rule in RULES:
        for match in rule.pattern.finditer(text):
            hit = match.group(0)
            if rule.placeholders_ok and PLACEHOLDER.search(hit):
                continue
            line = text.count("\n", 0, match.start()) + 1
            findings.append(f"{name}:{line}: {rule.name}")
    return findings


def selftest() -> int:
    cases: list[tuple[str, int]] = [
        ("nothing to see here\n", 0),
        ("-----BEGIN OPENSSH PRIVATE KEY-----\n", 1),
        ("token = ghp_" + "a" * 36 + "\n", 1),
        ("aws = AKIAIOSFODNN7EXAMPLE\n", 0),
        ('api_key: "' + "b" * 32 + '"\n', 1),
        ('api_key: "${QUINJET_TOKEN}"\n', 0),
        ("https://deploy:8f3a9c2b1d4e6f7a0b5c@example.com\n", 1),
        ("https://user:secret@github.com/acme/widget.git\n", 0),
        ("https://example.com/path\n", 0),
        ("let repo = \"https://github.com/pulkitxm/quinjet\";\n", 0),
    ]
    failures = 0
    for text, expected in cases:
        actual = len(scan(text, "<selftest>"))
        if actual != expected:
            failures += 1
            print(f"selftest: expected {expected}, got {actual} for {text!r}", file=sys.stderr)
    if failures:
        return 1
    print(f"check_secrets: {len(cases)} selftest cases pass")
    return 0


def main(argv: list[str]) -> int:
    if "--selftest" in argv:
        return selftest()

    root = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    listed = subprocess.run(
        ["git", "ls-files", "-z"], cwd=root, check=True, capture_output=True, text=True
    ).stdout

    findings: list[str] = []
    scanned = 0
    for name in listed.split("\0"):
        if not name or name == SELF or name.endswith(SKIP_SUFFIXES):
            continue
        try:
            text = (root / name).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        scanned += 1
        findings.extend(scan(text, name))

    if findings:
        for finding in findings:
            print(finding, file=sys.stderr)
        print(f"{len(findings)} potential secret(s) found", file=sys.stderr)
        return 1

    print(f"check_secrets: {scanned} tracked file(s) clean")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
