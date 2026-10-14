#!/usr/bin/env python3
"""Repeatable macOS measurements; input and capture are restricted to fixture apps."""
import argparse
import ctypes as C
import json
import math
import os
from pathlib import Path
import plistlib
import resource
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
OBSERVER = Path("/private/tmp/magistr-foundation-observer")
APPS = Path("/private/tmp/magistr-foundation-apps")
LIB = C.CDLL("/usr/lib/libproc.dylib", use_errno=True)
SYSTEM = C.CDLL("/usr/lib/libSystem.B.dylib")
class TaskInfo(C.Structure):
    _fields_ = [(name,C.c_uint64) for name in
                ("virtual","rss","user","system","thread_user","thread_system")] + [
                (name,C.c_int32) for name in ("policy","faults","pageins","cow","sent","received",
                "mach","unix","switches","threads","running","priority")]
class Coalition(C.Structure):
    _fields_ = [("ids",C.c_uint64*2),("reserved",C.c_uint64*3)]
class Timebase(C.Structure):
    _fields_ = [("numer",C.c_uint32),("denom",C.c_uint32)]
TIMEBASE=Timebase()
SYSTEM.mach_timebase_info(C.byref(TIMEBASE))
TICK_NS=TIMEBASE.numer/TIMEBASE.denom
LIB.proc_pidinfo.argtypes=[C.c_int,C.c_int,C.c_uint64,C.c_void_p,C.c_int]
LIB.proc_pidinfo.restype=C.c_int

def info(pid,flavor,kind):
    value=kind()
    count=LIB.proc_pidinfo(pid,flavor,0,C.byref(value),C.sizeof(value))
    return value if count==C.sizeof(value) else None

def pids():
    length=LIB.proc_listallpids(None,0)
    values=(C.c_int*(length+128))()
    count=LIB.proc_listallpids(values,C.sizeof(values))
    return [pid for pid in values[:count] if pid>0]

def coalition(pid):
    value=info(pid,20,Coalition)
    if value is None:
        raise RuntimeError(f"Coalition query failed for fixture {pid}: errno {C.get_errno()}")
    return tuple(value.ids)

def sample(pid):
    group=coalition(pid)
    assert group[0] and group!=coalition(os.getpid()),"App must have its own launch coalition"
    result=[]
    for candidate in pids():
        membership=info(candidate,20,Coalition)
        if membership is None or membership.ids[0]!=group[0]:
            continue
        task=info(candidate,4,TaskInfo)
        if task is None:
            raise RuntimeError(f"Task accounting unavailable for coalition member {candidate}")
        name=C.create_string_buffer(512)
        LIB.proc_name(candidate,name,len(name))
        result.append({"pid":candidate,"name":name.value.decode("utf-8","replace"),
                       "rss_bytes":task.rss,"cpu_ns":(task.user+task.system)*TICK_NS,
                       "coalition":list(membership.ids)})
    assert any(row["pid"]==pid for row in result),"Leader omitted"
    return {"at":time.monotonic(),"rss_bytes":sum(row["rss_bytes"] for row in result),"processes":result}

def calibration():
    before=info(os.getpid(),4,TaskInfo)
    usage=resource.getrusage(resource.RUSAGE_SELF)
    prior=usage.ru_utime+usage.ru_stime
    end=time.monotonic()+0.25
    value=0
    while time.monotonic()<end: value+=1
    usage=resource.getrusage(resource.RUSAGE_SELF)
    expected=usage.ru_utime+usage.ru_stime-prior
    after=info(os.getpid(),4,TaskInfo)
    measured=(after.user+after.system-before.user-before.system)*TICK_NS/1e9
    ratio=measured/expected
    assert .95<ratio<1.05,("CPU units failed calibration",ratio)
    return {"cpu_seconds":measured,"getrusage_seconds":expected,"ratio":ratio,
            "mach_numer":TIMEBASE.numer,"mach_denom":TIMEBASE.denom}

def native(*args, json_result=False):
    done=subprocess.run([str(OBSERVER),*(str(arg) for arg in args)],
                        capture_output=True,text=True,timeout=40)
    if done.returncode:
        raise RuntimeError(f"Native operation {args[0]} exited {done.returncode}: {done.stderr.strip()}")
    return json.loads(done.stdout) if json_result else done.stdout

def write(path,value):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+"\n")

def read(path):
    return json.loads(Path(path).read_text())

def wait_file(path,predicate=lambda value:True,timeout=20):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        try:
            value=read(path)
            if predicate(value): return value
        except (FileNotFoundError,json.JSONDecodeError):
            pass
        time.sleep(.01)
    raise RuntimeError("No completed evidence: "+str(path))

def bundle():
    return APPS/"MagistrFoundation-tauri.app"

def prepare():
    subprocess.run(["swiftc","-swift-version","5","-parse-as-library","-module-cache-path",
                    "/private/tmp/magistr-foundation-swift-cache",str(ROOT/"native.swift"),
                    "-o",str(OBSERVER)],check=True)
    binary=ROOT/"target/release"/"magistr-foundation-tauri"
    if not binary.exists():
        raise RuntimeError("Build the release fixture before preparing the native app")
    contents=bundle()/"Contents"
    (contents/"MacOS").mkdir(parents=True,exist_ok=True)
    (contents/"Resources").mkdir(parents=True,exist_ok=True)
    shutil.copy2(binary,contents/"MacOS"/binary.name)
    shutil.copy2(ROOT/"assets/icon.png",contents/"Resources/icon.png")
    with (contents/"Info.plist").open("wb") as file:
        plistlib.dump({"CFBundleIdentifier":"dev.magistr.foundation.tauri",
                      "CFBundleName":"Magistr","CFBundleDisplayName":"Magistr",
                      "CFBundleExecutable":binary.name,"CFBundlePackageType":"APPL",
                      "CFBundleVersion":"1","CFBundleShortVersionString":"0.1.0",
                      "LSMinimumSystemVersion":"13.0","NSHighResolutionCapable":True,
                      "CFBundleIconFile":"icon.png",
                      "NSPrincipalClass":"NSApplication"},file)
    subprocess.run(["codesign","--force","--sign","-",str(bundle())],check=True,
                   stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)

def launch(output):
    output=Path(output);output.mkdir(parents=True,exist_ok=True)
    for name in ["ready","snapshot","metrics","journey"]:
        (output/(name+".json")).unlink(missing_ok=True)
    started=time.monotonic()
    result=native("launch",bundle(),output,json_result=True)
    pid=result["pid"]
    try:
        ready=wait_file(output/"ready.json")
        assert ready=={"items":1000,"messages":160,"diff":8000},ready
    except Exception:
        quit_fixture(pid)
        raise
    return pid,(time.monotonic()-started)*1000

def quit_fixture(pid):
    native("quit",pid)
    for _ in range(200):
        if info(pid,4,TaskInfo) is None: return
        time.sleep(.01)
    raise RuntimeError("Fixture did not quit")

def p95(values):
    assert values,"No samples"
    return sorted(values)[max(0,math.ceil(.95*len(values))-1)]

def cpu_between(first,last):
    before={row["pid"]:row["cpu_ns"] for row in first["processes"]}
    assert set(before) <= {row["pid"] for row in last["processes"]}, "A helper exited; endpoint CPU accounting is incomplete"
    elapsed=last["at"]-first["at"]
    used=sum(max(0,row["cpu_ns"]-before.get(row["pid"],0)) for row in last["processes"])
    return used/elapsed/1e9*100

def performance(output,launches=5):
    output=Path(output);output.mkdir(parents=True,exist_ok=True)
    pid=None
    timings=[]
    try:
        for number in range(launches):
            pid,elapsed=launch(output/("launch-"+str(number)))
            timings.append(elapsed)
            quit_fixture(pid);pid=None
        run=output/"active"
        pid,elapsed=launch(run)
        # Establish the same native window/AX state as the journey before sampling.
        native("resize",pid,1280,832)
        native("focus",pid,"Message")
        native("key",pid,18,1<<20)
        time.sleep(.4)
        first=sample(pid); samples=[first]
        for _ in range(20):
            time.sleep(.25);samples.append(sample(pid))
        idle=cpu_between(first,samples[-1])
        native("focus",pid,"Message")
        native("typing",pid,40,output/"typing.json")
        native("latency",pid,40,output/"latency.json")
        native("motion",pid,output/"motion.json")
        native("resize",pid,1280,832)
        stream_before=sample(pid)
        native("press",pid,"Start work")
        while not (run/"metrics.json").exists():
            samples.append(sample(pid))
            if len(samples)>200: raise RuntimeError("Stream did not finish")
            time.sleep(.1)
        metrics=wait_file(run/"metrics.json")
        assert metrics["received"]==10000 and metrics["sequences"]==[1250]*8,metrics
        assert metrics["acknowledged_sequences"]==[1250]*8,metrics
        assert metrics["dropped"]==0 and not metrics["errors"],metrics
        samples.append(sample(pid))
        native("ax",pid,output/"accessibility.json")
        latencies=read(output/"latency.json")["samples"]
        typing=read(output/"typing.json")["samples"]
        motion=read(output/"motion.json")
        assert len(motion["sizes"])==60 and all(row["error"]==0 and len(row["actual"])==2 and all(abs(a-b)<=1 for a,b in zip(row["actual"],row["requested"])) for row in motion["sizes"]),motion["sizes"]
        frame_results={}
        for label,begin,end in [("scroll",motion["scroll_start_s"],motion["resize_start_s"]),
                                ("resize",motion["resize_start_s"],motion["end_s"])]:
            frames=[row["display_s"] for row in motion["frames"] if row["changed"] and begin<=row["display_s"]<=end]
            intervals=[(b-a)*1000 for a,b in zip(frames,frames[1:])]
            frame_results[label]={"samples":len(intervals),"visible_interval_p95_ms":p95(intervals)}
        results={"backend":"tauri","launch_ms":timings,"launch_p95_ms":p95(timings),
                 "idle_cpu_percent_one_core":idle,"idle_rss_mib":first["rss_bytes"]/2**20,
                 "peak_rss_mib":max(row["rss_bytes"] for row in samples)/2**20,
                 "input_visible_p95_ms":p95([row["display_ms"] for row in typing]),
                 "stream_cpu_percent_one_core":cpu_between(stream_before,samples[-1]),
                 "capture_callback_p95_ms":p95([row["callback_ms"] for row in latencies]),
                 "workspace_switch_p95_ms":p95([row["display_ms"] for row in latencies]),
                 "motion":frame_results,"stream":metrics,
                 "events_per_second":10000/(metrics["elapsed_ms"]/1000),
                 "render_ack_p95_ms":p95(metrics["delays_ms"]),
                 "process_samples":samples,"stream_cpu_begin":stream_before,
                 "stream_cpu_end":samples[-1],"cpu_calibration":calibration(),
                 "load_average":os.getloadavg()}
        write(output/"results.json",results)
        print(json.dumps({key:value for key,value in results.items() if key not in ("process_samples","stream")}),flush=True)
        return results
    finally:
        if pid: quit_fixture(pid)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("mode",choices=["prepare","launch","performance","sample","calibration"])
    parser.add_argument("--output",type=Path,default=ROOT/"results")
    parser.add_argument("--pid",type=int)
    parser.add_argument("--launches",type=int,default=5)
    args=parser.parse_args()
    if args.mode=="prepare":prepare()
    elif args.mode=="launch":
        pid,elapsed=launch(args.output)
        print(json.dumps({"pid":pid,"launch_ms":elapsed}))
    elif args.mode=="sample":print(json.dumps(sample(args.pid)))
    elif args.mode=="calibration":print(json.dumps(calibration()))
    elif args.mode=="performance":
        environment=native("environment",json_result=True)
        write(args.output/"environment.json",environment)
        assert environment["accessibility"] and environment["screen_capture"],environment
        assert not environment["voiceover_running"],"Turn VoiceOver off for performance sampling"
        try:performance(args.output,args.launches)
        finally:native("restore-front",environment["front_pid"])

if __name__=="__main__":main()
