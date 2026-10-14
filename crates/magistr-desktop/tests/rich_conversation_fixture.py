"""Deterministic ACP traffic for the real native rich-conversation journey."""
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

PROMPT = "Show the rich conversation review"
SELECTION_TEXT = "Native selection café 日本語 👩🏽‍💻."
LATE_TEXT = "\n\nSelected reply received an update."
TOOL_TITLE = "Review local build"
PROGRESS_TOOL_TITLE = "Track reading progress"
TOOL_ERROR = "error[E0425]: cannot find value `release_name` in this scope\n日本語: release_name を確認してください。"
STREAM_PREFIX = "Reading progress "
LINK_LABEL = "Open the local reading note"


def source(link):
    """Keep canonical bytes distinct from the semantic text selected on screen."""
    intro = f"""# Release reading note

{SELECTION_TEXT}

This reply contains **strong emphasis**, *quiet emphasis*, ~~retired wording~~ and `inline_code()`.
Read [{LINK_LABEL}]({link}) when you are ready.

> Keep the reading position while new work arrives.
>
> Ελληνικά · العربية · हिन्दी · 한국어 · עברית · é · café · 👩🏽‍💻

1. Review the changed module.
2. Check the error details.
   - Keep the original message.
   - Read the relevant line.

| Language | Example |
| --- | --- |
| Japanese | 日本語の文章 |
| Arabic | مرحباً بالعالم |
| Emoji | 👩🏽‍💻 🧭 |

```rust
fn reading_note() -> &'static str {{
    "café 日本語 👩🏽‍💻"
}}
// A deliberately long source line stays readable by scrolling within the code block: abcdefghijklmnopqrstuvwxyz_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz_0123456789
```

<script>window.location = "https://example.invalid/never";</script>

[Unavailable action](javascript:alert(1))
![Remote image stays text]({link.replace("/rich-link/", "/rich-image/")})

---

"""
    paragraphs = "\n\n".join(
        f"## Reading section {number:02d}\n\n"
        f"Section {number:02d} preserves the original source. "
        "The conversation remains selectable while another reply arrives. "
        "日本語を読み続けます。 Café, naïve, mañana, Ελληνικά and العربية stay intact. "
        "A reader can pause here, inspect the result and return to the latest reply."
        for number in range(1, 33)
    )
    return intro + paragraphs + "\n\nEnd of the original reading note."


def _write_status(directory, **values):
    temporary = directory / "progress.next"
    temporary.write_text(json.dumps(values))
    temporary.replace(directory / "progress.json")


def proxy(executable):
    """Intercept only the corpus turn; delegate existing checks to the real peer."""
    child = subprocess.Popen(
        [str(executable), "acp-agent", "--provider", "scripted"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1,
    )
    output_lock = threading.Lock()
    cancelled = threading.Event()
    rich_thread = None
    active_rich = threading.Event()

    def output(message):
        line = message if isinstance(message, str) else json.dumps(message, ensure_ascii=False)
        with output_lock:
            sys.stdout.write(line.rstrip("\n") + "\n")
            sys.stdout.flush()

    def forward_output():
        for line in child.stdout:
            output(line)

    relay = threading.Thread(target=forward_output, daemon=True)
    relay.start()

    def rich_turn(message):
        directory = Path(os.environ["JOURNEY_RICH"])
        directory.mkdir(exist_ok=True)
        session = message["params"]["sessionId"]
        count = 0
        started = time.monotonic()

        def update(value):
            output({"jsonrpc": "2.0", "method": "session/update",
                    "params": {"sessionId": session, "update": value}})

        def text(value):
            update({"sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": value}})

        update({"sessionUpdate": "tool_call", "toolCallId": "native-rich-build",
                "title": TOOL_TITLE, "kind": "execute", "status": "failed",
                "rawInput": {"command": "cargo test --package reading-note", "directory": "."},
                "rawOutput": {"exit_code": 1, "stderr": TOOL_ERROR},
                "content": [{"type": "content", "content": {"type": "text", "text": TOOL_ERROR}}],
                "locations": [{"path": str(Path.cwd() / "sample.rs"), "line": 2}]})
        text(source(os.environ["JOURNEY_RICH_LINK"]))
        _write_status(directory, stage="prepared", chunks=0, elapsed=0)
        deadline = time.monotonic() + 120
        while not (directory / "start").exists() and not cancelled.wait(.02):
            if time.monotonic() > deadline:
                cancelled.set()
        if not cancelled.is_set():
            # The selected article itself changes before a different tool/reply does.
            text(LATE_TEXT)
            update({"sessionUpdate": "tool_call_update", "toolCallId": "native-rich-build",
                    "status": "in_progress",
                    "content": [{"type": "content", "content": {
                        "type": "text", "text": TOOL_ERROR + "\nChecking the next result."}}]})
            update({"sessionUpdate": "tool_call", "toolCallId": "native-rich-progress",
                    "title": PROGRESS_TOOL_TITLE, "kind": "think", "status": "in_progress"})
            while not cancelled.is_set() and not (directory / "finish").exists():
                count += 1
                text(f"{STREAM_PREFIX}{count:04d}: new result café 日本語 👩🏽‍💻.\n\n")
                _write_status(directory, stage="streaming", chunks=count,
                              elapsed=time.monotonic() - started)
                if time.monotonic() > deadline:
                    cancelled.set()
                cancelled.wait(.08)
            update({"sessionUpdate": "tool_call_update", "toolCallId": "native-rich-build",
                    "status": "failed",
                    "content": [{"type": "content", "content": {
                        "type": "text", "text": TOOL_ERROR + "\nReview finished with an error."}}]})
            update({"sessionUpdate": "tool_call_update", "toolCallId": "native-rich-progress",
                    "status": "completed"})
        reason = "cancelled" if cancelled.is_set() else "end_turn"
        output({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": reason}})
        _write_status(directory, stage="finished", chunks=count,
                      elapsed=time.monotonic() - started, stop_reason=reason)
        active_rich.clear()

    try:
        for line in sys.stdin:
            message = json.loads(line)
            method = message.get("method")
            content = "".join(block.get("text", "") for block in message.get("params", {}).get("prompt", []))
            if method == "session/prompt" and content == PROMPT:
                if rich_thread is not None:
                    raise RuntimeError("The native rich corpus is used once per peer")
                active_rich.set()
                rich_thread = threading.Thread(target=rich_turn, args=(message,), daemon=True)
                rich_thread.start()
            elif method == "session/cancel" and active_rich.is_set():
                cancelled.set()
            else:
                child.stdin.write(line)
                child.stdin.flush()
    finally:
        cancelled.set()
        if rich_thread is not None:
            rich_thread.join(timeout=2)
        child.stdin.close()
        try:
            child.wait(timeout=8)
        except subprocess.TimeoutExpired:
            child.terminate()
            child.wait(timeout=4)
        relay.join(timeout=1)
