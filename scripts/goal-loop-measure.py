#!/usr/bin/env python3
"""Measure a next-loop iteration in a fixed-size pseudo-terminal.

Captures terminal output, loop traces, exit status, and elapsed time.
Sends Ctrl-D after the loop's end event.

    scripts/goal-loop-measure.py --mode native --out /tmp/run-native
    scripts/goal-loop-measure.py --mode auto --out /tmp/run-auto
"""

from __future__ import annotations

import argparse
import datetime as dt
import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time

ENDED_MARK = "event=Ended("
LEAVE_ALT_SCREEN = b"\x1b[?1049l"


def parse() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--mode", default="native", help="MAGISTR_GOAL value")
    parser.add_argument("--binary", default="magistr", help="the magistr binary to run")
    parser.add_argument("--dir", default=".", help="the project directory")
    parser.add_argument("--out", required=True, help="where screen.bin, loop.log and result.txt go")
    parser.add_argument("--cols", type=int, default=140)
    parser.add_argument("--rows", type=int, default=45)
    parser.add_argument("--max-iterations", type=int, default=1)
    parser.add_argument("--timeout", type=float, default=7200.0, help="seconds before the run is abandoned")
    parser.add_argument("--settle", type=float, default=5.0, help="seconds between the loop's end and ctrl-d")
    return parser.parse_args()


def child(args: argparse.Namespace, stderr_path: str, trace_path: str) -> None:
    """The cockpit's side: window size, stderr onto its file, exec."""
    fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", args.rows, args.cols, 0, 0))
    log_fd = os.open(stderr_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
    os.dup2(log_fd, 2)
    os.close(log_fd)
    env = dict(os.environ)
    # A nested Claude Code session refuses to start; the agent behind
    # the loop is one, so the marker of the session running this script
    # must not reach it.
    for key in [k for k in env if k.startswith("CLAUDECODE") or k.startswith("CLAUDE_CODE")]:
        env.pop(key)
    env["MAGISTR_GOAL"] = args.mode
    # The cockpit owns fd 2 while it runs (it replays it into the
    # scrollback on leave), so the trace goes to its own file.
    env["MAGISTR_LOG"] = "magistr::loop=info"
    env["MAGISTR_LOG_FILE"] = trace_path
    env["TERM"] = "xterm-256color"
    env["MAGISTR_MOTION"] = "reduced"
    argv = [args.binary, "next-loop", "--max-iterations", str(args.max_iterations), args.dir]
    os.execvpe(args.binary, argv, env)


def trace_says_ended(trace_path: str) -> bool:
    try:
        with open(trace_path, "rb") as trace:
            return ENDED_MARK.encode() in trace.read()
    except OSError:
        return False


def main() -> int:
    args = parse()
    os.makedirs(args.out, exist_ok=True)
    screen_path = os.path.join(args.out, "screen.bin")
    stderr_path = os.path.join(args.out, "stderr.log")
    trace_path = os.path.join(args.out, "trace.log")
    result_path = os.path.join(args.out, "result.txt")
    for stale in (trace_path,):
        try:
            os.remove(stale)
        except FileNotFoundError:
            pass

    started = time.monotonic()
    started_at = dt.datetime.now(dt.timezone.utc)
    pid, fd = pty.fork()
    if pid == 0:
        child(args, stderr_path, trace_path)
        return 1  # unreachable

    screen = open(screen_path, "wb")
    ended_seen: float | None = None
    quit_sent: float | None = None
    status: int | None = None
    last_poll = 0.0
    try:
        while True:
            ready, _, _ = select.select([fd], [], [], 0.25)
            if ready:
                try:
                    chunk = os.read(fd, 65536)
                except OSError as err:
                    if err.errno == errno.EIO:
                        chunk = b""
                    else:
                        raise
                if not chunk:
                    break
                screen.write(chunk)
                screen.flush()
            done, code = os.waitpid(pid, os.WNOHANG)
            if done:
                status = code
                break
            now = time.monotonic()
            if now - last_poll >= 1.0:
                last_poll = now
                if ended_seen is None and trace_says_ended(trace_path):
                    ended_seen = now
                    print(f"[measure] loop ended after {now - started:.1f}s; quitting in {args.settle:.0f}s", flush=True)
            if ended_seen is not None and quit_sent is None and now - ended_seen >= args.settle:
                os.write(fd, b"\x04")
                quit_sent = now
                print("[measure] ctrl-d sent", flush=True)
            if quit_sent is not None and now - quit_sent >= 30.0:
                print("[measure] the cockpit did not leave in 30s; sending ctrl-d again", flush=True)
                os.write(fd, b"\x04")
                quit_sent = now
            if now - started >= args.timeout:
                print(f"[measure] timeout after {args.timeout:.0f}s; terminating", flush=True)
                os.kill(pid, signal.SIGTERM)
                break
        if status is None:
            try:
                _, status = os.waitpid(pid, 0)
            except ChildProcessError:
                status = -1
    finally:
        screen.close()
        os.close(fd)
    wall = time.monotonic() - started
    ended_at = dt.datetime.now(dt.timezone.utc)

    with open(screen_path, "rb") as captured:
        painted = captured.read()
    tail = painted.rsplit(LEAVE_ALT_SCREEN, 1)
    scrollback = tail[1] if len(tail) == 2 else painted[-4000:]
    scrollback_text = scrollback.decode("utf-8", "replace").replace("\r\n", "\n")

    exit_code = os.waitstatus_to_exitcode(status) if status is not None and status >= 0 else status
    lines = [
        f"mode: {args.mode}",
        f"started: {started_at.isoformat()}",
        f"ended: {ended_at.isoformat()}",
        f"wall_seconds: {wall:.1f}",
        f"loop_ended_seen_after_seconds: {'' if ended_seen is None else f'{ended_seen - started:.1f}'}",
        f"exit: {exit_code}",
        "scrollback:",
        scrollback_text.rstrip(),
    ]
    with open(result_path, "w", encoding="utf-8") as result:
        result.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if exit_code == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
