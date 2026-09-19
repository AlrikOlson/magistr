#!/usr/bin/env python3
"""Native window + real host/process composition with a controlled Ollama HTTP peer."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
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
        body = json.dumps({"models": [{"name": "desktop-test"}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        assert self.path == "/api/chat", self.path
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        requests.append(body)
        content = b'{"message":{"content":"desktop native response"},"done":false}\n'
        content += b'{"done":true,"done_reason":"stop","prompt_eval_count":4,"eval_count":3}\n'
        self.send_response(200)
        self.send_header("Content-Type", "application/x-ndjson")
        self.send_header("Content-Length", str(len(content)))
        self.end_headers()
        self.wfile.write(content)


binary = Path(sys.argv[1]).resolve()
driver = Path(__file__).with_suffix(".swift").resolve()
with tempfile.TemporaryDirectory(prefix="magistr-desktop-native-") as directory:
    root = Path(directory)
    (root / "sample.rs").write_text("pub fn desktop_source() {}\n")
    server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    env = dict(os.environ, OLLAMA_HOST=f"http://127.0.0.1:{server.server_port}")
    try:
        result = subprocess.run(
            ["swift", str(driver), str(binary), str(root)], env=env,
            capture_output=True, text=True, timeout=100,
        )
        print(result.stdout, end="")
        assert result.returncode == 0, result.stderr
        assert len(requests) == 1, requests
        assert requests[0]["model"] == "desktop-test", requests
        assert requests[0]["messages"][-1] == {
            "role": "user", "content": "desktop native request",
        }, requests
        # Reopening the same project's owner verifies release after native close.
        subprocess.run(
            [str(binary), "mcp-launch", "workbench-mcp", str(root)],
            input="", text=True, capture_output=True, timeout=15, check=True,
        )
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)
print("PASS: native Send, streamed reply, retained draft, window close and project reopen")

