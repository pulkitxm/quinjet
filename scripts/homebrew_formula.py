#!/usr/bin/env python3
"""Render the Homebrew formula for a published release.

`Formula/quinjet.rb` is the authored source: it carries the whole
formula with one placeholder per release-specific value. The release workflow
runs this script against the `SHA256SUMS` file it just generated and pushes the
rendered formula to the tap repository, so nothing about the formula is ever
hand-edited.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "Formula" / "quinjet.rb"

ASSETS = {
    "SHA256_MACOS_AARCH64": "quinjet-macos-aarch64",
    "SHA256_MACOS_X86_64": "quinjet-macos-x86_64",
    "SHA256_LINUX_AARCH64": "quinjet-linux-aarch64",
    "SHA256_LINUX_X86_64": "quinjet-linux-x86_64",
}

VERSION = re.compile(r"^\d+\.\d+\.\d+$")
CHECKSUM = re.compile(r"^[0-9a-f]{64}$")
PLACEHOLDER = re.compile(r"@[A-Z0-9_]+@")
CHECKSUM_FIELDS = 2


def read_checksums(text: str) -> tuple[dict[str, str], str | None]:
    """Map every asset name in a `sha256sum` listing to its checksum."""
    checksums: dict[str, str] = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        fields = line.split()
        if len(fields) != CHECKSUM_FIELDS:
            return {}, f"unreadable checksum line: {line}"
        digest, name = fields[0], fields[1].lstrip("*")
        if not CHECKSUM.match(digest):
            return {}, f"{name} has no sha-256 checksum: {digest}"
        if name in checksums:
            return {}, f"duplicate checksum for {name}"
        checksums[name] = digest
    return checksums, None


def render(template: str, version: str, checksums: dict[str, str]) -> tuple[str, str | None]:
    """Substitute one release into the formula template."""
    if not VERSION.match(version):
        return "", f"not a stable release version: {version}"
    if template.count("@VERSION@") != 1:
        return "", "the template must carry exactly one @VERSION@ placeholder"
    formula = template.replace("@VERSION@", version)
    for placeholder, asset in ASSETS.items():
        token = f"@{placeholder}@"
        if template.count(token) != 1:
            return "", f"the template must carry exactly one {token} placeholder"
        digest = checksums.get(asset)
        if digest is None:
            return "", f"the release published no {asset}"
        formula = formula.replace(token, digest)
    leftover = PLACEHOLDER.search(formula)
    if leftover:
        return "", f"unsubstituted placeholder: {leftover.group()}"
    return formula, None


def selftest_problem() -> str | None:
    """Return why the real template cannot render a representative release."""
    template = TEMPLATE.read_text()
    digests = {asset: str(index) * 64 for index, asset in enumerate(ASSETS.values())}
    listing = "\n".join(f"{digest}  {asset}" for asset, digest in digests.items())
    checksums, problem = read_checksums(listing)
    if problem:
        return problem
    formula, problem = render(template, "1.2.3", checksums)
    if problem:
        return problem
    missing = [digest for digest in digests.values() if digest not in formula]
    if 'version "1.2.3"' not in formula or missing:
        return "the template carries no place for every release value"
    duplicate = template.replace("@VERSION@", "@VERSION@\n@VERSION@")
    rejections = [
        (render(template, "1.2.3", {})[1], "a release without assets"),
        (render(duplicate, "1.2.3", checksums)[1], "duplicate placeholders"),
        (
            read_checksums(f"{listing}\n{listing.splitlines()[0]}")[1],
            "duplicate checksums",
        ),
        (render(template, "v1.2.3", checksums)[1], "prefixed versions"),
    ]
    for rejection, subject in rejections:
        if not rejection:
            return f"{subject} must not render"
    return None


def selftest() -> int:
    """Render the real template so a broken placeholder fails before a release."""
    problem = selftest_problem()
    if problem:
        print(f"homebrew_formula: {problem}", file=sys.stderr)
        return 1
    print("homebrew_formula: the template renders")
    return 0


def main() -> int:
    """Render the formula, or check that the template still renders."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="release version, without the v prefix")
    parser.add_argument("--checksums", type=Path, help="SHA256SUMS file for the release")
    parser.add_argument("--template", type=Path, default=TEMPLATE)
    parser.add_argument("--output", type=Path, help="write here instead of stdout")
    parser.add_argument("--selftest", action="store_true")
    args = parser.parse_args()

    if args.selftest:
        return selftest()
    if not args.version or not args.checksums:
        parser.error("--version and --checksums are required")

    checksums, problem = read_checksums(args.checksums.read_text())
    if problem is None:
        formula, problem = render(args.template.read_text(), args.version, checksums)
    if problem:
        print(f"homebrew_formula: {problem}", file=sys.stderr)
        return 1

    if args.output:
        args.output.write_text(formula)
    else:
        sys.stdout.write(formula)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
