"""Isolated peers for native session choice and resume-refusal journeys."""
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
import threading

def start_api():
    class API(BaseHTTPRequestHandler):
        def do_GET(self):
            self.server.visits.append(self.path)
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            token=self.path.rsplit("/",1)[-1]
            marker="Native link reached: "+token
            self.wfile.write(("<!doctype html><title>"+marker+"</title><h1>"+marker+"</h1>").encode())
        def do_POST(self):
            self.rfile.read(int(self.headers.get("Content-Length", 0)))
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            frame={"choices":[{"index":0,"delta":{"content":"Isolated API reply"},"finish_reason":"stop"}]}
            self.wfile.write(("data: "+json.dumps(frame)+"\n\ndata: [DONE]\n\n").encode())
        def log_message(self, *_): pass
    server=HTTPServer(("127.0.0.1",0), API)
    server.visits=[]
    threading.Thread(target=server.serve_forever,daemon=True).start()
    return server

def peer_script(executable):
    return """#!/usr/bin/python3
import os,json,sys
from pathlib import Path
p=Path(os.environ['JOURNEY_PEERS']);p.mkdir(exist_ok=True)
(p/(str(os.getpid())+'.json')).write_text(json.dumps({'pid':os.getpid(),'parent':os.getppid(),'workspace':os.getcwd()}))
if Path(os.environ['JOURNEY_REFUSE_LOAD']).exists():
    for line in sys.stdin:
        message=json.loads(line)
        method=message.get('method')
        if method=='initialize':
            reply={'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':1,'agentCapabilities':{'loadSession':True}}}
        elif method=='session/load':
            reply={'jsonrpc':'2.0','id':message['id'],'error':{'code':-32000,'message':'This saved conversation is unavailable.'}}
        elif method=='session/new':
            raise RuntimeError('A refused resume must never open a new conversation')
        else:
            continue
        print(json.dumps(reply),flush=True)
else:
    sys.path.insert(0,HELPERS)
    from rich_conversation_fixture import proxy
    proxy(EXECUTABLE)
""".replace("EXECUTABLE",repr(str(executable))).replace("HELPERS",repr(str(Path(__file__).resolve().parent)))
