"""Attachment fixtures and exact ACP observations for the disposable native journey."""
import json
import os
from pathlib import Path
import struct
import time
import zlib

PREFIX = "Attachment check: "


def png_bytes():
    """A deterministic RGB PNG created entirely from fixture pixels."""
    width, height = 96, 64
    pixels = b"".join(
        b"\0" + b"".join(bytes((40 + x, 60 + y * 2, 160)) for x in range(width))
        for y in range(height)
    )

    def chunk(kind, payload):
        return (struct.pack(">I", len(payload)) + kind + payload +
                struct.pack(">I", zlib.crc32(kind + payload) & 0xffffffff))

    return (b"\x89PNG\r\n\x1a\n" +
            chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) +
            chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))


def fixture_files(directory):
    text = ("Native attachment reading note\r\n"
            "café · 日本語 · 👩🏽‍💻\r\n").encode() * 1200 + b"No final newline"
    contents = {
        "reading-note.txt": text,
        "preview.png": png_bytes(),
        "changed-note.txt": b"Selected original bytes\r\n",
        "missing-note.txt": b"Selected before deletion\r\n",
        "drop-note.txt": "Native file drop café\r\n日本語".encode(),
        "binary.bin": b"\0\xff\x80not an image",
        "too-large.txt": b"x" * (1024 * 1024 + 1),
    }
    for name, data in contents.items():
        (directory / name).write_bytes(data)
    return contents


def control_directory():
    value = os.environ.get("JOURNEY_ATTACHMENTS")
    return Path(value) if value else None


def record(directory, kind, value):
    directory.mkdir(exist_ok=True)
    path = directory / f"{kind}-{time.time_ns()}-{os.getpid()}.json"
    temporary = path.with_suffix(".next")
    temporary.write_text(json.dumps(value, ensure_ascii=False))
    temporary.replace(path)


def observe_response(line):
    """Negotiate the controlled peer's actual image mode on initialize."""
    directory = control_directory()
    if directory is None:
        return line
    message = json.loads(line)
    result = message.get("result", {})
    if isinstance(result, dict) and "protocolVersion" in result:
        supported = not (directory / "without-images").exists()
        capabilities = result.setdefault("agentCapabilities", {})
        capabilities.setdefault("promptCapabilities", {})["image"] = supported
        record(directory, "negotiation", {"images": supported, "pid": os.getpid()})
        return json.dumps(message, ensure_ascii=False)
    return line


def attachment_turn(message, output):
    """Capture the real wire blocks and answer only explicit attachment checks."""
    if message.get("method") != "session/prompt":
        return False
    directory = control_directory()
    if directory is None:
        return False
    record(directory, "prompt", message)
    prompt = message["params"].get("prompt", [])
    text = prompt[0].get("text", "") if prompt else ""
    if not text.startswith(PREFIX):
        return False
    output({"jsonrpc": "2.0", "method": "session/update", "params": {
        "sessionId": message["params"]["sessionId"], "update": {
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": "Native attachment accepted: " + text[len(PREFIX):]}}}})
    output({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
    return True
