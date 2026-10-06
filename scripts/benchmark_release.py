import argparse
import contextlib
import gzip
import hashlib
import json
import math
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

COMMANDS = {
    "version": ["--version"],
    "help": ["--help"],
    "capabilities": ["capabilities"],
    "status": ["--json", "status"],
    "diff_128_files": ["diff"],
}


def environment(root):
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("GIT_", "QUINJET_", "SSH_", "XDG_"))
    }
    env.update(
        HOME=str(root),
        USERPROFILE=str(root),
        XDG_BIN_HOME=str(root / "bin"),
        XDG_CONFIG_HOME=str(root / "config"),
        XDG_DATA_HOME=str(root / "data"),
        XDG_STATE_HOME=str(root / "state"),
        XDG_CACHE_HOME=str(root / "cache"),
        QUINJET_STATE_DIR=str(root / "state" / "quinjet"),
        QUINJET_CACHE_DIR=str(root / "cache" / "quinjet"),
        GIT_CONFIG_GLOBAL=os.devnull,
        GIT_CONFIG_NOSYSTEM="1",
        GIT_TERMINAL_PROMPT="0",
        GH_PROMPT_DISABLED="1",
        LC_ALL="C",
        TERM="xterm-256color",
        SHELL="/bin/bash",
    )
    return env


def fixture(root, env):
    repository = root / "repository"
    repository.mkdir()

    def git(*args):
        subprocess.run(
            ["git", "-C", str(repository), *args],
            env=env,
            check=True,
            capture_output=True,
        )

    git("init", "--initial-branch=benchmark")
    git("config", "user.name", "Example Developer")
    git("config", "user.email", "developer@example.invalid")
    git("config", "commit.gpgsign", "false")
    for index in range(128):
        path = repository / f"example_{index:03}.rs"
        path.write_text(f"fn value_{index}() -> u32 {{\n    {index}\n}}\n", encoding="utf-8")
    git("add", ".")
    git("commit", "--message=example baseline")
    for index in range(128):
        path = repository / f"example_{index:03}.rs"
        path.write_text(f"fn value_{index}() -> u32 {{\n    {index + 1}\n}}\n", encoding="utf-8")
    return repository


def summary(values):
    ordered = sorted(values)
    return {
        "samples": len(values),
        "median_ms": round(statistics.median(values), 3),
        "p95_ms": round(ordered[math.ceil(len(values) * 0.95) - 1], 3),
        "min_ms": round(ordered[0], 3),
    }


def invocation(binary, args, repository, env):
    started = time.perf_counter_ns()
    result = subprocess.run(
        [str(binary), "-C", str(repository), *args],
        env=env,
        capture_output=True,
        check=True,
        timeout=30,
    )
    return (time.perf_counter_ns() - started) / 1_000_000, result.stdout


def first_frame(binary, repository, env, *, responsive):
    import fcntl
    import pty
    import select
    import struct
    import termios

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 45, 160, 0, 0))
    started = time.perf_counter_ns()
    terminal_env = dict(env, QUINJET_IMAGE_PROTOCOL="auto")
    appearance = "dark" if responsive else "system"
    process = subprocess.Popen(
        [str(binary), "-C", str(repository), "tui", "--appearance", appearance, "--no-mouse"],
        env=terminal_env,
        stdin=slave,
        stdout=slave,
        stderr=slave,
        start_new_session=True,
    )
    os.close(slave)
    output = bytearray()
    try:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            readable, _, _ = select.select([master], [], [], 0.1)
            if not readable:
                continue
            try:
                chunk = os.read(master, 1 << 20)
            except OSError:
                break
            output.extend(chunk)
            if responsive and b"\x1b[6n" in chunk:
                os.write(master, b"\x1b[1;1R")
            if responsive and (b"\x1b[c" in chunk or b"\x1b[0c" in chunk):
                os.write(master, b"\x1b[?1;2c")
            if responsive and b"\x1b[16t" in chunk:
                os.write(master, b"\x1b[6;18;9t")
            if responsive and b"\x1b[14t" in chunk:
                os.write(master, b"\x1b[4;810;1440t")
            if b"\x1b[?25l" in chunk and len(output) > 1_000:
                return (time.perf_counter_ns() - started) / 1_000_000
        msg = "the synthetic terminal did not receive a complete first frame"
        raise RuntimeError(msg)
    finally:
        with contextlib.suppress(OSError):
            os.write(master, b"q")
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        os.close(master)


def measure_trial(trial, binaries, results, root, *, terminal):
    order = binaries if trial % 2 == 0 else list(reversed(binaries))
    repository = root / "repository"
    outputs = {}
    for label, binary in order:
        env = environment(root / label)
        cold_env = environment(root / f"first-use-{label}-{trial}")
        elapsed, _ = invocation(binary, ["--version"], repository, cold_env)
        results[label]["timings"]["first_use_version"].append(elapsed)
        for name, command in COMMANDS.items():
            elapsed, output = invocation(binary, command, repository, env)
            results[label]["timings"][name].append(elapsed)
            outputs[label, name] = output
        if terminal:
            elapsed = first_frame(binary, repository, env, responsive=True)
            results[label]["timings"]["first_frame_160x45"].append(elapsed)
            elapsed = first_frame(binary, repository, env, responsive=False)
            results[label]["timings"]["first_frame_system_no_responses"].append(elapsed)
    for name in COMMANDS:
        if outputs["baseline", name] != outputs["candidate", name]:
            msg = f"{name} output differs between the baseline and candidate"
            raise RuntimeError(msg)


def binary_results(binaries, *, terminal):
    results = {}
    for label, binary in binaries:
        content = binary.read_bytes()
        results[label] = {
            "bytes": len(content),
            "gzip_bytes": len(gzip.compress(content, compresslevel=9, mtime=0)),
            "sha256": hashlib.sha256(content).hexdigest(),
            "timings": {name: [] for name in COMMANDS},
        }
        results[label]["timings"]["first_use_version"] = []
        if terminal:
            results[label]["timings"]["first_frame_160x45"] = []
            results[label]["timings"]["first_frame_system_no_responses"] = []
    return results


def benchmark(args):
    binaries = [("baseline", args.baseline.resolve()), ("candidate", args.candidate.resolve())]
    terminal = os.name == "posix" and not args.no_terminal
    results = binary_results(binaries, terminal=terminal)
    with tempfile.TemporaryDirectory(prefix="quinjet-benchmark-", dir=args.temp_dir) as directory:
        root = Path(directory)
        repository = fixture(root, environment(root / "git-home"))
        for label, binary in binaries:
            invocation(binary, ["--version"], repository, environment(root / label))
        for trial in range(args.samples):
            measure_trial(trial, binaries, results, root, terminal=terminal)
    for result in results.values():
        result["timings"] = {name: summary(values) for name, values in result["timings"].items()}
    report = {
        "platform": platform.system(),
        "architecture": platform.machine(),
        "fixture": "128 modified synthetic Rust files, one local commit, no remotes",
        "method": (
            "alternating order; warm OS cache; fresh process; isolated homes; p95 nearest rank"
        ),
        "results": results,
    }
    rendered = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    if args.budget and results["candidate"]["bytes"] >= args.budget:
        msg = f"candidate exceeds the strict {args.budget} byte budget"
        raise RuntimeError(msg)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=30)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--budget", type=int)
    parser.add_argument("--temp-dir", type=Path)
    parser.add_argument("--no-terminal", action="store_true")
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    benchmark(args)


if __name__ == "__main__":
    main()
