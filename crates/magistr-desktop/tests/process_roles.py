#!/usr/bin/env python3
"""Exercise the actual desktop executable's protocol roles in disposable projects."""
import json
from pathlib import Path
import queue
import struct
import subprocess
import sys
import tempfile
import threading

BINARY = str(Path(sys.argv[1]).resolve())


class Rpc:
    def __init__(self, args, cwd):
        self.stderr = tempfile.TemporaryFile()
        self.child = subprocess.Popen(
            [BINARY, *args], cwd=cwd, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=self.stderr, text=True,
        )
        self.lines = queue.Queue()
        self.reader = threading.Thread(target=self.read, daemon=True)
        self.reader.start()
        self.serial = 0
        self.notifications = []

    def read(self):
        for line in self.child.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def send(self, message):
        self.child.stdin.write(json.dumps(message) + "\n")
        self.child.stdin.flush()

    def request(self, method, params):
        self.serial += 1
        self.send({"jsonrpc": "2.0", "id": self.serial, "method": method, "params": params})
        while True:
            line = self.lines.get(timeout=20)
            assert line is not None, f"EOF before {method}"
            message = json.loads(line)
            if message.get("id") == self.serial:
                assert "error" not in message, message
                return message["result"]
            self.notifications.append(message)

    def __enter__(self):
        return self

    def __exit__(self, kind, error, trace):
        self.child.stdin.close()
        try:
            code = self.child.wait(timeout=15)
            if kind is None:
                assert code == 0, f"protocol process exited {code}"
        finally:
            if self.child.poll() is None:
                self.child.kill()
                self.child.wait()
            self.reader.join(timeout=2)
            self.child.stdout.close()
            self.stderr.seek(0)
            if kind is not None:
                sys.stderr.write(self.stderr.read().decode(errors="replace"))
            self.stderr.close()


def frame(value):
    data = json.dumps(value).encode()
    return struct.pack("<I", len(data)) + data


def worker(root):
    result = subprocess.run(
        [BINARY, "--workspace-worker"], cwd=root,
        input=frame({"v": 1, "kind": "shutdown"}), capture_output=True, timeout=20,
    )
    assert result.returncode == 0, result.stderr
    size = struct.unpack("<I", result.stdout[:4])[0]
    assert json.loads(result.stdout[4:4 + size]) == {"kind": "ready", "v": 1}
    assert len(result.stdout) == size + 4, "stdout must contain only protocol frames"


def mcp(root, target, expected):
    with Rpc(["mcp-launch", target, str(root)], root) as rpc:
        rpc.request("initialize", {
            "protocolVersion": "2025-03-26", "capabilities": {},
            "clientInfo": {"name": "desktop-test", "version": "1"},
        })
        rpc.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        result = rpc.request("tools/list", {})
        assert expected in {tool["name"] for tool in result["tools"]}, result
        if target == "workbench-mcp":
            reply = rpc.request("tools/call", {
                "name": "read", "arguments": {"file": "sample.rs"},
            })
            data = json.loads(reply["content"][0]["text"])
            assert data["ok"], data
            assert any("desktop_source" in row.get("text", "") for row in data["results"])


def acp(root):
    with Rpc(["acp-agent", "--provider", "scripted"], root) as rpc:
        init = rpc.request("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
        assert init["protocolVersion"] == 1
        session = rpc.request("session/new", {"cwd": str(root), "mcpServers": []})
        reply = rpc.request("session/prompt", {
            "sessionId": session["sessionId"],
            "prompt": [{"type": "text", "text": "hello"}],
        })
        assert reply["stopReason"] == "end_turn", reply
        assert any(n.get("method") == "session/update" for n in rpc.notifications)


with tempfile.TemporaryDirectory(prefix="magistr-desktop-roles-") as directory:
    root = Path(directory)
    for args, code in [(["--help"], 0), (["--workspace-worker", "extra"], 2)]:
        result = subprocess.run([BINARY, *args], cwd=root, capture_output=True, timeout=15)
        assert result.returncode == code, result.stderr
    assert not list(root.iterdir()), "argument handling must not admit a project"
    worker(root)
    acp(root)
    (root / "sample.rs").write_text("pub fn desktop_source() {}\n")
    mcp(root, "magistr", "roadmap_status")
    mcp(root, "workbench-mcp", "read")
print("PASS: desktop argv, worker readiness/shutdown, ACP turn and both MCP endpoints")

