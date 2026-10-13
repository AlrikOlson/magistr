#!/usr/bin/env python3
"""Open, switch and refuse projects in one production desktop window, with a controlled
Ollama HTTP peer. Recent projects come from a disposable session database."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import threading

requests = []


class Provider(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        assert self.path == "/api/tags", self.path
        self.reply(json.dumps({"models": [{"name": "desktop-test"}]}).encode(), "application/json")

    def do_POST(self):
        assert self.path == "/api/chat", self.path
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        requests.append(body["messages"][-1]["content"])
        content = b'{"message":{"content":"reply from the second project"},"done":false}\n'
        content += b'{"done":true,"done_reason":"stop","prompt_eval_count":4,"eval_count":3}\n'
        self.reply(content, "application/x-ndjson")

    def reply(self, body, kind):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def project(parent, name):
    root = parent / name
    (root / ".magistr").mkdir(parents=True)
    (root / ".magistr" / "project.json").write_text(json.dumps({
        "project": {"name": name, "description": "", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                  "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"},
        "roadmap_backend": "think-and-ship",
        "roadmap_exports": False,
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": name,
    }))
    (root / f"{name}.txt").write_text(f"{name}\n")
    return root.resolve()


def record_sessions(data_home, roots):
    """The machine's session records, as a finished session would have left them."""
    database = data_home / "magistr" / "sessions.db"
    database.parent.mkdir(parents=True)
    with sqlite3.connect(database) as db:
        db.execute("""CREATE TABLE sessions (
            session_id TEXT PRIMARY KEY, project TEXT NOT NULL, started_at TEXT NOT NULL,
            ended_at TEXT NOT NULL, cost_usd REAL NOT NULL, turns INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL, files_changed TEXT NOT NULL DEFAULT '[]',
            tags TEXT NOT NULL DEFAULT '[]', summary TEXT)""")
        for index, root in enumerate(roots):
            db.execute(
                "INSERT INTO sessions VALUES (?, ?, ?, ?, 0.0, 1, 1000, '[]', '[]', NULL)",
                (f"recorded-{index}", str(root), f"2026-09-1{index}T10:00:00+00:00",
                 f"2026-09-1{index}T10:01:00+00:00"))
        db.execute("PRAGMA user_version = 1")


def main():
    binary = Path(sys.argv[1]).resolve()
    driver = Path(__file__).with_suffix(".swift").resolve()
    with tempfile.TemporaryDirectory(prefix="magistr-desktop-switch-") as directory:
        parent = Path(directory)
        first, second, held = (project(parent, name) for name in ("first", "second", "held"))
        data_home = parent / "data"
        record_sessions(data_home, [first, second, held])
        server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = dict(os.environ, OLLAMA_HOST=f"http://127.0.0.1:{server.server_port}",
                   XDG_DATA_HOME=str(data_home))
        try:
            result = subprocess.run(
                ["swift", str(driver), str(binary), str(first), str(second), str(held)],
                env=env, capture_output=True, text=True, timeout=240,
            )
            print(result.stdout, end="")
            assert result.returncode == 0, result.stderr
            assert requests == ["sent in the second project"], requests
            # Every project the window held is released: each owner opens again.
            for root in (first, second, held):
                subprocess.run(
                    [str(binary), "mcp-launch", "workbench-mcp", str(root)],
                    input="", text=True, capture_output=True, timeout=15, check=True, env=env,
                )
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
    print("PASS: refused switch kept the first project; pointer, keyboard and accessibility "
          "switches restored each session's own draft; the second project answered; every owner was released")


if __name__ == "__main__":
    main()
