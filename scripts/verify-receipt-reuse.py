"""Exercise receipt reuse through the real CLI/MCP owner, without repeating a check."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    with tempfile.TemporaryDirectory(prefix="magistr-reuse-smoke-") as temporary:
        base = Path(temporary).resolve()
        root = base / "project"
        root.mkdir()
        (root / ".magistr").mkdir()
        (root / ".gitignore").write_text("calls.log\n", encoding="utf-8")
        (root / "source.txt").write_text("ok\n", encoding="utf-8")
        (root / "metadata.json").write_text("{}", encoding="utf-8")
        (root / "verify.py").write_text(
            "from pathlib import Path\n"
            "with Path('calls.log').open('a') as stream: stream.write('ran\\n')\n"
            "assert Path('source.txt').read_text() == 'ok\\n'\n", encoding="utf-8")
        (root / ".magistr/project.json").write_text(json.dumps({
            "project": {"name": "receipt-smoke", "description": "", "type": "cli"},
            "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                      "test_framework": "cargo-test", "linter": "clippy", "formatter": "rustfmt"},
            "quality_gate": {"check": "cargo check", "test": "cargo test", "lint": "cargo clippy"},
            "format": {"extensions": ["rs"], "command": "cargo fmt"},
            "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
            "reference_repos": [], "state_prefix": "receipt-smoke", "roadmap_exports": False,
        }), encoding="utf-8")
        env = os.environ.copy()
        env["MAGISTR_WORKFLOW_DATA_DIR"] = str(base / "store")
        with (base / "server.log").open("w", encoding="utf-8") as stderr:
            process = subprocess.Popen([str(binary), "mcp", "--dir", str(root)], env=env,
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                text=True, encoding="utf-8", creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
            serial = 0
            def rpc(method, params):
                nonlocal serial
                serial += 1
                process.stdin.write(json.dumps(dict(jsonrpc="2.0", id=serial, method=method, params=params)) + "\n")
                process.stdin.flush()
                while True:
                    line = process.stdout.readline()
                    if not line:
                        raise RuntimeError((base / "server.log").read_text(encoding="utf-8"))
                    result = json.loads(line)
                    if result.get("id") == serial:
                        if "error" in result:
                            raise AssertionError(result["error"])
                        return result["result"]
            def call(name, arguments, *, allow_error=False):
                result = rpc("tools/call", dict(name=name, arguments=arguments))
                value = result.get("structuredContent")
                if value is None:
                    value = json.loads(next(item["text"] for item in result["content"] if item["type"] == "text"))
                if not allow_error:
                    assert not result.get("isError") and value.get("ok", True), value
                return value
            def operation(name, arguments, **options):
                return call("execute", {"source": "return magistr.call(" + json.dumps(name) + "," +
                                       json.dumps(arguments) + ");"}, **options)
            try:
                rpc("initialize", dict(protocolVersion="2024-11-05", capabilities={},
                    clientInfo=dict(name="receipt-reuse-smoke", version="1.0")))
                process.stdin.write(json.dumps(dict(jsonrpc="2.0", method="notifications/initialized")) + "\n")
                process.stdin.flush()
                schema = rpc("tools/list", {})
                check = next(tool for tool in schema["tools"] if tool["name"] == "check_work")
                assert "reuse_only" in check["inputSchema"]["properties"], "server lacks reuse-only capability"
                operation("roadmap_add_chunk", dict(id="reuse", title="Verify reuse", group="Verification"))
                operation("roadmap_start_chunk", dict(id="reuse"))
                operation("ship_set_objective", dict(description="Verify reuse", scope="chunk:reuse",
                                                     verification="focused"))
                call("plan_work", dict(action="add", task_id="verify", title="Verify", task_type="test"))
                call("start_work", dict(task_id="verify"))
                request = dict(task_id="verify", name="source-test", type="test", required=True,
                               command="python verify.py", reuse=True,
                               inputs=["verify.py", "source.txt", "metadata.json"])
                first = call("check_work", request)
                assert first["check"]["value"]["passed"], first
                assert first["receipt"]["execution"] == dict(kind="exited", code=0), first
                (root / "metadata.json").write_text('{"finished":true}', encoding="utf-8")
                request.update(inputs=["verify.py", "source.txt"], reuse_only=True)
                reused = call("check_work", request)
                assert reused["check"]["value"]["passed"], reused
                assert reused["receipt"]["execution"] == dict(kind="reused", receipt_id=first["receipt"]["id"]), reused
                assert (root / "calls.log").read_text().splitlines() == ["ran"]
                operation("roadmap_complete_chunk", dict(id="reuse", ship_ref="task:verify"))
                (root / "source.txt").write_text("changed\n", encoding="utf-8")
                refused = call("check_work", request)
                check = refused["check"]["value"]
                assert not check["passed"] and not check["verified"] and check.get("exit_code") is None, refused
                assert refused["receipt"]["execution"]["kind"] == "reuse_unavailable", refused
                assert (root / "calls.log").read_text().splitlines() == ["ran"]
                closure = operation("roadmap_complete_chunk", dict(id="reuse", ship_ref="task:verify"),
                                    allow_error=True)
                assert not closure["ok"], closure
                print(json.dumps(dict(command_runs=1, reused=True, changed_input_ran=False,
                                      unchanged_completion=True, changed_completion_refused=True,
                                      original_receipt=first["receipt"]["id"]), sort_keys=True))
            finally:
                process.stdin.close()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=10)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
