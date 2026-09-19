"""Project-owner half of native_live.py; every path belongs to its disposable project."""
import json
import subprocess

from process_roles import Rpc


def setup(root, binary):
    (root / ".magistr").mkdir()
    (root / ".magistr" / "project.json").write_text(json.dumps({
        "project": {"name": "desktop-acceptance", "type": "cli"},
        "roadmap_exports": False,
        "quality_gate": {"check": "python3 check.py", "test": "python3 check.py",
                         "lint": "python3 check.py"},
    }))
    (root / ".mcp.json").write_text(json.dumps({"mcpServers": {
        target: {"command": str(binary), "args": ["mcp-launch", target, str(root)]}
        for target in ("magistr", "workbench-mcp")
    }}))
    (root / "check.py").write_text(
        'from pathlib import Path\nassert Path("sample.txt").read_text() == "before\\n"\n')
    (root / ".gitignore").write_text(".magistr/\n.workbench/\n")
    for args in [("init", "-q"), ("add", "."), ("-c", "user.name=Desktop test",
                 "-c", "user.email=desktop@example.invalid", "commit", "-qm", "Fixture")]:
        subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)


def initialize(rpc):
    rpc.request("initialize", {
        "protocolVersion": "2025-03-26", "capabilities": {},
        "clientInfo": {"name": "desktop-native-acceptance", "version": "1"},
    })
    rpc.send({"jsonrpc": "2.0", "method": "notifications/initialized"})


def tool(rpc, name, arguments, refused=False):
    result = rpc.request("tools/call", {
        "name": name, "arguments": arguments,
        "_meta": {"workbench/sessionId": "desktop-acceptance"},
    })
    data = result.get("structuredContent")
    if data is None:
        text = next(c["text"] for c in result.get("content", []) if c["type"] == "text")
        try:
            data = json.loads(text)
        except json.JSONDecodeError:
            data = {"text": text}
    if not refused:
        assert not result.get("isError") and data.get("ok", True), (name, data)
    return data


def exercise(window, root):
    with Rpc(["mcp-launch", "workbench-mcp", str(root)], root) as workspace, Rpc(
        ["mcp-launch", "magistr", str(root)], root
    ) as workflow:
        initialize(workspace)
        initialize(workflow)
        content = tool(workspace, "read", {"file": "sample.txt"})
        assert any("before" in r.get("text", "") for r in content["results"])
        missing = tool(workspace, "run", {"command": "missing-check"}, refused=True)
        assert missing["error"]["code"] == "COMMAND_NOT_DECLARED", missing
        print("PASS: source read and refused missing command through the desktop owner", flush=True)

        move = {"op": "move", "from": "sample.txt", "to": "moved.txt"}
        pending = tool(workspace, "files", move, refused=True)
        assert pending["error"]["code"] == "CONFIRM_REQUIRED", pending
        assert (root / "sample.txt").exists() and not (root / "moved.txt").exists()
        assert any("sample.txt" in note for note in pending.get("notes", [])), pending
        confirmed = tool(workspace, "files", dict(move, confirm=pending["error"]["confirmToken"]))
        sequence = next(r["journalSeq"] for r in confirmed["results"] if r.get("applied"))
        assert not (root / "sample.txt").exists() and (root / "moved.txt").read_text() == "before\n"
        tool(workspace, "undo", {"ops": [sequence], "scope": "session"})
        assert (root / "sample.txt").read_text() == "before\n" and not (root / "moved.txt").exists()
        print("PASS: reviewed source confirmation and journal undo", flush=True)

        tool(workflow, "roadmap_add_chunk", {
            "id": "sample", "title": "Check the sample", "acceptance": ["The sample check is current"],
        })
        tool(workflow, "roadmap_start_chunk", {"id": "sample"})
        tool(workflow, "ship_set_objective", {
            "description": "Check the sample", "scope": "chunk:sample", "verification": "focused",
            "acceptance_criteria": ["The recorded check follows the current sample"],
        })
        tool(workflow, "ship_plan", {
            "action": "add", "task_id": "sample-check", "title": "Check sample", "task_type": "test",
        })
        tool(workflow, "ship_start", {"task_id": "sample-check"})
        check = tool(workflow, "ship_check", {
            "name": "sample-content", "type": "test", "command": "python3 check.py",
        })
        assert check["verified"] and check["passed"] and check["exit_code"] == 0, check
        print("PASS: check executed and recorded by the desktop workflow owner", flush=True)

        window.send(
            "Use only workbench-mcp tools: read sample.txt, then use edit with op replace_text "
            "to change before to after in sample.txt. Do not use shell, apply_patch, or workflow tools. "
            "If any tool refuses, report the complete error verbatim. On success reply exactly SOURCE_DONE."
        )
        for _ in range(3):
            rows = window.wait("source permission or result", lambda rows:
                window.has("Allow")(rows) or window.has("Send", enabled=False)(rows))
            if window.has("Send", enabled=False)(rows):
                break
            window.call("press", name="Allow", role="AXButton")
        window.reply("SOURCE_DONE")
        assert (root / "sample.txt").read_text() == "after\n"
        journal = tool(workspace, "undo", {"action": "list", "scope": "workspace", "diff": True})
        assert any(r.get("path") == "sample.txt" and "+after" in r.get("diff", "")
                   for r in journal["results"]), journal
        refusal = tool(workflow, "roadmap_complete_chunk", {
            "id": "sample", "ship_ref": "task:sample-check",
        }, refused=True)
        assert "source changed since the gate ran" in json.dumps(refusal), refusal
        print("PASS: changed source invalidates the earlier server check", flush=True)
        window.command("changes")
        window.wait("recorded source change", lambda rows: any(r["name"] == "sample.txt" for r in rows))
        window.call("click", name="sample.txt", last=True)
        window.wait("readable change diff", window.text("+after"))
        window.call("press", name="Open", role="AXButton")
        window.wait("source file opened", lambda rows: any(
            r["name"] == "inspector" and "after" in r["value"] and "+after" not in r["value"]
            for r in rows))
        window.call("key", code=53)  # Escape closes the file viewer.
        window.wait("Undo available", window.has("Undo"))
        window.call("key", name="Undo", role="AXButton", code=36)
        window.wait("source undo completed", lambda rows: (root / "sample.txt").read_text() == "before\n")
        print("PASS: native pointer inspection, source Open and keyboard Undo", flush=True)
        check = tool(workflow, "ship_check", {
            "name": "sample-content", "type": "test", "command": "python3 check.py",
        })
        assert check["verified"] and check["passed"] and check["exit_code"] == 0, check
        tool(workflow, "ship_complete", {"task_id": "sample-check"})
        complete = tool(workflow, "roadmap_complete_chunk", {
            "id": "sample", "ship_ref": "task:sample-check",
        })
        assert complete["status"] == "done", complete
        tool(workflow, "ship_finalize", {"summary": "Source restored and current check passed"})
        print("PASS: current check permits completion through the same workflow owner", flush=True)

