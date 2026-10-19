#!/usr/bin/env python3
"""Real resident hosts, provider adapters and source workers against a local HTTP peer."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import sqlite3
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import time

seen = []


class Provider(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        body = json.dumps({"models": [{"name": "desktop-test"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        content = data["messages"][-1]["content"]
        seen.append(content)
        if content == "headless failure":
            self.send_response(500)
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "application/x-ndjson")
        self.send_header("Connection", "close")
        self.end_headers()
        try:
            self.wfile.write(json.dumps({"message": {"content": content}, "done": False}).encode() + b"\n")
            self.wfile.flush()
            if content in ("first streaming", "third active"):
                time.sleep(3)
            self.wfile.write(b'{"done":true,"done_reason":"stop","prompt_eval_count":4,"eval_count":3}\n')
            self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        self.close_connection = True


def main():
    binary = str(Path(sys.argv[1]).resolve())
    with tempfile.TemporaryDirectory(prefix="magistr-resident-process-") as directory:
        parent = Path(directory)
        for name in ("first", "second", "failed"):
            root = parent / name
            (root / ".magistr").mkdir(parents=True)
            (root / ".magistr/project.json").write_text(json.dumps({
                "project": {"name": name, "description": "", "type": "cli"},
                "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                          "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"},
                "roadmap_backend": "native", "roadmap_exports": False,
                "quality_gate": {"check": "true", "test": "true", "lint": "true"},
                "format": {"command": "true", "extensions": ["rs"]},
                "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
                "reference_repos": [], "state_prefix": name,
            }))
            (root / "sample.rs").write_text("pub fn resident_source() {}\n")
        server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = dict(os.environ, OLLAMA_HOST=f"http://127.0.0.1:{server.server_port}",
                   XDG_DATA_HOME=str(parent / "data"), XDG_CONFIG_HOME=str(parent / "config"),
                   MAGISTR_HOME=str(parent / "state"), MAGISTR_WORKSPACE_NETWORK="0",
                   MAGISTR_RESIDENT_TEST_DIR=str(parent), MAGISTR_USER="history-author")
        try:
            result = subprocess.run([binary, str(parent)], env=env, capture_output=True, text=True, timeout=90)
            assert result.returncode == 0, result.stderr
            receipt = json.loads(result.stdout.strip().splitlines()[-1])
            assert receipt["sessions"] == 3 and receipt["workspaces"] == 2, receipt
            assert all(value for key, value in receipt.items() if key not in ("sessions", "workspaces")), receipt
            assert seen == ["first streaming", "second complete", "third active"], seen
            children = [json.loads(path.read_text()) for path in parent.glob("provider-*.json")]
            assert len(children) == 3 and len({child["execution"] for child in children}) == 3, children
            assert sorted(Path(child["workspace"]).name for child in children) == ["first", "first", "second"], children
            database = parent / "data/magistr/sessions.db"
            with sqlite3.connect(database) as history:
                rows = history.execute(
                    "SELECT session_id, workspace, provider, provider_session_id, outcome, "
                    "cost_usd, turns, summary FROM sessions"
                ).fetchall()
            assert len(rows) == 3 and len({row[0] for row in rows}) == 3, rows
            completed = next(row for row in rows if Path(row[1]).name == "second")
            assert completed[2] == "ollama" and completed[3], completed
            assert completed[4] == "completed" and completed[6] == 1, completed
            assert completed[7] == "second complete", completed
            assert completed[5] == 0.0, completed  # The local adapter reports a known free cost.
            assert any(row[5] is None for row in rows if row[4] != "completed"), rows
            receipt["history_reopened"] = True
            if len(sys.argv) > 2:
                cli = str(Path(sys.argv[2]).resolve())
                project = parent / "second"
                headless = subprocess.run(
                    [cli, "run", "--headless", "--agent", "ollama", "--model", "desktop-test", "headless complete"],
                    cwd=project, env=env, capture_output=True, text=True, timeout=45)
                assert headless.returncode == 0, headless.stderr
                failed = subprocess.run(
                    [cli, "run", "--headless", "--agent", "ollama", "--model", "desktop-test", "headless failure"],
                    cwd=project, env=env, capture_output=True, text=True, timeout=45)
                assert failed.returncode != 0, failed.stdout
                with sqlite3.connect(database) as history:
                    rows = history.execute(
                        "SELECT session_id, outcome, provider_session_id, cost_usd, summary FROM sessions"
                    ).fetchall()
                assert len(rows) == 5, rows
                assert any(row[1] == "completed" and row[4] == "headless complete" for row in rows), rows
                assert any(row[1] == "failed" and row[2] for row in rows), rows
                assert any(row[3] is None for row in rows if row[1] == "failed"), rows
                denied = subprocess.run(
                    [cli, "run", "--headless", "--resume", completed[0], "must not reach provider"],
                    cwd=project, env=env, capture_output=True, text=True, timeout=20)
                assert denied.returncode != 0 and "cannot resume" in denied.stderr, denied.stderr
                assert "must not reach provider" not in seen, seen
                receipt["headless_history"] = True
                receipt["headless_failure"] = True
                receipt["unsupported_resume_refused"] = True
            print(json.dumps(receipt, sort_keys=True))
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


if __name__ == "__main__":
    main()
