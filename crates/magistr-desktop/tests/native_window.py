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
import sys
import tempfile
import time
from session_controls_fixture import start_api, peer_script
from rich_conversation_journey import exercise_rich_conversation
from attachment_journey import exercise_attachments

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / "crates/magistr-desktop"
def run(*args, **kwargs):
    return subprocess.run(list(map(str,args)), check=True, capture_output=True, text=True, timeout=kwargs.pop("timeout",30), **kwargs)
def finish_fixture(process, log, log_path, call, front):
    primary_error=sys.exc_info()[1]
    errors=[]
    try:
        if process.poll() is None:
            call("quit",process.pid,failure_prefix="cleanup" if primary_error else "failure")
        code=process.wait(timeout=12)
        assert code==0,("Native Quit exit",code,log_path.read_text()[-3000:])
    except Exception as error:
        errors.append(error)
        try:
            if process.poll() is None:process.terminate()
            try:process.wait(timeout=12)
            except subprocess.TimeoutExpired:
                process.kill();process.wait(timeout=5)
        except Exception as settlement:
            errors.append(settlement)
    finally:
        log.close()
        try:call("restore-front",front)
        except Exception as error:errors.append(error)
    if errors:
        if primary_error is None:raise errors[0]
        print("Native cleanup after failure:",repr(errors),file=sys.stderr)

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
    files = sorted([*SOURCE.glob("src/*.rs"),*SOURCE.glob("ui/*"),*SOURCE.glob("tests/rich_conversation*"),*SOURCE.glob("tests/attachment*"),SOURCE/"Cargo.toml",SOURCE/"Cargo.lock",SOURCE/"build.rs",SOURCE/"tauri.conf.json",SOURCE/"tests/native_window.py",SOURCE/"tests/native_window.swift",SOURCE/"tests/session_controls_fixture.py"])
    files.extend(ROOT/path for path in (
        "crates/magistr-application/src/sessions/event.rs",
        "crates/magistr-application/src/sessions/session_activity.rs",
        "crates/magistr-application/src/sessions/session_policy.rs",
        "crates/magistr-cli/src/agent/session_supervisor/permissions.rs",
        "crates/magistr-cli/src/agent/session_supervisor/opening.rs",
        "crates/magistr-cli/src/agent/cockpit_session.rs",
        "crates/magistr-cli/src/orchestration/roadmap_request.rs",
        "crates/magistr-cli/src/application/mod.rs",
        "crates/magistr-cli/src/application/session_catalog.rs",
        "crates/magistr-cli/src/application/session_observation.rs",
        "crates/magistr-cli/src/application/session_observation.tests.rs",
        "crates/magistr-cli/src/application/retained_observation.rs",
        "crates/magistr-cli/src/application/retained_observation.tests.rs",
        "crates/magistr-application/src/sessions/tool_record.rs",
        "scripts/desktop-visual-gate.py",
        "crates/magistr-cli/src/application/session_registry.rs",
        "crates/magistr-cli/src/agent/session_supervisor/mod.rs",
        "crates/magistr-cli/src/agent/session_supervisor/owner.rs",
        "crates/magistr-cli/src/agent/session_supervisor/control.rs",
        "crates/magistr-cockpit-runtime/src/interactive_session.rs",
        "crates/magistr-cockpit-runtime/src/session.rs",
        "crates/magistr-application/src/sessions/session_handle.rs",
        "crates/magistr-application/src/sessions/conversation.rs",
        "crates/magistr-application/src/sessions/session_config.rs",
        "crates/magistr-cli/src/application/session_opening.rs",
        "crates/magistr-cli/src/application/saved_conversations.rs",
        "crates/magistr-cli/src/application/session_admissions.rs",
        "crates/magistr-cli/src/agent/runtime.rs",
        "crates/magistr-cli/src/agent/resolved_session_spec.rs",
        "crates/magistr-acp/src/transport/handshake.rs",
        "crates/magistr-cli/src/agent/probe.rs",
        "crates/magistr-cli/src/orchestration/interactive_session.rs",
        "crates/magistr-cli/src/permissions/session_permission_policy.rs",
        "crates/magistr-cli/src/agent/translate.rs",
        "crates/magistr-cli/src/agent/persistence/history.rs",
        "crates/magistr-cli/src/agent/persistence/storage.rs",
        "crates/magistr-cli/src/agent/persistence/session_record.rs",
        "crates/magistr-core/src/config/inputs/project_inputs.rs",
        "crates/magistr-application/src/sessions/prompt_input.rs",
        "crates/magistr-cli/src/application/session_attachments.rs",
        "crates/magistr-acp/src/transport/prompt_content.rs",
        "crates/magistr-acp/src/transport/command.rs",
        "crates/magistr-acp/src/transport/command_loop.rs",
        "crates/magistr-acp/src/transport/sender.rs",
        "crates/magistr-acp/src/transport/mod.rs",
        "crates/magistr-workspace/src/query/source_access.rs",
        "crates/magistr-workspace/src/query/source_observation.rs",
        "crates/magistr-workspace/src/service/workspace_service.rs",
    ))
    return {str(path.relative_to(ROOT)):hashlib.sha256(path.read_bytes()).hexdigest() for path in files if path.is_file()}

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("binary",type=Path)
    parser.add_argument("--evidence",type=Path,default=ROOT/".magistr/desktop-native")
    args=parser.parse_args()
    binary=args.binary.resolve()
    evidence=args.evidence.resolve();evidence.mkdir(parents=True,exist_ok=True)
    driver=evidence/"driver"
    try:run("swiftc","-parse-as-library",SOURCE/"tests/native_window.swift",SOURCE/"tests/rich_conversation.swift",SOURCE/"tests/attachment.swift","-o",driver,timeout=60)
    except subprocess.CalledProcessError as error:
        print(error.stderr,file=sys.stderr);raise
    def call(mode,*values,failure_prefix="failure"):
        command=list(map(str,(driver,mode,*values)))
        process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        stamp=str(time.time_ns());samples={}
        try:
            try:output,diagnostic=process.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                # Sample both sides before terminating the observer: AX calls may
                # be waiting on the app or traversing a native menu themselves.
                if values and mode not in ("environment","restore-front"):
                    for label,target in (("observer",process.pid),("app",values[0])):
                        path=evidence/(stamp+"-"+failure_prefix+"-"+label+"-sample.txt")
                        try:run("sample",target,1,1,"-file",path,timeout=5);samples[label]=path.name
                        except (subprocess.CalledProcessError,subprocess.TimeoutExpired) as error:samples[label]=str(error)
                process.kill();output,diagnostic=process.communicate()
                raise subprocess.TimeoutExpired(command,30,output=output,stderr=diagnostic)
            if process.returncode:
                raise subprocess.CalledProcessError(process.returncode,command,output=output,stderr=diagnostic)
            result=subprocess.CompletedProcess(command,process.returncode,output,diagnostic)
        except (subprocess.CalledProcessError,subprocess.TimeoutExpired) as error:
            print(error.stderr,file=sys.stderr)
            if values and mode not in ("environment","restore-front"):
                failure={"timestamp_ns":stamp,"mode":mode,"pid":str(values[0]),"error":str(error),"stderr":error.stderr,"samples":samples}
                for operation,name in (("ax",failure_prefix+"-ax.json"),("screenshot",failure_prefix+".png")):
                    try:
                        run(driver,operation,values[0],evidence/name,timeout=10)
                        shutil.copy2(evidence/name,evidence/(stamp+"-"+name))
                    except (subprocess.CalledProcessError,subprocess.TimeoutExpired) as capture:failure[operation]=str(capture)
                (evidence/(failure_prefix+"-driver.json")).write_text(json.dumps(failure,indent=2))
            if mode=="activate" and values:
                try:run("sample",values[0],1,1,"-file",evidence/"startup-failure-sample.txt",timeout=5)
                except (subprocess.CalledProcessError,subprocess.TimeoutExpired):pass
            raise
        return json.loads(result.stdout) if result.stdout.strip() else None
    environment=call("environment")
    assert environment["accessibility"] and environment["screen_capture"] and environment["post_events"],environment
    (evidence/"environment.json").write_text(json.dumps(environment,indent=2))
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
        peer.write_text(peer_script(executable))
        peer.chmod(0o755)
        peers=parent/"peers"
        refuse_load=parent/"refuse-load"
        api=start_api()
        rich=parent/"rich";rich.mkdir()
        attachments=parent/"attachments";attachments.mkdir()
        rich_link=f"http://127.0.0.1:{api.server_port}/rich-link/{time.time_ns()}"
        # WebKit requires the real macOS home for its GPU process. Isolate the
        # application's settings/history through their supported XDG/Magistr roots.
        env=dict(os.environ,PATH=str(tools)+os.pathsep+os.environ["PATH"],XDG_DATA_HOME=str(parent/"data"),XDG_CONFIG_HOME=str(parent/"config"),MAGISTR_HOME=str(parent/"state"),MAGISTR_WORKSPACE_NETWORK="0",JOURNEY_PEERS=str(peers),JOURNEY_REFUSE_LOAD=str(refuse_load),OPENAI_API_KEY="native-fixture",OPENAI_BASE_URL="http://127.0.0.1:"+str(api.server_port),RUST_LOG="info")
        env.update(JOURNEY_RICH=str(rich),JOURNEY_RICH_LINK=rich_link,JOURNEY_ATTACHMENTS=str(attachments))
        started=time.monotonic()
        log_path=evidence/"launch.log"
        log=log_path.open("w")
        process=subprocess.Popen([str(executable),"--provider","copilot","--trust","cautious","--reduce-motion"],env=env,cwd=first,stdout=log,stderr=log)
        pid=process.pid
        def rows():
            if process.poll() is not None:
                raise AssertionError("Desktop exited: "+log_path.read_text())
            try:return call("ax",pid)
            except subprocess.CalledProcessError as error:
                if process.poll() is not None:raise AssertionError(log_path.read_text()) from error
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
            try:call("screenshot",pid,evidence/"failure.png")
            except subprocess.CalledProcessError:pass
            try:run("sample",pid,1,1,"-file",evidence/"failure-sample.txt",timeout=5)
            except (subprocess.CalledProcessError,subprocess.TimeoutExpired):pass
            raise AssertionError(label+"; see failure-ax.json, failure.png and launch.log")
        def enabled(label, expected=True):
            return lambda tree:any(r.get("AXRole")=="AXButton" and (r.get("AXTitle")==label or r.get("AXDescription")==label) and r.get("enabled")==expected for r in tree)
        def exact_draft(value):
            return lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("AXValue")==value for r in tree)
        def draft(value):
            call("focus",pid,"Message the agent");call("key",pid,0,1048576);call("paste",pid,value)
        def send(value,method="key"):
            draft(value)
            if method=="key":call("key",pid,36,0)
            else:call(method,pid,"Send")
        def choose_theme(label,down):
            call("press",pid,"Appearance");call("press",pid,label.capitalize());time.sleep(.15)
            call("screenshot",pid,evidence/(label+".png"))
        def enter_folder(directory):
            call("press",pid,"Open project")
            call("focus",pid,"Project folder");call("key",pid,0,1048576);call("paste",pid,str(directory))
            call("press",pid,"Open")
        try:
            call("activate",pid)
            wait("clean launch opens a usable chooser",text_has("Choose a folder…"))
            results["launch_to_window_ms"]=(time.monotonic()-started)*1000
            assert not peers.exists(),"Showing the chooser must not launch a provider"
            assert not (parent/"data/magistr/sessions.db").exists(),"Showing the chooser must not create history"
            results["checks"]["chooser launches no provider and creates no history"]=True
            call("ax",pid,evidence/"chooser-accessibility.json")
            choose_theme("dark",1);call("screenshot",pid,evidence/"chooser-dark.png")
            choose_theme("light",2);call("screenshot",pid,evidence/"chooser-light.png")
            enter_folder(parent/"missing")
            wait("invalid directory stays in the chooser",text_has("Could not open folder"))
            call("press",pid,"Cancel")
            new=parent/"new";new.mkdir()
            (new/"CLAUDE.md").write_text("My own project instructions\\n")
            blocker=new/".claude/rules/conventions.md";blocker.mkdir(parents=True)
            enter_folder(new)
            wait("new project previews real setup with refusal",text_has("Cannot change"))
            assert not (new/".magistr").exists()
            call("screenshot",pid,evidence/"setup-review.png")
            call("press",pid,"Cancel")
            wait("declined setup returns to the chooser",text_has("Open the folder again"))
            assert not (new/".magistr").exists()
            enter_folder(new)
            wait("setup can be reviewed again",text_has("Review project setup"))
            call("press",pid,"Clear selection");call("press",pid,".magistr/project.json")
            call("press",pid,"Apply selected files")
            wait("partial setup stays visible without a session",text_has("Setup is incomplete"))
            assert not peers.exists(),"Partial setup must not admit a session"
            assert (new/".magistr/project.json").is_file()
            call("screenshot",pid,evidence/"setup-partial.png")
            blocker.rmdir()
            call("press",pid,"Review again")
            wait("partial setup gets a fresh plan",text_has("Apply selected files"))
            call("press",pid,"Apply selected files")
            wait("successful setup opens the agent in the same window",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("enabled") for r in tree))
            assert (new/"CLAUDE.md").read_text().startswith("My own project instructions")
            assert (new/".think-and-ship/project.json").is_file()
            results["checks"]["setup preserves user instructions and project identity"]=True
            call("press",pid,"Close session")
            wait("setup session closes back to chooser",text_has("Your session has closed"))
            call("press",pid,"Choose a folder…")
            wait("native folder selector opened",lambda tree:any(r.get("AXRole")=="AXSheet" for r in tree))
            call("folder",pid,str(first))
            wait("native folder selection opens the prepared project",text_has("First project"))
            wait("real window opened",lambda tree:text_has("First project")(tree) and any(r.get("AXRole")=="AXTextArea" and r.get("enabled") and r.get("AXTitle")=="Message the agent" for r in tree))
            call("press",pid,"Close session")
            wait("closing the last session returns to chooser with session actions unavailable",lambda tree:text_has("Choose a folder…")(tree) and any(r.get("AXRole")=="AXButton" and r.get("AXTitle")=="New session" and not r.get("enabled") for r in tree) and not any(r.get("AXRole")=="AXButton" and r.get("AXTitle")=="Close session" for r in tree))
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
            wait("available agent choices open without a provider",enabled("Start conversation"))
            call("screenshot",pid,evidence/"agent-choice.png")
            call("press",pid,"Choose model…")
            wait("initial model choices come from the selected agent",text_has("Initial model"))
            results["initial_model_picker"]=call("choose-option",pid,"New conversation","Initial model","Scripted (fast)")
            wait("native initial model picker commits before admission",lambda tree:any(r.get("AXRole")=="AXPopUpButton" and r.get("AXTitle")=="Initial model" and r.get("AXValue")=="Scripted (fast)" for r in tree))
            call("press",pid,"Start conversation")
            wait("second resident session",text_has("First project session 4"))
            draft("Second draft")
            call("press",pid,"First project session 3")
            wait("session switch restores draft",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("AXValue")=="Keep my unsent draft" for r in tree))
            call("press",pid,"Working copy");call("press",pid,"feature")
            wait("working copy opened",text_has(str(worktree)))
            call("key",pid,31,1048576)
            wait("keyboard project picker",text_has("Open a project"))
            call("focus",pid,"Project folder");call("key",pid,0,1048576);call("paste",pid,str(second))
            call("press",pid,"Open")
            wait("second project admitted",text_has(str(second)))
            wait("resident project appears before history settlement",lambda tree:any(r.get("AXRole")=="AXButton" and (r.get("AXTitle")=="Second project" or r.get("AXDescription")=="Second project") for r in tree))
            send("second project response")
            wait("second project streams independently",text_has("[scripted-fast] echo: second project response"))
            copied=call("copy-button",pid,"Copy conversation")["text"]
            assert "[scripted-fast] echo: second project response" in copied,copied
            results["checks"]["Copy conversation uses native clipboard"]=True
            # Supervision uses only the existing native controls and real peers.
            peer_count=len(list(peers.glob("*.json")))
            call("press",pid,"First project session 3")
            send("permission background")
            wait("background request started",text_has("Allow once"))
            draft("First supervision draft")
            call("press",pid,"Second project session 6")
            draft("Second supervision draft")
            wait("background attention updates without changing selection",lambda tree:text_has("1 request waiting")(tree) and exact_draft("Second supervision draft")(tree))
            attention="First project session 3: Bash"
            wait("waiting request names its exact session",text_has(attention))
            assert len(list(peers.glob("*.json")))==peer_count,"Supervision must not launch another provider"
            results["checks"]["catalog subscription launches no providers"]=True
            send("hang concurrent")
            wait("another session works while the first waits",lambda tree:enabled("Stop Second project session 6")(tree) and text_has("1 request waiting")(tree))
            draft("Second supervision draft")
            call("screenshot",pid,evidence/"supervision.png")
            call("press",pid,attention)
            wait("attention navigates to the owning copy and preserves its draft",lambda tree:text_has(str(first))(tree) and exact_draft("First supervision draft")(tree))
            wait("attention focuses the exact pending request",lambda tree:any(r.get("focused") and r.get("AXDescription")=="Request: Bash" for r in tree))
            call("press",pid,"Allow once")
            wait("accepted request leaves global attention",text_has("0 requests waiting"))
            call("press",pid,"Stop Second project session 6")
            wait("background stop settles only the addressed agent",lambda tree:enabled("Stop Second project session 6",False)(tree) and exact_draft("First supervision draft")(tree))
            send("fail supervision")
            wait("failure exposes reconnect for its owning agent",enabled("Reconnect First project session 3"))
            call("press",pid,"Second project session 6")
            wait("background failure preserves the selected draft",exact_draft("Second supervision draft"))
            call("press",pid,"Reconnect First project session 3")
            wait("background reconnect restores the owner without selecting it",lambda tree:enabled("Reconnect First project session 3",False)(tree) and exact_draft("Second supervision draft")(tree))
            wait("reconnect creates precisely one replacement peer",lambda tree:len(list(peers.glob("*.json")))==peer_count+1)
            call("press",pid,"First project session 3")
            send("after reconnect")
            wait("reconnected conversation accepts another turn",text_has("[scripted-fast] echo: after reconnect"))
            send("permission cancel background")
            wait("another request offered",text_has("Allow once"))
            call("press",pid,"Second project session 6")
            wait("second background request updates attention",text_has("1 request waiting"))
            call("press",pid,"Stop First project session 3")
            wait("cancelled request leaves attention and cannot be answered",lambda tree:text_has("0 requests waiting")(tree) and not text_has(attention)(tree) and exact_draft("Second supervision draft")(tree))
            call("press",pid,"Close First project session 4")
            wait("background close preserves another session and draft",lambda tree:not enabled("Close First project session 4")(tree) and exact_draft("Second supervision draft")(tree))
            send("supervised sessions remain independent")
            wait("selected session remains usable after background close",text_has("[scripted-fast] echo: supervised sessions remain independent"))
            call("press",pid,"Model");call("press",pid,"Scripted (slow)")
            wait("model choice applied by its owner",lambda tree:any(r.get("AXRole")=="AXPopUpButton" and r.get("AXValue")=="Scripted (slow)" for r in tree))
            send("chosen model")
            wait("next turn uses the chosen model",text_has("[scripted-slow] echo: chosen model"))
            send("fail model reconnect")
            wait("changed model session exposes reconnect",enabled("Reconnect Second project session 6"))
            call("press",pid,"Reconnect")
            wait("changed model reconnect settles without a new prompt",enabled("Reconnect",False))
            send("after model reconnect")
            wait("confirmed model survives provider reconnect",text_has("[scripted-slow] echo: after model reconnect"))
            send("hang controls")
            wait("active turn can accept followups",text_has("hanging until cancel"))
            send("follow-up to cancel")
            wait("accepted followup is queued and clears its exact draft",lambda tree:text_has("1 message queued")(tree) and exact_draft("")(tree))
            draft("Keep a newer draft")
            call("press",pid,"Stop")
            wait("cancel clears queued input while retaining the newer draft",lambda tree:text_has("0 messages queued")(tree) and text_has("1 queued message was not sent.")(tree) and exact_draft("Keep a newer draft")(tree))
            copied=call("copy-button",pid,"Copy conversation")["text"]
            assert "[scripted-slow] echo: follow-up to cancel" not in copied,copied
            call("focus",pid,"Conversation name");call("paste",pid,"Release review");call("press",pid,"Save name")
            wait("owner name reaches the rail and conversation",text_has("Release review"))
            call("screenshot",pid,evidence/"conversation-controls.png")
            call("press",pid,"Close session")
            wait("closing a named conversation settles its history",text_has("Conversation"))
            call("press",pid,"New session")
            wait("second provider is offered from configured availability",enabled("Start conversation"))
            results["provider_picker"]=call("choose-option",pid,"New conversation","Agent","OpenAI")
            wait("native provider picker commits OpenAI before admission",lambda tree:any(r.get("AXRole")=="AXPopUpButton" and r.get("AXTitle")=="Agent" and r.get("AXValue")=="OpenAI" for r in tree))
            call("press",pid,"Start conversation")
            wait("second provider opens independently",lambda tree:any(r.get("AXRole")=="AXTextArea" and r.get("enabled") for r in tree) and any(r.get("AXRole")=="AXStaticText" and r.get("AXValue")=="OpenAI" for r in tree))
            send("API history")
            wait("configured API provider replies through the real owner",text_has("Isolated API reply"))
            call("focus",pid,"Conversation name");call("paste",pid,"Summary only");call("press",pid,"Save name")
            wait("second provider has its own name",text_has("Summary only"))
            call("press",pid,"Close session")
            wait("other sessions remain usable after second provider closes",text_has("Conversation"))
            before_history=len(list(peers.glob("*.json")))
            call("press",pid,"Saved conversations")
            wait("existing saved names are listed",text_has("Summary only"))
            call("press",pid,"Summary only")
            wait("unsupported resume remains an honest saved summary",lambda tree:text_has("Isolated API reply")(tree) and enabled("Resume conversation",False)(tree) and text_has("cannot resume")(tree))
            assert len(list(peers.glob("*.json")))==before_history,"Reading summaries must not launch a provider"
            call("screenshot",pid,evidence/"resume-unavailable.png")
            call("press",pid,"Release review")
            wait("supported saved conversation offers an explicit resume",enabled("Resume conversation"))
            call("screenshot",pid,evidence/"saved-summary.png")
            refuse_load.write_text("refuse one provider load")
            call("press",pid,"Resume conversation")
            wait("refused resume stays recoverable beside the saved summary",text_has("This saved conversation is unavailable."))
            call("screenshot",pid,evidence/"resume-refused.png")
            refuse_load.unlink()
            call("press",pid,"Resume conversation")
            wait("saved conversation resumes its exact provider context",lambda tree:text_has("Release review")(tree) and text_has("resumed ")(tree) and any(r.get("AXRole")=="AXTextArea" and r.get("enabled") for r in tree))
            send("after saved reopen")
            wait("saved model and name remain with resumed conversation",lambda tree:text_has("[scripted-slow] echo: after saved reopen")(tree) and text_has("Release review")(tree))
            exercise_attachments(call=call,pid=pid,evidence=evidence,results=results,send=send,draft=draft,wait=wait,text_has=text_has,rows=rows,directory=second,control=attachments)
            exercise_rich_conversation(call=call,pid=pid,evidence=evidence,results=results,send=send,wait=wait,text_has=text_has,rows=rows,directory=second,control=rich,link=rich_link,api=api)
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
            assert len(list(peers.glob("*.json")))>=5
        finally:
            finish_fixture(process,log,log_path,call,front)
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
        assert len(history_rows)==7,history_rows
        results["history"]=[{"workspace":Path(w).name,"turns":t,"outcome":o} for w,t,o in history_rows]
        results["checks"]["session history written by application"]=True
        launch_new=parent/"launch-new";launch_new.mkdir()
        invalid_settings=parent/"invalid-settings"
        (invalid_settings/".magistr").mkdir(parents=True)
        (invalid_settings/".magistr/project.json").write_text("invalid")
        cases=[
            ("invalid current settings keep the window usable",invalid_settings,"copilot","project.json"),
            ("explicit invalid folder keeps the window usable",parent/"missing","copilot","Could not open folder"),
            ("explicit new folder previews setup before provider startup",launch_new,"copilot","Review project setup"),
            ("unavailable provider keeps the window usable",first,"copilot","not found on PATH"),
        ]
        provider_count=len(list(peers.glob("*.json")))
        unavailable_env=dict(env,PATH="/usr/bin:/bin")
        assert shutil.which("copilot",path=unavailable_env["PATH"]) is None
        for label,directory,provider,expected in cases:
            missing_provider=label=="unavailable provider keeps the window usable"
            log_path=evidence/(provider+"-"+directory.name+".log");log=log_path.open("w")
            process=subprocess.Popen([str(executable),"--dir",str(directory),"--provider",provider,"--reduce-motion"],env=unavailable_env if missing_provider else env,cwd=parent,stdout=log,stderr=log)
            pid=process.pid
            try:
                call("activate",pid)
                # Native window activation may animate independently of WebView motion.
                time.sleep(.5)
                wait(label,text_has(expected))
                assert len(list(peers.glob("*.json")))==provider_count,label
                if missing_provider:
                    call("screenshot",pid,evidence/"provider-unavailable.png")
                    assert all(not r.get("enabled") for r in rows() if r.get("AXRole")=="AXButton" and r.get("AXTitle")=="New session")
                call("press",pid,"Cancel")
                wait(label+" returns to chooser",text_has("Choose a folder…"))
            finally:
                finish_fixture(process,log,log_path,call,front)
        assert not (launch_new/".magistr").exists(),"Explicit new launch is a read-only preview"
        assert (invalid_settings/".magistr/project.json").read_text()=="invalid"
        with sqlite3.connect(database) as history:
            assert history.execute("SELECT COUNT(*) FROM sessions").fetchone()[0]==7,"Rejected opens must not create session history"
        results["checks"]["rejected launches preserve settings and create no session history"]=True
        api.shutdown();api.server_close()
        call("restore-front",front)
    results["source_hashes"]=source_hashes()
    (evidence/"receipt.json").write_text(json.dumps(results,ensure_ascii=False,indent=2)+"\n")
    print("PASS: native desktop journey, captured light/dark/narrow windows and accessibility tree")
if __name__=="__main__":main()
