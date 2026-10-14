"""A real ACP peer whose work turns are released by the native journey."""
import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time


def proxy(executable):
    directory = Path(os.environ["JOURNEY_WORK"])
    child = subprocess.Popen([str(executable), "acp-agent", "--provider", "scripted"],
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
    output_lock = threading.Lock()
    cancelled = threading.Event()
    active = threading.Event()
    worker = None

    def output(message):
        line = message if isinstance(message, str) else json.dumps(message)
        with output_lock:
            print(line.rstrip("\n"), flush=True)

    def relay():
        for line in child.stdout:
            output(line)

    reader = threading.Thread(target=relay, daemon=True)
    reader.start()

    def turn(message):
        with (directory / "counter").open("a+") as counter:
            fcntl.flock(counter, fcntl.LOCK_EX)
            counter.seek(0)
            number = int(counter.read() or "0") + 1
            counter.seek(0)
            counter.truncate()
            counter.write(str(number))
            counter.flush()
        (directory / f"started-{number}.json").write_text(json.dumps(message))
        output({"jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": message["params"]["sessionId"], "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": f"Working on item {number}."}}}})
        deadline = time.monotonic() + 180
        while not (directory / f"release-{number}").exists() and not cancelled.wait(.02):
            if time.monotonic() > deadline:
                raise RuntimeError("Native journey did not release the work turn")
        reason = "cancelled" if cancelled.is_set() else "end_turn"
        (directory / f"ended-{number}").write_text(reason)
        output({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": reason}})
        active.clear()

    try:
        for line in sys.stdin:
            message = json.loads(line)
            if message.get("method") == "session/prompt":
                assert not active.is_set(), "one current turn"
                cancelled.clear()
                active.set()
                worker = threading.Thread(target=turn, args=(message,), daemon=True)
                worker.start()
            elif message.get("method") == "session/cancel" and active.is_set():
                cancelled.set()
            else:
                child.stdin.write(line)
                child.stdin.flush()
    finally:
        cancelled.set()
        if worker:
            worker.join(timeout=3)
        child.stdin.close()
        child.wait(timeout=8)
        reader.join(timeout=2)
