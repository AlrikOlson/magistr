#!/usr/bin/env python3
"""Serve a real interactive client in a loopback PTY for screenshot capture.
No desktop activation, OS input, transcript rewriting or simulated replies.
The invocation JSON and private raw stream stay in the ignored capture directory.
"""
import argparse, base64, fcntl, json, os, pty, secrets, select, signal, struct
import subprocess, termios, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse, parse_qs

p=argparse.ArgumentParser()
p.add_argument("config",type=Path)
a=p.parse_args()
config=json.loads(a.config.read_text())
output=bytearray()
mutex=threading.Lock()
master=None
child=None
token=secrets.token_hex(24)
root=Path(config["repo"])
assets=root/"target/release-capture-tools/node_modules/@xterm/xterm"
raw=Path(config["scratch"])/"terminal.raw"
raw.parent.mkdir(parents=True,exist_ok=True)

def start():
    global master,child
    if child is not None:return
    pid,fd=pty.fork()
    if pid==0:
        fcntl.ioctl(0,termios.TIOCSWINSZ,struct.pack("HHHH",config["rows"],config["cols"],0,0))
        os.chdir(config["cwd"])
        env={k:v for k,v in os.environ.items() if not k.startswith(("MAGISTR_","CLAUDECODE","CLAUDE_CODE"))}
        env.update(config["env"],TERM="xterm-256color",COLORTERM="truecolor")
        os.execvpe(config["argv"][0],config["argv"],env)
    child=pid;master=fd
    def read():
        with raw.open("wb") as stream:
            while True:
                try:
                    data=os.read(fd,65536)
                    if not data:break
                except OSError:break
                stream.write(data);stream.flush()
                with mutex:output.extend(data)
        _,status=os.waitpid(pid,0)
        (raw.parent/"exit.json").write_text(json.dumps({"exit":os.waitstatus_to_exitcode(status)}))
    threading.Thread(target=read,daemon=True).start()

HTML="""<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="/xterm.css">
<style>html,body{margin:0;background:BACKGROUND}#terminal{padding:24px;display:inline-block}.xterm-viewport{scrollbar-width:none}.xterm-screen{outline:none}</style>
</head><body><main id="terminal" aria-label="Interactive agent session"></main>
<script src="/xterm.js"></script><script>
const config=CONFIG,token=TOKEN;
const term=new Terminal({cols:config.cols,rows:config.rows,fontFamily:'"SF Mono",Menlo,monospace',fontSize:16,lineHeight:1.22,letterSpacing:0,cursorBlink:false,scrollback:10000,theme:config.theme});
term.open(document.querySelector('#terminal'));window.term=term;
let offset=0;
async function send(data){await fetch('/input',{method:'POST',headers:{'Content-Type':'application/json','X-Capture-Token':token},body:JSON.stringify({data})});}
window.send=send;term.onData(send);
window.ready=(async()=>{await document.fonts.ready;await fetch('/start',{method:'POST',headers:{'X-Capture-Token':token}});
while(true){const r=await (await fetch('/output?offset='+offset)).json();offset=r.offset;
if(r.data){const bytes=Uint8Array.from(atob(r.data),c=>c.charCodeAt(0));await new Promise(resolve=>term.write(bytes,resolve));}
await new Promise(resolve=>setTimeout(resolve,80));}})();
window.screenText=()=>{const b=term.buffer.active;return Array.from({length:b.length},(_,i)=>b.getLine(i)?.translateToString(true)||'').join(String.fromCharCode(10));};
</script></body></html>"""
HTML=HTML.replace("BACKGROUND",config["theme"]["background"]).replace("CONFIG",json.dumps(config)).replace("TOKEN",json.dumps(token))

class Handler(BaseHTTPRequestHandler):
    def log_message(self,*_):pass
    def reply(self,data,kind):
        self.send_response(200);self.send_header("Content-Type",kind);self.send_header("Cache-Control","no-store");self.end_headers();self.wfile.write(data)
    def do_GET(self):
        route=urlparse(self.path)
        if route.path=="/":self.reply(HTML.encode(),"text/html");return
        if route.path=="/xterm.js":self.reply((assets/"lib/xterm.js").read_bytes(),"text/javascript");return
        if route.path=="/xterm.css":self.reply((assets/"css/xterm.css").read_bytes(),"text/css");return
        if route.path=="/output":
            offset=int(parse_qs(route.query).get("offset",["0"])[0])
            with mutex:data=bytes(output[offset:]);end=len(output)
            self.reply(json.dumps({"offset":end,"data":base64.b64encode(data).decode()}).encode(),"application/json");return
        self.send_error(404)
    def do_POST(self):
        if self.headers.get("X-Capture-Token")!=token:self.send_error(403);return
        if self.path=="/start":start()
        elif self.path=="/input" and master is not None:
            data=json.loads(self.rfile.read(int(self.headers["Content-Length"])))["data"]
            os.write(master,data.encode())
        else:self.send_error(404);return
        self.reply(b"{}","application/json")
server=ThreadingHTTPServer(("127.0.0.1",config["port"]),Handler)
print(json.dumps({"url":f"http://127.0.0.1:{server.server_port}","host":config["host"]}),flush=True)
try:server.serve_forever()
finally:
    if child:
        try:os.kill(child,signal.SIGTERM)
        except ProcessLookupError:pass
