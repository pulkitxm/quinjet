import argparse
import contextlib
import gzip
import hashlib
import json
import math
import os
import platform
import shutil
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
    "contents_search_128_files": ["--json", "search", "value_[0-9]+", "--mode", "contents"],
}
TERMINAL_CASES = {
    "first_frame_160x45": ("kitty", "kitty", "dark"),
    "first_frame_system_no_responses": ("kitty", "none", "system"),
    "first_frame_auto_sixel_160x45": ("auto", "sixel", "dark"),
    "first_frame_auto_no_responses": ("auto", "none", "dark"),
}
TERMINAL_REPLIES = (
    (b"\x1b[?u", b"\x1b[?31u"),
    (b"\x1b[6n", b"\x1b[1;1R"),
    (b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\", None),
    (b"\x1b[c", b"\x1b[?64;4;28c"),
    (b"\x1b[0c", b"\x1b[?64;4;28c"),
    (b"\x1b[16t", b"\x1b[6;18;9t"),
    (b"\x1b[14t", b"\x1b[4;810;1440t"),
    (b"\x1b[5n", b"\x1b[0n"),
)


def environment(root):
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(
            ("GIT_", "QUINJET_", "SSH_", "XDG_", "CMUX_", "KITTY_", "ITERM_", "WEZTERM_")
        )
        and key
        not in {
            "TERM_PROGRAM",
            "LC_TERMINAL",
            "TMUX",
            "TMUX_PANE",
            "COLORTERM",
            "COLORFGBG",
            "WT_SESSION",
        }
    }
    env.update(
        HOME=str(root),
        USERPROFILE=str(root),
        LOCALAPPDATA=str(root / "local-app-data"),
        APPDATA=str(root / "roaming-app-data"),
        ZDOTDIR=str(root),
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
    env = dict(env, PATH=os.pathsep.join((str(binary.parent), env["PATH"])))
    started = time.perf_counter_ns()
    result = subprocess.run(
        [str(binary), "-C", str(repository), *args],
        env=env,
        capture_output=True,
        check=True,
        timeout=30,
    )
    return (time.perf_counter_ns() - started) / 1_000_000, result.stdout


def finish_terminal(process, master, original_mode):
    import select
    import termios

    try:
        deadline = time.monotonic() + 3
        while process.poll() is None and time.monotonic() < deadline:
            with contextlib.suppress(OSError):
                os.write(master, b"q")
            readable, _, _ = select.select([master], [], [], 0.1)
            if not readable:
                continue
            try:
                if not os.read(master, 1 << 20):
                    break
            except OSError:
                break
        try:
            process.wait(timeout=max(0, deadline - time.monotonic()))
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            msg = "the synthetic terminal did not quit after receiving input"
            raise RuntimeError(msg) from None
        if process.returncode != 0:
            msg = "the synthetic terminal exited unsuccessfully"
            raise RuntimeError(msg)
        if termios.tcgetattr(master) != original_mode:
            msg = "the synthetic terminal did not restore its original mode"
            raise RuntimeError(msg)
    finally:
        os.close(master)


def answer_terminal(master, pending, mode):
    if mode == "none":
        pending.clear()
        return
    for request, reply in TERMINAL_REPLIES:
        while request in pending:
            if reply is None:
                status = b"OK" if mode == "kitty" else b"ENOTSUP"
                reply_bytes = b"\x1b_Gi=31;" + status + b"\x1b\\"
            else:
                reply_bytes = reply
            os.write(master, reply_bytes)
            position = pending.index(request)
            del pending[position : position + len(request)]
    if len(pending) > 256:
        del pending[:-256]


def first_frame(binary, repository, env, *, case):
    import fcntl
    import pty
    import select
    import struct
    import termios

    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 45, 160, 0, 0))
    original_mode = termios.tcgetattr(slave)
    started = time.perf_counter_ns()
    protocol, replies, appearance = TERMINAL_CASES[case]
    terminal_env = dict(
        env,
        PATH=os.pathsep.join((str(binary.parent), env["PATH"])),
        QUINJET_IMAGE_PROTOCOL=protocol,
    )
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
    pending = bytearray()
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
            pending.extend(chunk)
            answer_terminal(master, pending, replies)
            if b"\x1b[?25l" in chunk and len(output) > 1_000:
                return (time.perf_counter_ns() - started) / 1_000_000
        msg = "the synthetic terminal did not receive a complete first frame"
        raise RuntimeError(msg)
    finally:
        finish_terminal(process, master, original_mode)


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
            for case in TERMINAL_CASES:
                elapsed = first_frame(binary, repository, env, case=case)
                results[label]["timings"][case].append(elapsed)
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
            for case in TERMINAL_CASES:
                results[label]["timings"][case] = []
    return results


def benchmark(args):
    binaries = [("baseline", args.baseline.resolve()), ("candidate", args.candidate.resolve())]
    terminal = os.name == "posix" and not args.no_terminal
    results = binary_results(binaries, terminal=terminal)
    with tempfile.TemporaryDirectory(prefix="quinjet-benchmark-", dir=args.temp_dir) as directory:
        root = Path(directory)
        repository = fixture(root, environment(root / "git-home"))
        staged = []
        binary_name = "quinjet.exe" if os.name == "nt" else "quinjet"
        for label, binary in binaries:
            destination = root / "executables" / label / binary_name
            destination.parent.mkdir(parents=True)
            shutil.copy2(binary, destination)
            destination.chmod(destination.stat().st_mode | 0o111)
            staged.append((label, destination))
        binaries = staged
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
            "alternating order; warm OS cache; fresh process; identical executable names; "
            "isolated homes and executable directories first on PATH; synthetic Bash integration; "
            "p95 nearest rank"
        ),
        "aspirational_budget_bytes": 5_000_000,
        "candidate_below_aspirational_budget": results["candidate"]["bytes"] < 5_000_000,
        "regression_budget_bytes": args.budget,
        "terminal_method": (
            "160x45 PTY; Kitty override with complete capability replies in dark mode "
            "or no replies in System mode; explicit auto with Sixel replies or no replies; "
            "bounded quit-input retries; clean exit and mode restoration"
            if terminal
            else None
        ),
        "terminal_cases": (
            {
                case: {
                    "image_protocol": protocol,
                    "capability_replies": replies,
                    "appearance": appearance,
                }
                for case, (protocol, replies, appearance) in TERMINAL_CASES.items()
            }
            if terminal
            else None
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
