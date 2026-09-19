#!/usr/bin/env python3
"""Opt-in real Codex + macOS window acceptance; uses only disposable projects."""
import json
import os
from pathlib import Path
import queue
import signal
import subprocess
import sys
import tempfile
import threading
import time


class Window:
    def __init__(self, binary, root, model, log):
        self.child = subprocess.Popen(
            ["swift", str(Path(__file__).with_suffix(".swift")), str(binary), str(root), model],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True,
        )
        self.lines = queue.Queue()
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()
        hello = self.receive()
        assert hello.get("ready"), hello
        self.pid = hello["pid"]
        self.provider_pid = self.provider_children()[0]

    def provider_children(self):
        return self.children(" acp-agent --provider codex")

    def children(self, role):
        output = subprocess.check_output(
            ["ps", "-axo", "pid=,ppid=,command="], text=True)
        return [int(parts[0]) for line in output.splitlines()
                if len(parts := line.split(None, 2)) == 3
                and int(parts[1]) == self.pid and role in parts[2]]

    def read(self):
        for line in self.child.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def receive(self):
        line = self.lines.get(timeout=60)
        assert line is not None, "Native driver exited; see launch log"
        return json.loads(line)

    def call(self, op, **args):
        self.child.stdin.write(json.dumps(dict(op=op, **args)) + "\n")
        self.child.stdin.flush()
        result = self.receive()
        assert result.get("ok"), result
        return result

    def rows(self):
        return self.call("snapshot")["rows"]

    def wait(self, description, predicate, timeout=90):
        deadline = time.monotonic() + timeout
        rows = []
        while time.monotonic() < deadline:
            rows = self.rows()
            if predicate(rows):
                print("PASS:", description, flush=True)
                return rows
            time.sleep(0.15)
        raise AssertionError(f"{description}: {json.dumps(rows, ensure_ascii=False)}")

    def has(self, name, enabled=True):
        return lambda rows: any(r["name"] == name and (not enabled or r["enabled"]) for r in rows)

    def text(self, text):
        return lambda rows: any(text in r["value"] or text in r["name"] for r in rows)

    def draft(self, text, planning=False):
        self.call("set", name="Request roadmap changes" if planning else "Message Magistr",
                  role="AXTextArea", value=text)

    def command(self, name, via="press"):
        self.call("key", code=40, flags=1 << 20)  # Cmd-K is available in every view.
        self.call("set", name="Commands", role="AXTextField", value=name)
        rows = self.wait("command result", lambda rows: any(
            name in r["name"] and r["role"] == "AXButton" for r in rows))
        result = next(r["name"] for r in rows if name in r["name"] and r["role"] == "AXButton")
        self.call(via, name=result, role="AXButton")

    def settled(self):
        self.wait("turn settled", self.has("Send", enabled=False))

    def reply(self, text):
        self.settled()
        self.wait("assistant reply", lambda rows: any(
            r["name"] == "Conversation" and r["value"].rstrip().endswith(text) for r in rows), timeout=3)

    def send(self, text, via="press"):
        self.draft(text)
        self.wait("Send available", self.has("Send"))
        self.call(via, name="Send", role="AXButton")

    def close(self):
        try:
            if self.child.poll() is None:
                self.call("close")
        finally:
            self.child.stdin.close()
            try:
                self.child.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.child.terminate()
                self.child.wait(timeout=10)
            self.reader.join(timeout=2)
            self.child.stdout.close()


def exercise(window, root):
    window.send(
        "Run exactly this shell command once: echo desktop-live-ok > approved.txt. "
        "Use only your command execution tool, not apply_patch or MCP. "
        "Do not inspect other files. Then reply with exactly APPROVED_DONE. "
        "Remember the phrase amber otter for this conversation."
    )
    window.wait("approval request", window.has("Allow"))
    window.call("press", name="Allow", role="AXButton")
    window.reply("APPROVED_DONE")
    assert (root / "approved.txt").read_text() == "desktop-live-ok\n"

    window.send(
        "Run exactly this shell command once: echo should-not-exist > denied.txt. "
        "Use only your command tool. If denied, do not retry or use another tool; "
        "reply exactly DENIED_DONE."
    )
    window.wait("denial request", window.has("Deny"))
    window.call("key", name="Deny", role="AXButton", code=36)
    window.reply("DENIED_DONE")
    assert not (root / "denied.txt").exists()

    window.send(
        "Run exactly this shell command once: sleep 15. Use only your command tool. "
        "After it completes, reply exactly SLEEP_DONE."
    )
    window.wait("slow command approval", window.has("Allow"))
    window.call("click", name="Allow", role="AXButton")
    window.wait("working", window.has("Stop"))
    window.command("Recorded work", via="key")
    window.command("Roadmap", via="click")
    window.draft("Do not use tools or change the roadmap. Reply exactly PLAN_SEPARATE.", planning=True)
    window.call("press", name="Send", role="AXButton")
    window.wait("planning queued behind live work", window.text("Queued"))
    window.call("press", name="Stop", role="AXButton")
    window.wait("queued request cancelled", window.text("Stopped"))
    window.draft("Do not use tools or change the roadmap. Reply exactly PLAN_SEPARATE.", planning=True)
    window.call("press", name="Send", role="AXButton")
    window.settled()
    rows = window.wait("separate planning response", lambda rows: any(
        r["name"] == "Requests and responses" and r["value"].count("PLAN_SEPARATE") >= 3
        for r in rows))
    body = next(r["value"] for r in rows if r["name"] == "Requests and responses")
    assert "SLEEP_DONE" not in body, body
    window.command("Conversation")
    window.send("Without tools, tell me the two-word phrase I asked you to remember. Reply only that phrase.")
    window.settled()
    window.wait("conversation context retained", lambda rows: any(
        r["name"] == "Conversation" and r["value"].rstrip().endswith("amber otter") for r in rows))

    window.send(
        "Run exactly this shell command once: sleep 30; echo bad > cancelled.txt. "
        "Use only your command tool. If cancelled do not retry."
    )
    window.wait("cancellable command approval", window.has("Allow"))
    window.call("press", name="Allow", role="AXButton")
    window.wait("running before stop", window.has("Stop"))
    window.draft("Keep my unsent draft")
    window.call("click", name="Stop", role="AXButton")
    window.settled()
    window.wait("draft survives cancellation", lambda rows: any(
        r["name"] == "Message Magistr" and r["value"] == "Keep my unsent draft" for r in rows))
    assert not (root / "cancelled.txt").exists()

    # Kill only this disposable window's original provider adapter. Never signal an owner.
    assert window.provider_pid in window.provider_children()
    os.kill(window.provider_pid, signal.SIGKILL)
    window.wait("provider death is visible", window.text("exited unexpectedly"))
    window.wait("draft survives provider death", lambda rows: any(
        r["name"] == "Message Magistr" and r["value"] == "Keep my unsent draft" for r in rows))
    window.send("Without tools, repeat the two-word phrase I asked you to remember at the start.")
    window.settled()
    window.wait("restored provider remembers conversation", lambda rows: any(
        r["name"] == "Conversation" and r["value"].rstrip().endswith("amber otter") for r in rows))
    assert window.provider_pid not in window.provider_children()
    print("PASS: provider death and restoration use the original conversation", flush=True)


def main():
    assert os.environ.get("MAGISTR_DESKTOP_LIVE") == "1", (
        "Set MAGISTR_DESKTOP_LIVE=1 to run live subscription turns in a disposable project")
    assert sys.platform == "darwin", "This driver requires macOS Accessibility"
    binary = Path(sys.argv[1]).resolve()
    model = os.environ.get("MAGISTR_DESKTOP_MODEL", "gpt-5.6-sol")
    with tempfile.TemporaryDirectory(prefix="magistr-desktop-live-") as directory:
        root = Path(directory).resolve()
        (root / "sample.txt").write_text("before\n")
        project_journey = os.environ.get("MAGISTR_DESKTOP_JOURNEY") == "project"
        import native_project
        native_project.setup(root, binary)
        with tempfile.TemporaryFile(mode="w+") as log:
            window = None
            try:
                window = Window(binary, root, model, log)
                workers = window.children(" --workspace-worker")
                assert len(workers) == 1, workers
                if project_journey:
                    native_project.exercise(window, root)
                else:
                    exercise(window, root)
                assert window.children(" --workspace-worker") == workers
                print("PASS: all peers shared one workspace worker", flush=True)
            finally:
                if window:
                    window.close()
                log.seek(0)
                print(log.read(), file=sys.stderr)
        # A new attachment can own the project after normal native close.
        subprocess.run([str(binary), "mcp-launch", "workbench-mcp", str(root)],
                       input="", text=True, capture_output=True, timeout=20, check=True)
        print("PASS: project reopened after native close", flush=True)
    print("PASS: live desktop journey", flush=True)


if __name__ == "__main__":
    main()

