#!/usr/bin/env python3
"""Render the Debian packaging that the Launchpad PPA builds from.

Launchpad compiles every upload from source without network access, so the
source tree carries vendored crates and builds them with the versioned Rust
toolchain that each Ubuntu series ships. This script writes the `debian/`
directory for one series into an unpacked source tree; `build_ppa_sources.sh`
calls it once per series.
"""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
from datetime import UTC, datetime
from email.utils import format_datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LICENSE = ROOT / "LICENSE"

MAINTAINER = "Pulkit <kpulkit15234@gmail.com>"

SERIES = {
    "jammy": ("22.04", "1.91"),
    "noble": ("24.04", "1.91"),
    "resolute": ("26.04", "1.93"),
    "stonking": ("26.10", "1.97"),
    "tenacious": ("27.04", "1.97"),
}

VERSION = re.compile(r"^\d+\.\d+\.\d+$")

CONTROL = """\
Source: quinjet
Section: vcs
Priority: optional
Maintainer: {maintainer}
Build-Depends: debhelper-compat (= 13), cargo-{rust}, rustc-{rust}
Standards-Version: 4.6.0
Homepage: https://github.com/pulkitxm/quinjet
Vcs-Browser: https://github.com/pulkitxm/quinjet
Rules-Requires-Root: no

Package: quinjet
Architecture: amd64 arm64
Depends: ${{shlibs:Depends}}, ${{misc:Depends}}, git
Suggests: gh
Description: fast, live, keyboard-first Git source-control interface
 Quinjet is a live, keyboard-first Git source-control interface for the
 terminal. It combines repository status, syntax-highlighted diffs, commit
 history, branches, stashes, and pull requests in one executable.
"""

RULES = """\
#!/usr/bin/make -f

export PATH := /usr/lib/rust-{rust}/bin:$(PATH)
export HOME := $(CURDIR)/debian/home
export CARGO_HOME := $(CURDIR)/debian/cargo-home

VENDORED := --config 'source.crates-io.replace-with="vendored-sources"' \\
\t--config 'source.vendored-sources.directory="vendor"'
BINARY := target/release/quinjet
GENERATED := debian/generated
PACKAGE := debian/quinjet

%:
\tdh $@

override_dh_auto_configure:

override_dh_auto_build:
\tcargo build --release --locked --offline $(VENDORED)
\tmkdir -p $(GENERATED)
\t$(BINARY) completions bash >$(GENERATED)/quinjet.bash
\t$(BINARY) completions fish >$(GENERATED)/quinjet.fish
\t$(BINARY) completions zsh >$(GENERATED)/_quinjet
\t$(BINARY) man >$(GENERATED)/quinjet.1

override_dh_auto_test:

override_dh_auto_install:
\tinstall -D -m 0755 $(BINARY) $(PACKAGE)/usr/bin/quinjet
\tln -s quinjet $(PACKAGE)/usr/bin/q
\tinstall -D -m 0644 $(GENERATED)/quinjet.bash \\
\t\t$(PACKAGE)/usr/share/bash-completion/completions/quinjet
\tinstall -D -m 0644 $(GENERATED)/quinjet.fish \\
\t\t$(PACKAGE)/usr/share/fish/vendor_completions.d/quinjet.fish
\tinstall -D -m 0644 $(GENERATED)/_quinjet \\
\t\t$(PACKAGE)/usr/share/zsh/vendor-completions/_quinjet
\tinstall -D -m 0644 $(GENERATED)/quinjet.1 $(PACKAGE)/usr/share/man/man1/quinjet.1

override_dh_auto_clean:
\trm -rf target $(GENERATED) debian/home debian/cargo-home

override_dh_dwz:
"""

CHANGELOG = """\
quinjet ({version}) {series}; urgency=medium

  * Release {upstream}.

 -- {maintainer}  {date}
"""

COPYRIGHT = """\
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: quinjet
Source: https://github.com/pulkitxm/quinjet

Files: *
Copyright: 2026 Pulkit
License: MIT

Files: vendor/*
Copyright: The authors of each vendored crate
License: various
 Each vendored crate carries its own license in its directory and
 Cargo.toml.

License: MIT
{license}"""


def package_version(upstream: str, series: str) -> str:
    """Return the Debian version for one upstream release in one series."""
    return f"{upstream}-1~ubuntu{SERIES[series][0]}"


def indented(text: str) -> str:
    """Fold a plain-text license into a Debian copyright paragraph."""
    lines = [f" {line}" if line.strip() else " ." for line in text.strip().splitlines()]
    return "\n".join(lines) + "\n"


def render(upstream: str, series: str, date: datetime) -> tuple[dict[str, str], str | None]:
    """Return every `debian/` file for one series, keyed by relative path."""
    if not VERSION.match(upstream):
        return {}, f"not a stable release version: {upstream}"
    if series not in SERIES:
        return {}, f"unknown series {series}; known: {', '.join(SERIES)}"
    rust = SERIES[series][1]
    files = {
        "control": CONTROL.format(maintainer=MAINTAINER, rust=rust),
        "rules": RULES.format(rust=rust),
        "changelog": CHANGELOG.format(
            version=package_version(upstream, series),
            series=series,
            upstream=upstream,
            maintainer=MAINTAINER,
            date=format_datetime(date),
        ),
        "copyright": COPYRIGHT.format(license=indented(LICENSE.read_text())),
        "source/format": "3.0 (quilt)\n",
    }
    return files, None


def write(files: dict[str, str], debian: Path) -> None:
    """Write rendered files, marking the rules file executable."""
    for name, contents in files.items():
        path = debian / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)
    (debian / "rules").chmod(0o755)


def series_problem(series: str, date: datetime) -> str | None:
    """Return why one series renders packaging that Launchpad cannot build."""
    number, rust = SERIES[series]
    files, problem = render("1.2.3", series, date)
    if problem:
        return problem
    if f"cargo-{rust}, rustc-{rust}" not in files["control"]:
        return f"{series} does not build with Rust {rust}"
    if f"/usr/lib/rust-{rust}/bin" not in files["rules"]:
        return f"{series} rules do not use Rust {rust}"
    if f"quinjet (1.2.3-1~ubuntu{number}) {series};" not in files["changelog"]:
        return f"{series} changelog has the wrong version line"
    if "\tdh $@\n" not in files["rules"] or "\tcargo build" not in files["rules"]:
        return "rules recipes must be indented with tabs"
    return None


def selftest_problem() -> str | None:
    """Return why the packaging cannot render a representative release."""
    date = datetime(2026, 1, 2, 3, 4, 5, tzinfo=UTC)
    problems = [series_problem(series, date) for series in SERIES]
    numbers = [tuple(map(int, number.split("."))) for number, _ in SERIES.values()]
    if numbers != sorted(numbers):
        problems.append("series must be listed oldest first so package versions increase")
    if "  Fri, 02 Jan 2026 03:04:05 +0000\n" not in render("1.2.3", "noble", date)[0]["changelog"]:
        problems.append("the changelog date is not RFC 2822")
    problem = next((problem for problem in problems if problem), None)
    if problem:
        return problem
    rejections = [
        (render("v1.2.3", "noble", date)[1], "prefixed versions"),
        (render("1.2.3", "bionic", date)[1], "unsupported series"),
    ]
    for rejection, subject in rejections:
        if not rejection:
            return f"{subject} must not render"
    with tempfile.TemporaryDirectory() as directory:
        debian = Path(directory) / "debian"
        write(render("1.2.3", "noble", date)[0], debian)
        if not (debian / "rules").stat().st_mode & 0o100:
            return "debian/rules is not executable"
    return None


def selftest() -> int:
    """Render every series so a broken template fails before a release."""
    problem = selftest_problem()
    if problem:
        print(f"ppa_source: {problem}", file=sys.stderr)
        return 1
    print("ppa_source: every series renders")
    return 0


def main() -> int:
    """Write `debian/` for one series, list the series, or run the self test."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="release version, without the v prefix")
    parser.add_argument("--series", help="Ubuntu series codename")
    parser.add_argument("--output", type=Path, help="source tree to write debian/ into")
    parser.add_argument("--list-series", action="store_true")
    parser.add_argument("--selftest", action="store_true")
    args = parser.parse_args()

    if args.selftest:
        return selftest()
    if args.list_series:
        print("\n".join(SERIES))
        return 0
    if not args.version or not args.series or not args.output:
        parser.error("--version, --series and --output are required")

    files, problem = render(args.version, args.series, datetime.now(UTC))
    if problem:
        print(f"ppa_source: {problem}", file=sys.stderr)
        return 1
    write(files, args.output / "debian")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
