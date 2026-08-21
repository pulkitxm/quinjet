#!/usr/bin/env python3
"""Render a multi-file Winget manifest for one Quinjet release."""

from __future__ import annotations

import argparse
import re
import sys
from datetime import date
from pathlib import Path

from homebrew_formula import read_checksums

ROOT = Path(__file__).resolve().parent.parent
TEMPLATES = ROOT / "packaging" / "winget" / "templates"
VERSION = re.compile(r"^\d+\.\d+\.\d+$")
CHECKSUM = re.compile(r"^[0-9a-f]{64}$")
PLACEHOLDER = re.compile(r"@[A-Z0-9_]+@")
WINDOWS_ASSET = "quinjet.exe"
MANIFEST_COUNT = 3


def render(template: str, version: str, release_date: str, checksum: str) -> tuple[str, str | None]:
    """Substitute one release into one manifest template."""
    if not VERSION.fullmatch(version):
        return "", f"not a stable release version: {version}"
    try:
        date.fromisoformat(release_date)
    except ValueError:
        return "", f"not an ISO release date: {release_date}"
    if not CHECKSUM.fullmatch(checksum):
        return "", f"{WINDOWS_ASSET} has no sha-256 checksum: {checksum}"

    values = {
        "@VERSION@": version,
        "@RELEASE_DATE@": release_date,
        "@SHA256_WINDOWS_X86_64@": checksum.upper(),
    }
    manifest = template
    for token, value in values.items():
        manifest = manifest.replace(token, value)
    leftover = PLACEHOLDER.search(manifest)
    if leftover:
        return "", f"unsubstituted placeholder: {leftover.group()}"
    return manifest, None


def render_all(
    version: str, release_date: str, checksums_text: str
) -> tuple[dict[str, str], str | None]:
    """Render every authored template for a release."""
    checksums, problem = read_checksums(checksums_text)
    if problem:
        return {}, problem
    checksum = checksums.get(WINDOWS_ASSET)
    if checksum is None:
        return {}, f"the release published no {WINDOWS_ASSET}"

    manifests: dict[str, str] = {}
    for template_path in sorted(TEMPLATES.glob("*.yaml")):
        manifest, problem = render(template_path.read_text(), version, release_date, checksum)
        if problem:
            return {}, f"{template_path.name}: {problem}"
        manifests[template_path.name] = manifest
    if len(manifests) != MANIFEST_COUNT:
        return {}, "exactly three Winget manifest templates are required"
    return manifests, None


def selftest_problem() -> str | None:
    """Return why the real templates cannot render a representative release."""
    digest = "a" * 64
    manifests, problem = render_all("1.2.3", "2026-08-21", f"{digest}  {WINDOWS_ASSET}\n")
    if problem:
        return problem
    combined = "\n".join(manifests.values())
    expected = ["1.2.3", "2026-08-21", digest.upper(), "Pulkitxm.Quinjet"]
    if any(value not in combined for value in expected):
        return "the templates carry no place for every release value"
    rejections = [
        render_all("v1.2.3", "2026-08-21", f"{digest}  {WINDOWS_ASSET}\n")[1],
        render_all("1.2.3", "21-08-2026", f"{digest}  {WINDOWS_ASSET}\n")[1],
        render_all("1.2.3", "2026-08-21", "")[1],
    ]
    if any(problem is None for problem in rejections):
        return "invalid release values must not render"
    return None


def selftest() -> int:
    """Render the real templates and exercise rejected release values."""
    problem = selftest_problem()
    if problem:
        print(f"winget_manifest: {problem}", file=sys.stderr)
        return 1
    print("winget_manifest: the templates render")
    return 0


def main() -> int:
    """Render the manifests, or check that the templates still render."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="release version, without the v prefix")
    parser.add_argument("--release-date", help="release date in YYYY-MM-DD form")
    parser.add_argument("--checksums", type=Path, help="SHA256SUMS file for the release")
    parser.add_argument("--output-dir", type=Path, help="directory for the rendered manifests")
    parser.add_argument("--selftest", action="store_true")
    args = parser.parse_args()

    if args.selftest:
        return selftest()
    if not args.version or not args.release_date or not args.checksums or not args.output_dir:
        parser.error("--version, --release-date, --checksums, and --output-dir are required")

    manifests, problem = render_all(args.version, args.release_date, args.checksums.read_text())
    if problem:
        print(f"winget_manifest: {problem}", file=sys.stderr)
        return 1
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for name, manifest in manifests.items():
        (args.output_dir / name).write_text(manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
