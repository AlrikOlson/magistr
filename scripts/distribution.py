#!/usr/bin/env python3
"""Prepare publishable Cargo/npm artifacts and verify installations. Never publishes."""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import signal
import threading
import http.server
import base64
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
from distribution_licenses import collect as collect_notices
from version import check as check_version, npm_dist_tag

ROOT=Path(__file__).resolve().parents[1]
def run(args, cwd=ROOT, **kwargs):
    return subprocess.run([str(a) for a in args],cwd=cwd,check=True,**kwargs)
def output(args,cwd=ROOT):
    return run(args,cwd,text=True,stdout=subprocess.PIPE).stdout
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def metadata():
    return json.loads(output(["cargo","metadata","--locked","--filter-platform","aarch64-apple-darwin","--format-version","1"]))
def package_order(meta):
    members={p["name"]:p for p in meta["packages"] if p["id"] in meta["workspace_members"]}
    order=[];done=set()
    def visit(name):
        if name in done:return
        done.add(name)
        for d in sorted(members[name]["dependencies"],key=lambda d:d["name"]):
            if d["kind"] is None and d["name"] in members:visit(d["name"])
        order.append(members[name])
    visit("magistr-cli")
    return order
def write_json(path,data):
    path.write_text(json.dumps(data,indent=2)+"\n")

def prepare(args):
    check_version(ROOT)
    if platform.system()!="Darwin" or platform.machine()!="arm64":
        raise SystemExit("Prepare the first-release native package on Apple Silicon macOS.")
    dest=args.out.resolve();dest.mkdir(parents=True,exist_ok=True)
    for receipt in ("cargo-install-verification.json","npm-install-verification.json"):
        (dest/receipt).unlink(missing_ok=True)
    meta=metadata();order=package_order(meta);version=order[-1]["version"]
    manifest=json.loads((ROOT/"npm/magistr/package.json").read_text())
    if manifest["version"]!=version:raise SystemExit("npm/Cargo versions differ")
    if args.binary:
        raise SystemExit("--binary is only for the installed-executable smoke check")
    run(["cargo","build","--locked","--release","-p","magistr-cli","--bin","magistr"])
    binary=Path(meta["target_directory"])/"release/magistr"
    if output([binary,"--version"]).strip()!=f"magistr {version}":
        raise SystemExit("Binary version does not match package")
    if "arm64" not in output(["/usr/bin/file",binary]):
        raise SystemExit("Package requires a macOS arm64 executable")
    linked=output(["/usr/bin/otool","-L",binary]).splitlines()[1:]
    assert all(line.strip().startswith(("/usr/lib/", "/System/Library/")) for line in linked), linked
    cargo=["cargo","package","--locked","--allow-dirty","--no-verify"]
    for p in order:cargo.extend(["-p",p["name"]])
    run(cargo)
    archives=[]
    for p in order:
        source=Path(meta["target_directory"])/"package"/f'{p["name"]}-{version}.crate'
        target=dest/source.name;shutil.copy2(source,target)
        with tarfile.open(target) as t:
            files=[m.name for m in t if m.isfile()]
        junk=[name for name in files if
              any(part in {".DS_Store", "__pycache__", "node_modules", "target"}
                  for part in Path(name).parts) or name.endswith((".pyc", ".pyo"))]
        if junk:raise SystemExit("Local build or desktop metadata in Cargo package: "+", ".join(junk))
        archives.append({"package":p["name"],"version":version,"file":target.name,
                         "sha256":sha(target),"bytes":target.stat().st_size,"files":files})
    with tempfile.TemporaryDirectory(prefix="magistr npm stage ") as temp:
        stage=Path(temp)/"package";stage.mkdir();(stage/"bin").mkdir()
        shutil.copy2(binary,stage/"bin/magistr");(stage/"bin/magistr").chmod(0o755)
        shutil.copy2(ROOT/"LICENSE",stage/"LICENSE")
        shutil.copy2(ROOT/"npm/magistr/README.md",stage/"README.md")
        notices=stage/"THIRD_PARTY_NOTICES";notices.mkdir()
        dependencies=collect_notices(meta,notices)
        shutil.copy2(ROOT/"crates/magistr-cli/assets/qjs/LICENSE",notices/"QuickJS-ng-0.16.2-LICENSE")
        shutil.copy2(ROOT/"crates/magistr-cli/assets/qjs/PROVENANCE.md",notices/"QuickJS-ng-PROVENANCE.md")
        shutil.copy2(ROOT/"crates/magistr-workspace/PROVENANCE.md",notices/"Workspace-PROVENANCE.md")
        inventory={"version":version,"target":"aarch64-apple-darwin","binarySha256":sha(binary),
                   "dependencies":dependencies}
        write_json(stage/"inventory.json",inventory)
        write_json(stage/"package.json",manifest)
        packed=json.loads(output(["npm","pack","--json","--ignore-scripts","--pack-destination",dest],stage))[0]
        npm_file=dest/packed["filename"]
        write_json(dest/"npm-inventory.json",packed)
        write_json(dest/"binary-inventory.json",inventory)
    write_json(dest/"cargo-inventory.json",archives)
    (dest/"publication-order.txt").write_text("\n".join(p["name"] for p in order)+"\n")
    artifacts=[dest/a["file"] for a in archives]+[npm_file]
    (dest/"SHA256SUMS").write_text("".join(f"{sha(f)}  {f.name}\n" for f in artifacts))
    print(json.dumps({"version":version,"cargoPackages":len(archives),"npm":str(npm_file),
                      "published":False}))

def verify_source(args):
    """Refuse publication if this checkout does not reproduce the verified Cargo archives."""
    check_version(ROOT)
    artifacts=args.out.resolve()
    records=json.loads((artifacts/"cargo-inventory.json").read_text())
    command=["cargo","package","--locked","--allow-dirty","--no-verify"]
    for record in records:command.extend(["-p",record["package"]])
    run(command)
    target=Path(metadata()["target_directory"])/"package"
    for record in records:
        if sha(target/record["file"])!=record["sha256"]:
            raise SystemExit("Checkout differs from verified Cargo package: "+record["package"])

def verify_cargo(args):
    """Use real package manifests/lockfiles and vendored registry dependencies, no path patches."""
    artifacts=args.out.resolve()
    with tempfile.TemporaryDirectory(prefix="magistr clean cargo ") as temp:
        scratch=Path(temp);vendor=scratch/"vendor"
        # Vendor only registry sources. Insert the unpublished, normalized Cargo
        # archives as checksum-verified registry packages in the same directory.
        run(["cargo","vendor","--locked",vendor],stdout=subprocess.DEVNULL)
        records=json.loads((artifacts/"cargo-inventory.json").read_text())
        for record in records:
            archive=artifacts/record["file"]
            if sha(archive)!=record["sha256"]:raise SystemExit("Cargo checksum mismatch")
            with tarfile.open(archive) as t:t.extractall(vendor,filter="data")
            package=vendor/f'{record["package"]}-{record["version"]}'
            files={str(f.relative_to(package)):sha(f) for f in package.rglob("*") if f.is_file()}
            write_json(package/".cargo-checksum.json",{"files":files,"package":record["sha256"]})
        config=scratch/"cargo-config.toml"
        config.write_text('[source.crates-io]\nreplace-with = "prepared"\n[source.prepared]\ndirectory = '+json.dumps(str(vendor))+'\n')
        cli=next(x for x in records if x["package"]=="magistr-cli")
        source=scratch/"install-source"
        with tarfile.open(artifacts/cli["file"]) as t:t.extractall(source,filter="data")
        root=scratch/"installed"
        command=["cargo","install","--locked","--offline","--path",source/f'magistr-cli-{cli["version"]}',
                 "--root",root,"--config",config]
        if args.debug:command.append("--debug")
        env=os.environ.copy();env["CARGO_TARGET_DIR"]=str(args.build_dir.resolve() if args.build_dir else scratch/"build")
        run(command,cwd=scratch,env=env)
        smoke(root/"bin/magistr",scratch/"project with spaces")
        write_json(artifacts/"cargo-install-verification.json",{
            "passed":True,"platform":platform.platform(),"version":cli["version"],
            "capturedAt":datetime.now(timezone.utc).isoformat(),
            "artifacts":{r["file"]:r["sha256"] for r in records},
            "registry":"checksum-verified vendored registry, including unpublished packages",
            "profile":"debug" if args.debug else "release","checkoutRequired":False})

def smoke(binary,root):
    root.mkdir();(root/"README.md").write_text("Package smoke fixture.\n")
    env={k:v for k,v in os.environ.items() if not k.startswith(("MAGISTR_WORKFLOW_","MAGISTR_","MAGISTR_WORKSPACE_"))}
    env.update(MAGISTR_WORKFLOW_DATA_DIR=str(root/"workflow-state"),XDG_CONFIG_HOME=str(root/"user-config"),XDG_DATA_HOME=str(root/"user-data"),MAGISTR_WORKSPACE_NETWORK_FS="0")
    run([binary,"init",root,"--agent","portable","--yes"],cwd=root,env=env,stdout=subprocess.PIPE)
    run([binary,"doctor",root],cwd=root,env=env,stdout=subprocess.PIPE)
    # Launch the installed executable as the server and worker; no repository source.
    spec=importlib.util.spec_from_file_location("unified_check",ROOT/"scripts/unified-mcp-check.py")
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    connection=module.Connection(root,root/"workflow-state",command=[str(binary),"mcp","--dir",str(root)])
    try:
        result=connection.execute("magistr.call('workspace_status', {}); return magistr.call('read', {file:'README.md'});",maxOperations=2)
        assert result["ok"],result
        assert len(result["receipts"])==2,result
    finally: connection.close()


def verify_npm(args):
    dest=args.out.resolve();version=json.loads((ROOT/"npm/magistr/package.json").read_text())["version"]
    tarball=dest/f"magistr-{version}.tgz"
    with tempfile.TemporaryDirectory(prefix="magistr clean npm ") as temp:
        scratch=Path(temp);prefix=scratch/"global";prefix.mkdir()
        # PATH deliberately has npm/Node and OS tools, but no cargo, rustc, Bun or checkout.
        commands=scratch/"commands";commands.mkdir()
        for name in ["node","npm","npx"]:
            source=Path(shutil.which(name)).resolve()
            (commands/name).symlink_to(source)
        env={k:v for k,v in os.environ.items() if not k.startswith(("MAGISTR_WORKFLOW_","MAGISTR_","MAGISTR_WORKSPACE_"))}
        env.update(MAGISTR_WORKFLOW_DATA_DIR=str(scratch/"workflow-state"),XDG_CONFIG_HOME=str(scratch/"user-config"),XDG_DATA_HOME=str(scratch/"user-data"))
        env["PATH"]=f"{commands}:/usr/bin:/bin:/usr/sbin:/sbin"
        env["npm_config_cache"]=str(scratch/"cache")
        run(["npm","install","--global","--prefix",prefix,"--ignore-scripts",tarball],cwd=scratch,env=env,stdout=subprocess.PIPE)
        binary=prefix/"bin/magistr"
        assert output([binary,"--version"],scratch).strip()==f"magistr {version}"
        # smoke uses this exact installed native executable, including child worker startup.
        # npx is tested with the packed artifact, not the source tree or an unpublished registry.
        (scratch/"npx project").mkdir()
        run(["npx","--yes","--package",tarball,"magistr","init",scratch/"npx project","--agent","portable","--yes"],
            cwd=scratch,env=env,stdout=subprocess.PIPE)
        project=scratch/"npx project"
        config=json.loads((project/".mcp.json").read_text())["mcpServers"]["magistr"]
        assert config["command"]=="npx" and f"magistr@{version}" in config["args"],config
        # Serve only the packed artifact on loopback. A fresh npm cache proves
        # the generated pinned registration resolves independently of npx init's cache.
        blob=tarball.read_bytes()
        class Registry(http.server.BaseHTTPRequestHandler):
            def log_message(self,*args): pass
            def do_GET(self):
                if self.path.startswith("/magistr/-/"):
                    payload=blob;kind="application/octet-stream"
                elif self.path=="/magistr":
                    package=dict(json.loads((ROOT/"npm/magistr/package.json").read_text()))
                    package["dist"]={"tarball":f"http://127.0.0.1:{self.server.server_port}/magistr/-/magistr-{version}.tgz",
                        "integrity":"sha512-"+base64.b64encode(hashlib.sha512(blob).digest()).decode()}
                    # Mirror the real registry: a pre-release sits on `beta`, not `latest`.
                    payload=json.dumps({"name":"magistr","dist-tags":{npm_dist_tag(version):version},
                        "versions":{version:package}}).encode();kind="application/json"
                else:
                    self.send_error(404);return
                self.send_response(200);self.send_header("Content-Type",kind)
                self.send_header("Content-Length",str(len(payload)));self.end_headers();self.wfile.write(payload)
        registry=http.server.ThreadingHTTPServer(("127.0.0.1",0),Registry)
        thread=threading.Thread(target=registry.serve_forever,daemon=True);thread.start()
        registry_env=dict(env,npm_config_registry=f"http://127.0.0.1:{registry.server_port}",
                          npm_config_cache=str(scratch/"fresh-cache"))
        messages=[{"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"pinned-npm","version":"1"}}},
            {"jsonrpc":"2.0","method":"notifications/initialized"},
            {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"context","arguments":{}}}]
        wire="".join(json.dumps(message)+"\n" for message in messages)
        try:
            # Send requests while the stream remains open so EOF cannot race initialization.
            proc=subprocess.Popen([config["command"],*config["args"]],cwd=project,env=registry_env,
                stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            proc.stdin.write(wire);proc.stdin.flush()
            import queue
            replies=queue.Queue()
            threading.Thread(target=lambda:[replies.put(line) for line in proc.stdout],daemon=True).start()
            while True:
                reply=json.loads(replies.get(timeout=45))
                if reply.get("id")==2:
                    assert "error" not in reply and not reply["result"].get("isError"),reply
                    break
            proc.stdin.close();assert proc.wait(timeout=20)==0
            proc.stdout.close();proc.stderr.close()
        finally:
            if "proc" in locals() and proc.poll() is None:proc.kill();proc.wait()
            registry.shutdown();registry.server_close();thread.join()
        # A process-local environment is used by the smoke without changing this machine.
        child_env=env.copy()
        run([os.sys.executable,__file__,"smoke","--binary",binary,"--out",scratch/"global project"],
            cwd=scratch,env=child_env)
        bad=subprocess.run([binary,"--no-such-option"],cwd=scratch,env=env,capture_output=True)
        assert bad.returncode==2,"native usage exit code changed"
        # npm links directly to the executable: SIGTERM must reach that process.
        proc=subprocess.Popen([binary,"mcp","--dir",str(project)],cwd=project,env=env,
            stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        proc.send_signal(signal.SIGTERM)
        assert proc.wait(timeout=15)==-signal.SIGTERM,"npm altered native signal delivery"
        proc.stdin.close();proc.stdout.close();proc.stderr.close()
        run([binary,"remove",project,"--yes"],cwd=project,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        remaining=json.loads((project/".mcp.json").read_text()) if (project/".mcp.json").exists() else {}
        assert "magistr" not in remaining.get("mcpServers",{}),remaining
        with tarfile.open(tarball) as t:
            names=t.getnames()
            assert "package/bin/magistr" in names
            assert not any(n.endswith(".rs") or "/node_modules/" in n for n in names)
        write_json(dest/"npm-install-verification.json",{
            "passed":True,"version":version,"platform":platform.platform(),"rustOrBunOnPath":False,
            "capturedAt":datetime.now(timezone.utc).isoformat(),"artifactSha256":sha(tarball),
            "globalInstall":True,"npxInit":True,"pinnedConfigFreshCache":True,"nativeMcpAndWorker":True,"usageExitCode":2,"sigterm":True,"removal":True})

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action",choices=["prepare","verify-source","verify-cargo","verify-npm","smoke"])
    parser.add_argument("--out",type=Path,required=True)
    parser.add_argument("--binary",type=Path)
    parser.add_argument("--debug",action="store_true")
    parser.add_argument("--build-dir",type=Path,help="Optional build cache; package sources still come only from extracted artifacts")
    args=parser.parse_args()
    if args.action=="prepare":prepare(args)
    elif args.action=="verify-source":verify_source(args)
    elif args.action=="verify-cargo":verify_cargo(args)
    elif args.action=="verify-npm":verify_npm(args)
    else:smoke(args.binary.resolve(),args.out.resolve())
if __name__=="__main__":main()
