#!/usr/bin/env python3
"""Exercise the shipping unified MCP process against disposable projects."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
import unittest

BINARY = Path(os.environ.get("MAGISTR_TEST_BINARY", "target/debug/magistr")).resolve()

class Connection:
    def __init__(self, root, data, access="full", command=None):
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(("MAGISTR_", "MAGISTR_WORKSPACE_", "MAGISTR_WORKFLOW_"))}
        env.update(MAGISTR_WORKFLOW_DATA_DIR=str(data),
                   XDG_CONFIG_HOME=str(data / "config"), XDG_DATA_HOME=str(data / "user"))
        self.process = subprocess.Popen(
            command or [str(BINARY), "mcp", "--dir", str(root), "--access", access], cwd=root, env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            text=True, bufsize=1)
        self.messages = queue.Queue()
        self.errors = []
        self.serial = 0
        def read():
            for line in self.process.stdout:
                self.messages.put(json.loads(line))
            self.messages.put(None)
        def errors():
            self.errors.extend(self.process.stderr)
        threading.Thread(target=read, daemon=True).start()
        threading.Thread(target=errors, daemon=True).start()
        self.ask("initialize", {"protocolVersion":"2025-06-18",
            "capabilities":{}, "clientInfo":{"name":"unified-check","version":"1"}})
        self.send({"jsonrpc":"2.0","method":"notifications/initialized"})

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def begin(self, method, params):
        self.serial += 1
        self.send({"jsonrpc":"2.0","id":self.serial,"method":method,"params":params})
        return self.serial

    def answer(self, serial, timeout=40):
        end = time.monotonic() + timeout
        while True:
            message = self.messages.get(timeout=max(0.01,end-time.monotonic()))
            if message is None:
                raise RuntimeError("MCP ended: " + "".join(self.errors))
            if message.get("method") == "elicitation/create":
                self.send({"jsonrpc":"2.0","id":message["id"],
                           "result":{"action":"decline"}})
            elif message.get("id") == serial:
                if "error" in message:
                    raise RuntimeError(json.dumps(message["error"]))
                return message["result"]

    def ask(self, method, params):
        return self.answer(self.begin(method, params))

    def tool(self, name, arguments):
        result = self.ask("tools/call", {"name":name,"arguments":arguments})
        if "structuredContent" in result:
            return result["structuredContent"]
        return json.loads(result["content"][0]["text"])

    def execute(self, source, **extra):
        return self.tool("execute", {"source":source, **extra})

    def operation(self, name, args):
        result = self.execute("return magistr.call(" + json.dumps(name) + "," + json.dumps(args) + ");")
        if not result["ok"]:
            raise AssertionError(json.dumps(result))
        return result["value"]

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise AssertionError("Unified MCP did not shut down")
        for stream in [self.process.stdin, self.process.stdout, self.process.stderr]:
            stream.close()

def fixture(root):
    (root / ".magistr").mkdir(parents=True)
    (root / ".magistr/project.json").write_text(json.dumps({
        "project":{"name":"unified-test","description":"","type":"cli"},
        "stack":{"language":"rust","framework":"none","package_manager":"cargo",
                 "test_framework":"cargo-test","linter":"clippy","formatter":"rustfmt"},
        "quality_gate":{"check":"true","test":"true","lint":"true"},
        "format":{"command":"true","extensions":["rs"]},
        "roadmap_exports":False, "roadmap_backend":"native",
        "testing":{"companion_pattern":"{name}.tests.{ext}","test_dir":"alongside"},
        "reference_repos":[], "state_prefix":"unified-test"
    }))
    (root / ".magistr/workspace.toml").write_text('[commands]\nslow = "sleep 30"\n')
    (root / "hello.txt").write_text("hello\n")
    (root / "lib.rs").write_text("pub fn answer() -> u32 { 42 }\n")

class UnifiedMcp(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="magistr unified ")
        self.base = Path(self.directory.name)
        self.root = self.base / "project with spaces"
        fixture(self.root)
        self.clients = []

    def connect(self, root=None):
        client = Connection(root or self.root, self.base / "state")
        self.clients.append(client)
        return client

    def tearDown(self):
        for client in reversed(self.clients):
            client.close()
        self.directory.cleanup()

    def test_planning_authority_checks_each_composed_effect(self):
        c = Connection(self.root, self.base / "state", "planning")
        self.clients.append(c)
        result = c.execute("""magistr.call('read', {file:'hello.txt'});
magistr.call('write', {path:'forbidden.txt',content:'no'});
magistr.call('signal_capture', {kind:'feedback',body:'must not run'});""")
        self.assertFalse(result["ok"])
        self.assertEqual(len(result["receipts"]),1)
        self.assertIn("not allowed",result["failure"]["message"])
        self.assertFalse((self.root/"forbidden.txt").exists())
        plan = c.operation("roadmap_add_chunk", {"id":"future","title":"Future improvement","status":"pending"})
        self.assertIsNotNone(plan)
        for operation, args in [("roadmap_set_status",{"id":"future","status":"done"}),
                                ("script",{"source":"return 1;"}),
                                ("check_work",{"name":"forbidden","type":"test","command":"true"})]:
            stopped = c.execute("return magistr.call("+json.dumps(operation)+","+json.dumps(args)+");")
            self.assertFalse(stopped["ok"],stopped)

    def test_catalog_and_schema_coverage(self):
        c = self.connect()
        tools = {t["name"]:t for t in c.ask("tools/list",{})["tools"]}
        self.assertEqual(set(tools), {"context","search","read","edit","write",
            "plan_work","start_work","record_progress","check_work","finish_work","discover","execute"})
        operations = {}
        offset = 0
        while True:
            page = c.tool("discover", {"offset":offset,"limit":50})
            operations.update((op["name"], op) for op in page["operations"])
            if page["nextOffset"] is None: break
            offset = page["nextOffset"]
        for name in ["map","files","run","undo","script","workspace_status","roadmap_add_chunk",
                     "think_record_step","ship_set_objective","ship_finalize","signal_capture"]:
            self.assertIn(name, operations)
        for name, tool in tools.items():
            if name in ("discover","execute"): continue
            discovered = c.tool("discover",{"operation":name})
            self.assertEqual(discovered["operation"]["inputSchema"],tool["inputSchema"])
        self.assertNotIn("magistr_delete",operations)
        self.assertNotIn("magistr_list",operations)

    def test_safe_edits_partial_failure_and_session_undo(self):
        a,b = self.connect(),self.connect()
        first = a.tool("read",{"file":"hello.txt"})
        content_hash = first["results"][0]["contentHash"]
        script = (
            'magistr.call("edit", {op:"replace_text",file:"hello.txt",old:"hello",new:"changed",expectedHash:' +
            json.dumps(content_hash) + '});\n' +
            'magistr.call("write", {path:".git/forbidden",content:"no"});\n' +
            'return magistr.call("write", {path:"never.txt",content:"no"});')
        result = a.execute(script)
        self.assertFalse(result["ok"],result)
        self.assertEqual(len(result["receipts"]),2,result)
        self.assertEqual((self.root/"hello.txt").read_text(),"changed\n")
        self.assertFalse((self.root/"never.txt").exists())
        stale = b.tool("edit",{"op":"replace_text","file":"hello.txt","old":"changed","new":"oops","expectedHash":content_hash})
        self.assertEqual(stale["error"]["code"],"STALE_ANCHOR")
        b.tool("write",{"path":"b.txt","content":"other session"})
        a.operation("undo",{"steps":1})
        self.assertEqual((self.root/"hello.txt").read_text(),"hello\n")
        self.assertEqual((self.root/"b.txt").read_text(),"other session")
        rejected = a.tool("edit",{"op":"replace_text","file":"lib.rs","old":"42","new":"{"})
        self.assertEqual(rejected["error"]["code"],"PARSE_REJECTED")

    def test_workflow_completion_and_startup_does_not_repair(self):
        (self.root/".claude/rules").mkdir(parents=True)
        path = self.root/".claude/rules/tools.md"
        path.write_text("customized instructions\n")
        c=self.connect()
        self.assertEqual(path.read_text(),"customized instructions\n")
        c.operation("ship_set_objective",{"description":"Verify unified tools","scope":"test","verification":"focused"})
        c.tool("plan_work",{"action":"add","task_id":"verify","title":"Verify","task_type":"test"})
        c.tool("start_work",{"task_id":"verify"})
        check=c.tool("check_work",{"name":"fixture check","type":"test","command":"true"})
        self.assertTrue(check["check"]["value"]["passed"],check)
        c.tool("finish_work",{"task_id":"verify"})
        final=c.operation("ship_finalize",{})
        self.assertNotEqual(final.get("ok"),False,final)
        result=c.execute('return magistr.call("roadmap_export", {});')
        self.assertFalse(result["ok"],result)
        self.assertFalse((self.root/"ROADMAP.md").exists())

    def test_bounds_and_confirmation(self):
        c=self.connect()
        result=c.execute('magistr.call("write",{path:"one.txt",content:"one"}); magistr.call("write",{path:"two.txt",content:"two"});',maxOperations=1)
        self.assertFalse(result["ok"],result)
        self.assertTrue((self.root/"one.txt").exists())
        self.assertFalse((self.root/"two.txt").exists())
        result=c.execute('while (true) {}')
        self.assertFalse(result["ok"],result)
        self.assertEqual(result["receipts"],[])
        args={"path":"large.txt","content":"line\n"*210}
        pending=c.tool("write",args)
        self.assertEqual(pending["error"]["code"],"CONFIRM_REQUIRED",pending)
        confirmation=pending["error"]["confirmToken"]
        changed=dict(args,content="different\n"*210,confirm=confirmation)
        rejected=c.tool("write",changed)
        self.assertEqual(rejected["error"]["code"],"CONFIRM_MISMATCH",rejected)
        self.assertFalse((self.root/"large.txt").exists())
        applied=c.tool("write",dict(args,confirm=confirmation))
        self.assertTrue(applied["ok"],applied)

    def test_cancellation_stops_a_native_command(self):
        c = self.connect()
        serial = c.begin("tools/call", {"name":"execute", "arguments":{
            "source":'magistr.call("run", {command:"slow"}); return magistr.call("write", {path:"after.txt",content:"no"});'
        }})
        def running_sleep():
            output = subprocess.check_output(["ps", "-axo", "pid=,ppid=,comm="], text=True)
            processes = [line.strip().split(None, 2) for line in output.splitlines()]
            descendants = {c.process.pid}
            for _ in range(8):
                descendants.update(int(pid) for pid, parent, _ in processes if int(parent) in descendants)
            return any(int(pid) in descendants and command.endswith("sleep") for pid, _, command in processes)
        deadline = time.monotonic() + 10
        while not running_sleep() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertTrue(running_sleep(), "the native command must start before cancellation")
        c.send({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":serial,"reason":"test cancellation"}})
        deadline = time.monotonic() + 10
        while running_sleep() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertFalse(running_sleep(), "cancel must reap the native command")
        self.assertFalse((self.root/"after.txt").exists())
        self.assertTrue(c.tool("read", {"file":"hello.txt"})["ok"])

    def test_owner_restart_never_replays_mutations(self):
        owner,attached=self.connect(),self.connect()
        epoch=attached.tool("context",{})["owner_epoch"]
        attached.tool("write",{"path":"persist.txt","content":"once"})
        owner.process.kill();owner.process.wait()
        # The first request after a killed owner may report the interrupted connection.
        # Only read-only probes are retried, never the write.
        for _ in range(3):
            try:
                current=attached.tool("context",{})
                if current.get("owner_epoch") != epoch and current.get("owner_epoch"):
                    break
            except RuntimeError:
                pass
            time.sleep(0.1)
        self.assertNotEqual(current.get("owner_epoch"),epoch,current)
        for _ in range(3):
            read=attached.tool("read",{"file":"persist.txt"})
            if read.get("ok"): break
        self.assertTrue(read["ok"],read)
        self.assertEqual((self.root/"persist.txt").read_text(),"once")

    def test_separate_worktrees_keep_workspace_ownership(self):
        subprocess.run(["git","init","-q",str(self.root)],check=True)
        subprocess.run(["git","-C",str(self.root),"add","hello.txt"],check=True)
        subprocess.run(["git","-C",str(self.root),"-c","user.name=Test","-c","user.email=test@example.invalid","commit","-qm","fixture"],check=True)
        other=self.base/"linked tree"
        subprocess.run(["git","-C",str(self.root),"worktree","add","-qb","other",str(other)],check=True,stdout=subprocess.DEVNULL)
        fixture_config=json.loads((self.root/".magistr/project.json").read_text())
        (other/".magistr").mkdir()
        (other/".magistr/project.json").write_text(json.dumps(fixture_config))
        a,b=self.connect(),self.connect(other)
        self.assertNotEqual(a.operation("workspace_status",{})["results"][0]["root"],
                            b.operation("workspace_status",{})["results"][0]["root"])
        a.tool("write",{"path":"hello.txt","content":"main"})
        b.tool("write",{"path":"hello.txt","content":"linked"})
        self.assertEqual((self.root/"hello.txt").read_text(),"main")
        self.assertEqual((other/"hello.txt").read_text(),"linked")

if __name__ == "__main__":
    unittest.main(verbosity=2)
