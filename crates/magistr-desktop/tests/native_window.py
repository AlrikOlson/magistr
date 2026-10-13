#!/usr/bin/env python3
"""Exercise the built macOS window through native input and accessibility.
Only disposable projects, app bundles and a controlled ACP peer are used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import sqlite3
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / "crates/magistr-desktop"
def run(*args, **kwargs):
    return subprocess.run(list(map(str,args)), check=True, capture_output=True, text=True, timeout=kwargs.pop("timeout",30), **kwargs)
def project(path, name):
    (path/".magistr").mkdir(parents=True)
    (path/".magistr/project.json").write_text(json.dumps({
        "project":{"name":name,"description":"","type":"cli"},
        "stack":{"language":"rust","framework":"none","package_manager":"cargo","test_framework":"cargo-test","linter":"clippy","formatter":"rustfmt"},
        "roadmap_backend":"think-and-ship","roadmap_exports":False,
        "quality_gate":{"check":"true","test":"true","lint":"true"},
        "format":{"command":"true","extensions":["rs"]},
        "testing":{"companion_pattern":"{name}.tests.{ext}","test_dir":"alongside"},
        "reference_repos":[],"state_prefix":name}))
    (path/"sample.rs").write_text("pub fn native_journey() {}\n")
    run("git","init",path)
    run("git","-C",path,"add",".")
    run("git","-C",path,"-c","user.name=Native Journey","-c","user.email=journey@example.invalid","commit","-m","fixture")
def source_hashes():
    files = sorted([*SOURCE.glob("src/*.rs"),*SOURCE.glob("ui/*"),SOURCE/"Cargo.toml",SOURCE/"Cargo.lock",SOURCE/"build.rs",SOURCE/"tauri.conf.json"])
    return {str(path.relative_to(ROOT)):hashlib.sha256(path.read_bytes()).hexdigest() for path in files if path.is_file() and not path.name.endswith(".tests.rs")}

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("binary",type=Path)
    parser.add_argument("--evidence",type=Path,default=ROOT/".magistr/desktop-native")
    args=parser.parse_args()
    binary=args.binary.resolve()
    evidence=args.evidence.resolve();evidence.mkdir(parents=True,exist_ok=True)
    driver=evidence/"driver"
    run("swiftc","-parse-as-library",SOURCE/"tests/native_window.swift","-o",driver,timeout=60)
    def call(mode,*values):
        result=run(driver,mode,*values)
        return json.loads(result.stdout) if result.stdout.strip() else None
    environment=call("environment")
    assert environment["accessibility"] and environment["screen_capture"] and environment["post_events"],environment
    front=environment["front_pid"]
    results={"environment":environment,"checks":{}}
    with tempfile.TemporaryDirectory(prefix="magistr-desktop-journey-") as temporary:
        parent=Path(temporary).resolve()
        first=parent/"first";second=parent/"second"
        project(first,"First project");project(second,"Second project")
        worktree=parent/"feature";run("git","-C",first,"worktree","add","-b","feature",worktree)
        bundle=parent/"Magistr Journey.app"
        executable=bundle/"Contents/MacOS/magistr-desktop"
        executable.parent.mkdir(parents=True)
        shutil.copy2(binary,executable)
        identity="dev.magistr.desktop.journey."+str(time.time_ns())
        (bundle/"Contents/Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier":identity,"CFBundleExecutable":"magistr-desktop","CFBundleName":"Magistr Journey","CFBundlePackageType":"APPL","NSHighResolutionCapable":True}))
        run("codesign","--force","--sign","-",bundle)
        tools=parent/"bin";tools.mkdir()
        peer=tools/"copilot"
        peer.write_text("#!/usr/bin/python3\nimport os,json\nfrom pathlib import Path\np=Path(os.environ['JOURNEY_PEERS']);p.mkdir(exist_ok=True)\n(p/(str(os.getpid())+'.json')).write_text(json.dumps({'pid':os.getpid(),'parent':os.getppid(),'workspace':os.getcwd()}))\nos.execv("+repr(str(executable))+",["+repr(str(executable))+",'acp-agent','--provider','scripted'])\n")
        peer.chmod(0o755)
        peers=parent/"peers"
        env=dict(os.environ,PATH=str(tools)+os.pathsep+os.environ["PATH"],HOME=str(parent/"home"),XDG_DATA_HOME=str(parent/"data"),XDG_CONFIG_HOME=str(parent/"config"),MAGISTR_HOME=str(parent/"state"),MAGISTR_WORKSPACE_NETWORK="0",JOURNEY_PEERS=str(peers),RUST_LOG="info")
        launch=parent/"launch.json";launch.write_text(json.dumps({"arguments":[str(first),"--provider","copilot","--reduce-motion"],"environment":env}))
        started=time.monotonic()
        log=(evidence/"launch.log").open("w")
        process=subprocess.Popen([str(executable),"--dir",str(first),"--provider","copilot","--trust","cautious","--reduce-motion"],env=env,cwd=first,stdout=log,stderr=log)
        pid=process.pid
        def rows():
            if process.poll() is not None:
                raise AssertionError("Desktop exited: "+(evidence/"launch.log").read_text())
            try:return call("ax",pid)
            except subprocess.CalledProcessError as error:
                if process.poll() is not None:raise AssertionError((evidence/"launch.log").read_text()) from error
                return [{"diagnostic":error.stderr}]
        def text_has(value):
            return lambda tree:any(value in str(row.get(key,"")) for row in tree for key in ("AXTitle","AXDescription","AXValue"))
        def wait(label,predicate,timeout=12):
            deadline=time.monotonic()+timeout;tree=[]
            while time.monotonic()<deadline:
                tree=rows()
                if predicate(tree):
                    results["checks"][label]=True;print("PASS:",label,flush=True);return tree
                time.sleep(.05)
            (evidence/"failure-ax.json").write_text(json.dumps(tree,ensure_ascii=False,indent=2))
            raise AssertionError(label+": "+json.dumps(tree,ensure_ascii=False)[-6000:])
        def draft(value):
            call("focus",pid,"Message the agent");call("key",pid,0,1048576);call("paste",pid,value)
        def send(value,method="key"):
            draft(value)
            if method=="key":call("key",pid,36,0)
            else:call(method,pid,"Send")
        def choose_theme(label,down):
            call("press",pid,"Appearance");call("press",pid,label.capitalize());time.sleep(.15)
            call("screenshot",pid,evidence/(label+".png"))
        try:
            wait("real window opened",lambda tree:text_has("First project")(tree) and any(r.get("AXRole")=="AXTextArea" and r.get("enabled") and r.get("AXTitle")=="Message the agent" for r in tree))
            results["launch_to_window_ms"]=(time.monotonic()-started)*1000
            call("press",pid,"Close session")
            wait("closing the last session disables session actions",lambda tree:all(any(r.get("AXRole")=="AXButton" and r.get("AXTitle")==label and not r.get("enabled") for r in tree) for label in ("Close session","New session")))
            call("press",pid,"First project")
            wait("project reopens after the last session closes",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("enabled") for r in tree))
            unicode="日本語 · café · 👩🏽‍💻\nA second line"
            draft(unicode)
            tree=wait("native Unicode and multiline editing",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("AXValue")==unicode for r in tree))
            call("key",pid,0,1048576)
            assert call("copy",pid)["text"]==unicode
            results["checks"]["native clipboard exact bytes"]=True
            send("first reply café",method="click")
            wait("pointer Send and streamed reply",text_has("[scripted-fast] echo: first reply café"))
            draft("timed keyboard reply")
            results["latency"]=call("send-latency",pid,"[scripted-fast] echo: timed keyboard reply")
            wait("keyboard Return sends",text_has("[scripted-fast] echo: timed keyboard reply"))
            send("permission allow")
            wait("permission offered",text_has("Allow once"))
            wait("requested action is visible",text_has("scripted-action"))
            call("press",pid,"Allow once")
            wait("permission allowed",text_has("permission: allowed"))
            send("permission deny")
            wait("denial offered",text_has("Deny"))
            call("focus",pid,"Deny");call("key",pid,49,0)
            wait("permission denied by keyboard",text_has("permission: rejected"))
            send("hang")
            wait("live cancellable work",text_has("hanging until cancel"))
            draft("Keep my unsent draft")
            call("click",pid,"Stop")
            wait("cancel visible",text_has("Stopped"))
            wait("draft survived cancellation",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("AXValue")=="Keep my unsent draft" for r in tree))
            call("press",pid,"New session")
            wait("second resident session",text_has("First project session 3"))
            draft("Second draft")
            call("press",pid,"First project session 2")
            wait("session switch restores draft",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("AXValue")=="Keep my unsent draft" for r in tree))
            call("press",pid,"Working copy");call("press",pid,"feature")
            wait("working copy opened",text_has(str(worktree)))
            call("key",pid,31,1048576)
            wait("keyboard project picker",text_has("Open a project"))
            call("focus",pid,"Project folder");call("paste",pid,str(second))
            call("press",pid,"Open")
            wait("second project admitted",text_has(str(second)))
            wait("resident project appears before history settlement",lambda tree:any(r.get("AXRole")=="AXButton" and (r.get("AXTitle")=="Second project" or r.get("AXDescription")=="Second project") for r in tree))
            send("second project response")
            wait("second project streams independently",text_has("[scripted-fast] echo: second project response"))
            copied=call("copy-button",pid,"Copy conversation")["text"]
            assert "[scripted-fast] echo: second project response" in copied,copied
            results["checks"]["Copy conversation uses native clipboard"]=True
            choose_theme("dark",1);choose_theme("light",2)
            call("resize",pid,800,680);call("screenshot",pid,evidence/"narrow.png")
            call("ax",pid,evidence/"accessibility.json")
            draft("")
            sources=environment["input_sources"]
            japanese=next((s for s in sources if "RomanTyping" in s and s.endswith(".Japanese")),next((s for s in sources if s.endswith(".Japanese")),None))
            if japanese:
                call("ime",pid,japanese,evidence/"ime.json")
                observed=json.loads((evidence/"ime.json").read_text())
                values=[r.get("AXValue","") for r in observed["committed"] if r.get("AXRole")=="AXTextArea"]
                results["ime"]={"source":observed["actual_source"],"committed":values,"native_composition":any(any(0x3040<=ord(c)<=0x30ff or 0x4e00<=ord(c)<=0x9fff for c in v) for v in values)}
                # A reported source alone does not prove native preedit activation.
                if observed["native_preedit"]:
                    assert results["ime"]["native_composition"],values
                    results["checks"]["IME commit keeps unsent draft"]=True
                else:
                    results["ime"]["reason"]="The reported Japanese source delivered Latin preedit; conversion automation is unavailable."
                    print("UNAVAILABLE: native Japanese preedit; observation retained",flush=True)
            else:
                results["ime"]={"native_composition":False,"reason":"Japanese input source unavailable"}
            call("press",pid,"Close session")
            wait("closed session returns to another resident",text_has("Conversation"))
            assert len(list(peers.glob("*.json")))>=4
        finally:
            if process.poll() is None:
                try:call("quit",pid)
                except subprocess.CalledProcessError:process.terminate()
            code=process.wait(timeout=12);log.close()
            assert code==0,("Native Quit exit",code,(evidence/"launch.log").read_text()[-3000:])
            deadline=time.monotonic()+12
            while time.monotonic()<deadline:
                try:os.kill(pid,0)
                except ProcessLookupError:break
                time.sleep(.1)
            else:raise AssertionError("Desktop did not settle after Quit")
            call("restore-front",front)
        for path in peers.glob("*.json"):
            child=json.loads(path.read_text())["pid"]
            try:os.kill(child,0)
            except ProcessLookupError:continue
            raise AssertionError("Provider remains after Quit: "+str(child))
        # Admission after Quit must acquire the released project owner.
        reopened=run(executable,"mcp-launch","workbench-mcp",first,input="",env=env,timeout=20)
        results["checks"]["quit settles providers and releases owner"]=True
        database=parent/"data/magistr/sessions.db"
        with sqlite3.connect(database) as history:
            history_rows=history.execute("SELECT workspace,turns,outcome FROM sessions").fetchall()
        print("HISTORY:",history_rows,flush=True)
        assert len(history_rows)==5,history_rows
        results["history"]=[{"workspace":Path(w).name,"turns":t,"outcome":o} for w,t,o in history_rows]
        results["checks"]["session history written by application"]=True
    results["source_hashes"]=source_hashes()
    (evidence/"receipt.json").write_text(json.dumps(results,ensure_ascii=False,indent=2)+"\n")
    print("PASS: native desktop journey, captured light/dark/narrow windows and accessibility tree")
if __name__=="__main__":main()