#!/usr/bin/env python3
"""Real resident hosts, provider adapters and source workers against a local HTTP peer."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
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
                "roadmap_backend": "think-and-ship", "roadmap_exports": False,
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
                   MAGISTR_RESIDENT_TEST_DIR=str(parent))
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
            print(json.dumps(receipt, sort_keys=True))
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


if __name__ == "__main__":
    main()
