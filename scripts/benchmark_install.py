import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

import benchmark_release as release

ASSETS = (
    "quinjet-linux-x86_64",
    "quinjet-linux-aarch64",
    "quinjet-macos-x86_64",
    "quinjet-macos-aarch64",
)
MOCK_CURL = """#!/usr/bin/env python3
import json
import os
import shutil
import sys
from pathlib import Path

args = sys.argv[1:]
url = next(value for value in args if value.startswith("https://"))
name = url.rsplit("/", 1)[1]
with Path(os.environ["MOCK_REQUEST_LOG"]).open("a") as stream:
    stream.write(json.dumps({"asset": name}) + "\\n")
if name == "latest" and "-w" in args:
    prefix = "https://github.com/pulkitxm/quinjet/releases/tag/v"
    sys.stdout.write(prefix + os.environ["MOCK_VERSION"])
else:
    output = args[args.index("-o") + 1]
    shutil.copy2(Path(os.environ["MOCK_RELEASE"]) / name, output)
"""


def prepare_transport(root, binaries, asset):
    tools = root / "tools"
    tools.mkdir()
    curl = tools / "curl"
    curl.write_text(MOCK_CURL, encoding="utf-8")
    curl.chmod(0o755)
    for label, binary in binaries.items():
        fixture = root / "releases" / label
        fixture.mkdir(parents=True)
        destination = fixture / asset
        shutil.copy2(binary, destination)
        digest = hashlib.sha256(destination.read_bytes()).hexdigest()
        checksums = fixture / "SHA256SUMS"
        checksums.write_text(f"{digest}  dist/{asset}\n", encoding="utf-8")
    return tools


def install_case(installer, case, env, version):
    binary_directory = case / "bin"
    command = ["sh", str(installer), "--bin-dir", str(binary_directory), "--no-modify-path"]
    if version:
        command.extend(["--version", version])
    started = time.perf_counter_ns()
    subprocess.run(command, env=env, check=True, capture_output=True, timeout=30)
    elapsed = (time.perf_counter_ns() - started) / 1_000_000
    completion = case / "home" / "data" / "bash-completion" / "completions" / "quinjet"
    if not completion.is_file():
        msg = "completion installation is missing"
        raise RuntimeError(msg)
    shortcut = binary_directory / "q"
    if not shortcut.is_symlink():
        msg = "immediate q shortcut is missing"
        raise RuntimeError(msg)
    output = subprocess.check_output([str(shortcut), "--version"], env=env, timeout=30)
    if output != f"quinjet {env['MOCK_VERSION']}\n".encode():
        msg = "installed shortcut cannot run the expected binary"
        raise RuntimeError(msg)
    log = case / "requests.jsonl"
    requests = [json.loads(line)["asset"] for line in log.read_text(encoding="utf-8").splitlines()]
    return elapsed, requests


def case_environment(root, case, tools, label, version):
    env = release.environment(case / "home")
    env.update(
        PATH=os.pathsep.join((str(tools), str(case / "bin"), env["PATH"])),
        MOCK_REQUEST_LOG=str(case / "requests.jsonl"),
        MOCK_RELEASE=str(root / "releases" / label),
        MOCK_VERSION=version,
    )
    return env


def benchmark(args):
    installers = {
        "baseline": args.baseline_installer.resolve(),
        "candidate": args.candidate_installer.resolve(),
    }
    binaries = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    outcomes = {label: {mode: [] for mode in ("pinned", "latest")} for label in installers}
    requests = {label: {} for label in installers}
    version = args.version.removeprefix("v")
    with tempfile.TemporaryDirectory(
        prefix="quinjet-install-comparison-", dir=args.temp_dir
    ) as directory:
        root = Path(directory).resolve()
        tools = prepare_transport(root, binaries, args.asset)
        for trial in range(args.samples):
            order = ("baseline", "candidate") if trial % 2 == 0 else ("candidate", "baseline")
            for mode in ("pinned", "latest"):
                for label in order:
                    case = root / f"{label}-{mode}-{trial}"
                    case.mkdir()
                    env = case_environment(root, case, tools, label, version)
                    pinned_version = version if mode == "pinned" else None
                    elapsed, assets = install_case(installers[label], case, env, pinned_version)
                    outcomes[label][mode].append(elapsed)
                    previous = requests[label].setdefault(mode, assets)
                    if previous != assets:
                        msg = "download ordering changed across trials"
                        raise RuntimeError(msg)
    report = {
        "platform": platform.system(),
        "architecture": platform.machine(),
        "version": version,
        "asset": args.asset,
        "method": (
            "alternating order; fresh isolated home; actual installer and executable; "
            "local mock curl without network delay"
        ),
        "samples": args.samples,
        "results": {
            label: {
                "binary_bytes": binary.stat().st_size,
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "installer_sha256": hashlib.sha256(installers[label].read_bytes()).hexdigest(),
                "pinned": release.summary(outcomes[label]["pinned"]),
                "latest": release.summary(outcomes[label]["latest"]),
                "requests": requests[label],
                "completion_and_immediate_shortcut_verified": True,
            }
            for label, binary in binaries.items()
        },
    }
    rendered = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--baseline-installer", type=Path, required=True)
    parser.add_argument("--candidate-installer", type=Path, default=Path("install.sh"))
    parser.add_argument("--asset", choices=ASSETS, required=True)
    parser.add_argument("--version", default="0.0.69")
    parser.add_argument("--samples", type=int, default=21)
    parser.add_argument("--temp-dir", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if os.name != "posix":
        parser.error("the shell installer benchmark requires a POSIX host")
    if args.samples < 1:
        parser.error("--samples must be positive")
    benchmark(args)


if __name__ == "__main__":
    main()
