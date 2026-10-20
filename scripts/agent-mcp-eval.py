#!/usr/bin/env python3
"""Opt-in comparison of two Magistr binaries in disposable projects."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

PROMPT = """Work only in this disposable project using its configured MCP tools.
Do not use subagents, other servers or external services. This is an authorized evaluation.
Investigate names.py and its test using source tools, identify the defect, and fix it
with a current content hash. Run the actual unittest command through recorded workflow
verification. Create a focused objective with a small plan, start the task, record
a concise design decision and the change, and capture the observed defect as feedback. Add one
small future improvement to the native roadmap (create a phase if needed).
Save a concise factual checkpoint for a fresh session, with observed evidence and\na decision summary. Do not record private chain-of-thought or hidden model reasoning. Finish the task after the check
passes, but leave the objective open for a second session to inspect and finalize.
Use discover for schemas if it is present; use execute with synchronous
magistr.call(name, args) for advanced operations. Inspect failures and recover
from current state; do not replay completed changes. Do not change the tests.
In your final response explain the source defect and report the verification result.
"""
RESUME = """Resume the existing work in this disposable project through its MCP tools.
Do not use subagents, other servers or external services. Read context, saved execution,
feedback, roadmap and the saved decision summary. Confirm the earlier task and its recorded
check completed; preserve its source change and all existing records. Finalize that
objective if its required checks pass. Do not create a new objective or repeat a
completed edit/check. Report what was recovered and the outcome. Advanced operations
use discover and execute with magistr.call when those tools are present.
"""

def run(cmd, **kwargs):
    return subprocess.run([str(x) for x in cmd], text=True, check=True, **kwargs)

TEST_SOURCE = "import unittest\nfrom names import normalize_names\nclass Names(unittest.TestCase):\n    def test_whitespace(self):\n        self.assertEqual(normalize_names([' Alice ', '  ', '', 'BOB']), ['alice','bob'])\n"

def required_checks_passed(checks):
    """Keep diagnostic failures as evidence; require the latest required checks."""
    latest = {check["name"]: check for check in checks if check["required"]}
    return bool(latest) and all(check["passed"] and check["verified"]
                               and check["exit_code"] == 0 for check in latest.values())


def verify_saved_work(root):
    """Independently inspect persisted records after both agent processes exit."""
    def saved(family):
        paths=list((root/"state"/family).rglob("sessions/*.json"))
        if len(paths)!=1:raise AssertionError(f"{family}: expected one isolated project, found {len(paths)}")
        return json.loads(paths[0].read_text())
    ship=saved("ship");think=saved("think");signals=saved("signal");roadmap=saved("roadmap")
    checks=[c for task in ship["tasks"] for c in task.get("checks",[])]
    proof=subprocess.run(["python3","-m","unittest","-q"],cwd=root,capture_output=True,text=True)
    result={
        "objectiveCompleted":ship["objective"]["status"]=="completed",
        "tasksCompleted":bool(ship["tasks"]) and all(t["status"]=="completed" for t in ship["tasks"]),
        "recordedCheckPassed":required_checks_passed(checks),
        "feedbackSaved":bool(signals["signals"]),
        "futureRoadmapSaved":any(c["status"] in ("backlog","pending") for c in roadmap["chunks"]),
        "decisionRecordsSaved":any(h["steps"] for h in think["histories"].values()),
        "testUnchanged":(root/"test_names.py").read_text()==TEST_SOURCE,
        "independentSourceCheckPassed":proof.returncode==0,
        "sourceSha256":hashlib.sha256((root/"names.py").read_bytes()).hexdigest(),
        "testSha256":hashlib.sha256((root/"test_names.py").read_bytes()).hexdigest(),
        "verificationCommands":[c["command"] for c in checks],
        "taskCount":len(ship["tasks"])
    }
    result["passed"]=all(v for v in result.values() if isinstance(v,bool))
    return result

def fixture(root):
    root.mkdir()
    run(["git","init","--quiet",root])
    (root/".magistr").mkdir()
    (root/"names.py").write_text("def normalize_names(names):\n    return [name.lower() for name in names if name]\n")
    (root/"test_names.py").write_text(TEST_SOURCE)
    cmd="python3 -m unittest -q"
    (root/".magistr/project.json").write_text(json.dumps({
        "project":{"name":"MCP evaluation","description":"","type":"library"},
        "stack":{"language":"python","framework":"none","package_manager":"pip",
                 "test_framework":"unittest","linter":"none","formatter":"none"},
        "format":{"command":"true","extensions":["py"]},
        "testing":{"companion_pattern":"test_{name}.py","test_dir":"."},
        "reference_repos":[],"state_prefix":"mcp-evaluation","roadmap_exports":False,
        "quality_gate":{"check":cmd,"test":cmd,"lint":cmd},"lifecycle_hooks":[]}))
    (root/".magistr/workspace.toml").write_text('[edit]\nconfirm_mode = "token"\n[commands]\ntest = "python3 -m unittest -q"\n')

def servers(binary, root, surface, env):
    def spec(args):
        return {"command":str(binary),"args":args,"env":env}
    return {"magistr":spec(["mcp","--dir",str(root)])}

def metrics(events, host):
    calls=[]; failures=0; usage=[]; model=None
    for e in events:
        if host=="codex":
            item=e.get("item",{})
            if e.get("type")=="item.completed" and item.get("type")=="mcp_tool_call":
                calls.append(item.get("tool"))
                failures += bool(item.get("error") or item.get("status")=="failed"
                                 or (item.get("result") or {}).get("isError"))
            if e.get("type")=="turn.completed": usage.append(e.get("usage",{}))
        else:
            if e.get("type")=="system": model=e.get("model",model)
            if e.get("type")=="assistant":
                for b in e.get("message",{}).get("content",[]):
                    if b.get("type")=="tool_use": calls.append(b.get("name"))
            if e.get("type")=="user":
                for b in e.get("message",{}).get("content",[]):
                    failures += b.get("type")=="tool_result" and bool(b.get("is_error"))
            if e.get("type")=="result":
                usage.append(e.get("usage",{}))
    return {"toolCalls":len(calls),"tools":calls,"toolErrors":failures,"usage":usage,"model":model,
            "eventBytes":sum(len(json.dumps(e)) for e in events)}

def public_events(events):
    """Retain observable tool evidence without private model thinking blocks."""
    for event in events:
        if event.get("item", {}).get("type") == "reasoning":
            continue
        if event.get("type") == "assistant" and "message" in event:
            event = dict(event, message=dict(event["message"]))
            event["message"]["content"] = [b for b in event["message"].get("content", [])
                if b.get("type") not in ("thinking", "redacted_thinking")]
        yield event


def evaluate(host, binary, surface, directory, timeout):
    root=directory/f"{host}-{surface}"
    fixture(root)
    scoped={"MAGISTR_WORKFLOW_DATA_DIR":str(root/"state"),
            "XDG_CONFIG_HOME":str(root/"user-config"),"XDG_DATA_HOME":str(root/"user-data"),
            "MAGISTR_WORKSPACE_NETWORK_FS":"0"}
    env={k:v for k,v in os.environ.items() if not k.startswith(("MAGISTR_",))}
    env.update(scoped)
    config=servers(binary,root,surface,scoped)
    result={"host":host,"surface":surface,"binary":str(binary),
            "binarySha256":hashlib.sha256(binary.read_bytes()).hexdigest(),"turns":[]}
    for label,prompt in [("work",PROMPT),("resume",RESUME)]:
        if host=="codex":
            args=["codex","exec","--ignore-user-config","--ignore-rules","--ephemeral",
                  "--skip-git-repo-check","--sandbox","workspace-write","--json",
                  "-c",'approval_policy="never"']
            for name,s in config.items():
                prefix=f"mcp_servers.{name}"
                env_table="{"+",".join(json.dumps(k)+"="+json.dumps(v) for k,v in s["env"].items())+"}"
                args.extend(["-c",f'{prefix}.command={json.dumps(s["command"])}',
                             "-c",f'{prefix}.args={json.dumps(s["args"])}',
                             "-c",f'{prefix}.env={env_table}',
                             "-c",f'{prefix}.startup_timeout_sec=60',
                             "-c",f'{prefix}.required=true',
                             "-c",f'{prefix}.default_tools_approval_mode="approve"'])
            args+=["-C",str(root),prompt]
        else:
            args=["claude","--print","--output-format","stream-json","--verbose",
                  "--no-session-persistence","--disable-slash-commands","--tools","",
                  "--setting-sources","","--strict-mcp-config","--mcp-config",
                  json.dumps({"mcpServers":config}),"--allowedTools",
                  ",".join(f"mcp__{n}__*" for n in config),"--",prompt]
        started=time.monotonic()
        proc=subprocess.Popen(args,cwd=root,env=env,text=True,stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE,start_new_session=True)
        try:
            stdout,stderr=proc.communicate(timeout=timeout)
            code=proc.returncode
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid,signal.SIGTERM)
            try: stdout,stderr=proc.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(proc.pid,signal.SIGKILL)
                stdout,stderr=proc.communicate()
            code="timeout"
        events=[]
        for line in stdout.splitlines():
            try: events.append(json.loads(line))
            except json.JSONDecodeError: pass
        # Synthetic prompts and fixture paths only; never copy authentication files.
        (directory/f"{host}-{surface}-{label}.jsonl").write_text("\n".join(json.dumps(e) for e in public_events(events))+"\n")
        (directory/f"{host}-{surface}-{label}.stderr").write_text(stderr)
        result["turns"].append({"turn":label,"exit":code,"seconds":round(time.monotonic()-started,2),
                               **metrics(events,host)})
        print(json.dumps(result["turns"][-1]),flush=True)
        if code!=0: break
    proof=subprocess.run(["python3","-m","unittest","-q"],cwd=root,capture_output=True,text=True)
    result["sourceCheckPassed"]=proof.returncode==0
    result["source"]=(root/"names.py").read_text()
    if len(result["turns"])==2 and all(t["exit"]==0 for t in result["turns"]):
        result["independentVerification"]=verify_saved_work(root)
    (directory/f"{host}-{surface}-summary.json").write_text(json.dumps(result,indent=2)+"\n")
    return result

def main():
    p=argparse.ArgumentParser()
    p.add_argument("--candidate",type=Path,required=True)
    p.add_argument("--baseline",type=Path,required=True)
    p.add_argument("--out",type=Path,required=True)
    p.add_argument("--host",choices=["codex","claude"],required=True)
    p.add_argument("--surface",choices=["baseline","hybrid"],required=True)
    p.add_argument("--timeout",type=int,default=300)
    p.add_argument("--verify-existing",action="store_true")
    a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
    binary=(a.baseline if a.surface=="baseline" else a.candidate).resolve()
    if a.verify_existing:
        proof=verify_saved_work(a.out.resolve()/f"{a.host}-{a.surface}")
        (a.out/f"{a.host}-{a.surface}-verification.json").write_text(json.dumps(proof,indent=2)+"\n")
        print(json.dumps(proof))
        if not proof["passed"]:raise SystemExit(1)
    else:
        result=evaluate(a.host,binary,a.surface,a.out.resolve(),a.timeout)
        if not result.get("independentVerification",{}).get("passed"):
            raise SystemExit(1)
if __name__=="__main__":main()
