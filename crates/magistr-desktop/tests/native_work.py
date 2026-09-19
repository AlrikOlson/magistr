#!/usr/bin/env python3
"""Start the roadmap loop from the production desktop window against a controlled Ollama peer.

The window's Start loop button runs one judged iteration on the shared host: the agent
turn, the project's gates, the judge's verdict and the completion all settle in the
loop's operation ledger. No model service is contacted.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading

from native_project import initialize, tool
from process_roles import Rpc

requests = []
VERDICT = json.dumps({"ok": True, "reason": "the controlled criterion holds"})


class Provider(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        assert self.path == "/api/tags", self.path
        self.reply("application/json", json.dumps({"models": [{"name": "desktop-test"}]}).encode())

    def do_POST(self):
        assert self.path == "/api/chat", self.path
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        prompt = body["messages"][-1]["content"]
        judged = '"ok"' in prompt
        requests.append("judge" if judged else "agent")
        text = VERDICT if judged else "desktop work response"
        content = json.dumps({"message": {"content": text}, "done": False}).encode() + b"\n"
        content += b'{"done":true,"done_reason":"stop","prompt_eval_count":4,"eval_count":3}\n'
        self.reply("application/x-ndjson", content)

    def reply(self, kind, body):
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def setup(root):
    (root / ".magistr").mkdir()
    (root / ".magistr" / "project.json").write_text(json.dumps({
        "project": {"name": "desktop-work", "description": "", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                  "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"},
        "roadmap_backend": "think-and-ship",
        "roadmap_exports": False,
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "true", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^\\s*- \\[ \\]"},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [],
        "state_prefix": "desktop-work",
    }))
    (root / "sample.txt").write_text("before\n")
    (root / ".gitignore").write_text(".magistr/\n.workbench/\ntas-data/\n")
    for args in [("init", "-q"), ("add", "."), ("-c", "user.name=Desktop test",
                 "-c", "user.email=desktop@example.invalid", "commit", "-qm", "Fixture")]:
        subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)


def main():
    binary = Path(sys.argv[1]).resolve()
    driver = Path(__file__).with_suffix(".swift").resolve()
    with tempfile.TemporaryDirectory(prefix="magistr-desktop-work-") as directory:
        root = Path(directory) / "project"
        root.mkdir()
        config = Path(directory) / "config" / "magistr"
        config.mkdir(parents=True)
        (config / "config.toml").write_text(
            '[session]\ndefault_provider = "ollama"\n[models]\nollama = "desktop-test"\n'
            "[general]\nauto_pause = false\n")
        setup(root)
        with Rpc(["mcp-launch", "magistr", str(root)], root) as workflow:
            initialize(workflow)
            tool(workflow, "roadmap_add_chunk", {
                "id": "work", "title": "Do the controlled work", "status": "pending",
                "acceptance": ["The controlled criterion holds"],
            })
            # The execution trace an agent with tools would open itself: the controlled
            # peer only chats, and the loop records its gates on the active task.
            tool(workflow, "ship_set_objective", {"description": "Do the work", "scope": "chunk:work"})
            tool(workflow, "ship_plan", {
                "action": "add", "task_id": "impl", "title": "Implement", "task_type": "implement",
            })
            tool(workflow, "ship_start", {"task_id": "impl"})
        server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        env = dict(os.environ, OLLAMA_HOST=f"http://127.0.0.1:{server.server_port}",
                   XDG_CONFIG_HOME=str(config.parent), MAGISTR_GOAL="native")
        try:
            result = subprocess.run(
                ["swift", str(driver), str(binary), str(root)], env=env,
                capture_output=True, text=True, timeout=300,
            )
            print(result.stdout, end="")
            assert result.returncode == 0, result.stderr
            state = json.loads((root / ".magistr" / "loop-state.json").read_text())
            records = state["operations"]["records"]
            outcomes = {r["target"]: r["outcome"] for r in records}
            assert outcomes.get("complete_chunk:work") == "completed", outcomes
            assert outcomes.get("dispatch") == "completed", outcomes
            # Accepted, running and indeterminate are the unsettled outcomes.
            assert all(o in ("completed", "failed", "cancelled") for o in outcomes.values()), outcomes
            assert requests == ["agent", "judge"], requests
            with Rpc(["mcp-launch", "magistr", str(root)], root) as workflow:
                initialize(workflow)
                chunk = tool(workflow, "roadmap_get", {"ids": ["work"]})["records"][0]
                assert chunk["status"] == "done", chunk
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
    print("PASS: the window's Start loop ran a judged iteration, completed the item and settled every operation")


if __name__ == "__main__":
    main()
