"""Execute a check once, retaining its complete output and actual exit status."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("a command is required after --")
    log = args.log.resolve()
    if not log.is_relative_to((ROOT / "local").resolve()):
        parser.error("check logs must remain under ignored local storage")
    if log.exists():
        parser.error("use a new log path; existing execution evidence is immutable")
    log.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with log.open("wb") as stream:
        result = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT,
                                check=False)
    raw = log.read_bytes()
    receipt = dict(command=command, exit_code=result.returncode,
                   duration_ms=round((time.monotonic() - started) * 1000),
                   log=str(log), sha256=hashlib.sha256(raw).hexdigest())
    log.with_suffix(log.suffix + ".json").write_text(
        json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    text = raw.decode("utf-8", errors="replace")
    if result.returncode:
        print("\n".join(text.splitlines()[-100:]))
    else:
        for line in text.splitlines():
            if line.startswith(("test result:", "Ran ", "Finished", "    Finished")):
                print(line)
    print(json.dumps(receipt, sort_keys=True))
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
