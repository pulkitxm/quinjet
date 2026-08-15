"""Drive Quinjet through a real pty and read back what it drew.

Every terminal-level claim about the pull-request pane in this repository was
checked with this: it opens a pull request against live GitHub, so it needs a
working `gh` login and network and cannot run in CI. Keep it for the things a
unit test cannot see, such as which requests overlap and how long a pane waits.

    python3 scripts/drive.py open   <repo> <pr>
    python3 scripts/drive.py switch <repo> <first> <second>

`open` reports how long the pane takes to answer from a cold cache. `switch`
opens one pull request, waits for its log warm-up to start, then switches to
another and reports whether the second one had to queue behind the first.

Requires pyte:  python3 -m venv .venv && .venv/bin/pip install pyte
Set QUINJET to test a build other than the installed binary.
"""

import fcntl
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time

import pyte

COLS, ROWS = 160, 45
BINARY = os.environ.get("QUINJET", os.path.expanduser("~/.cargo/bin/quinjet"))


class Session:
    def __init__(self, repository):
        master, slave = pty.openpty()
        # Without a window size ratatui has no room and draws nothing at all.
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.master = master
        self.process = subprocess.Popen(
            [BINARY],
            cwd=repository,
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=dict(os.environ, TERM="xterm-256color"),
            preexec_fn=os.setsid,
        )
        os.close(slave)
        self.screen = pyte.Screen(COLS, ROWS)
        self.stream = pyte.ByteStream(self.screen)
        self.group = os.getpgid(self.process.pid)
        self.pump(2.5)

    def pump(self, seconds):
        deadline = time.time() + seconds
        while time.time() < deadline:
            ready, _, _ = select.select([self.master], [], [], 0.02)
            if not ready:
                continue
            try:
                chunk = os.read(self.master, 1 << 20)
            except OSError:
                return
            if not chunk:
                return
            self.stream.feed(chunk)

    def body(self):
        return "\n".join(self.screen.display)

    def type(self, text, settle=0.06):
        for character in text:
            os.write(self.master, character.encode())
            time.sleep(settle)

    def gh_children(self):
        """What the workers are doing right now, which is the only way to see
        whether two reads overlapped or one queued behind the other."""
        listing = subprocess.run(
            ["ps", "-eo", "pgid,etimes,args"], capture_output=True, text=True
        ).stdout.splitlines()
        running = []
        for line in listing[1:]:
            parts = line.split(None, 2)
            if len(parts) < 3 or parts[0] != str(self.group):
                continue
            if not parts[2].startswith("gh "):
                continue
            running.append((int(parts[1]), parts[2]))
        return running

    def open_pull_request(self, number, focus="3"):
        self.type(focus)
        self.pump(1.0)
        self.type("\x7f" * 8, settle=0.04)
        self.type(str(number))
        self.pump(0.5)
        # The field can swallow the first Enter while a lookup is still settling.
        for _ in range(4):
            os.write(self.master, b"\r")
            self.pump(1.5)
            if "Source" in self.body() or "State" in self.body():
                return True
        return False

    def close(self):
        os.write(self.master, b"q")
        self.pump(0.5)
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()


def command_open(repository, number):
    session = Session(repository)
    try:
        started = time.time()
        if not session.open_pull_request(number):
            print("the pane never opened")
            return 1
        print(f"pane opened in {time.time() - started:.1f}s")
        for _ in range(60):
            session.pump(1.0)
            if "Loading the conversation" not in session.body():
                print(f"conversation rendered in {time.time() - started:.1f}s")
                return 0
        print("the conversation never rendered")
        return 1
    finally:
        session.close()


def command_switch(repository, first, second):
    session = Session(repository)
    try:
        if not session.open_pull_request(first):
            print(f"PR {first} never opened")
            return 1
        for _ in range(30):
            session.pump(1.0)
            if any("/logs" in command for _, command in session.gh_children()):
                break
        else:
            print("the log warm-up never started, so there is nothing to queue behind")
            return 1

        print(f"warm-up running, switching to {second}")
        session.open_pull_request(second, focus="/")
        started = time.time()
        overlapped = False
        for _ in range(120):
            session.pump(1.0)
            running = [command for _, command in session.gh_children()]
            if any("/logs" in c for c in running) and any(str(second) in c for c in running):
                overlapped = True
            if "Loading the conversation" not in session.body() and time.time() - started > 3:
                elapsed = time.time() - started
                print(f"PR {second} answered in {elapsed:.1f}s")
                print(
                    "the warm-up and the switch overlapped"
                    if overlapped
                    else "the switch waited for the warm-up to drain"
                )
                return 0 if overlapped else 1
        print(f"PR {second} never answered")
        return 1
    finally:
        session.close()


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    action = sys.argv[1]
    if action == "open" and len(sys.argv) == 4:
        return command_open(sys.argv[2], sys.argv[3])
    if action == "switch" and len(sys.argv) == 5:
        return command_switch(sys.argv[2], sys.argv[3], sys.argv[4])
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
