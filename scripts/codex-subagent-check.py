#!/usr/bin/env python3
"""Codex parent/subagent check (retained ADR-018 acceptance row 23), native.

The retained incident: a Codex CLI parent had every workspace tool, its
`spawn_agent` subagent inherited the names, then every call answered "not
available to the model", because Codex starts one MCP server process per
subagent thread and the second server exited 75 on the exclusive lock.

This is that reproduction against Magistr's native owner: the REAL
`codex exec` under an isolated CODEX_HOME whose only MCP server is a shell
wrapper that logs every launch with pid and exit code before exec'ing
`magistr mcp-launch workbench-mcp <ws>` on a disposable workspace. The prompt
makes the parent call `read`, spawn one subagent that calls `read`, and answer
through `--output-schema`. Three witnesses decide: the parent's
`mcp_tool_call` events, the schema-shaped final answer, and the wrapper's
launch log (two launches, zero exit-75).

Opt-in (the optional `codex-subagent` Magistr gate): skips with exit 0 when
`codex` is not on PATH, no auth.json exists, or no magistr binary is found.
Only auth.json is copied (mode 0600) and deleted again before exit.

Usage: scripts/codex-subagent-check.py [--json] [--keep] [--timeout-sec N]
Exit codes: 0 pass or skipped; 1 fail or inconclusive; 2 harness failure.
"""

from __future__ import annotations

import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SERVER_NAME = "workbench-mcp"
FILE_UNDER_TEST = "src/a.ts"
FILE_CONTENT = "export const a = 1;\n"

SUBAGENT_INSTRUCTION = (
    f"Call the MCP tool `read` from the server `{SERVER_NAME}` with arguments "
    f'{{"file":"{FILE_UNDER_TEST}"}}. Reply with (1) the raw result exactly as returned, '
    "including any error text, and (2) the names of every tool available to you whose "
    "name contains 'workbench', one per line. Do not retry, do not use the shell."
)

PROMPT = "\n".join(
    [
        "You are a harness check. Do exactly the following steps and nothing else; never use the shell.",
        f"1. Call the MCP tool `read` from the server `{SERVER_NAME}` with arguments "
        f'{{"file":"{FILE_UNDER_TEST}"}}. Keep the raw result text.',
        f"2. Use spawn_agent to start ONE subagent with this instruction, verbatim: {json.dumps(SUBAGENT_INSTRUCTION)}",
        "3. Wait for that subagent to finish and read its reply.",
        "4. Answer with the JSON object the output schema requires: parent_read_ok is true only if your own read "
        f"returned the file content {json.dumps(FILE_CONTENT.strip())} with no error; subagent_read_ok is true only if the "
        "subagent's raw result contained that same content and no error; copy both raw results verbatim; "
        "subagent_workbench_tool_names is the list the subagent reported.",
    ]
)

OUTPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "parent_read_ok": {"type": "boolean"},
        "parent_read_raw": {"type": "string"},
        "subagent_read_ok": {"type": "boolean"},
        "subagent_read_raw": {"type": "string"},
        "subagent_workbench_tool_names": {"type": "array", "items": {"type": "string"}},
    },
    "required": [
        "parent_read_ok",
        "parent_read_raw",
        "subagent_read_ok",
        "subagent_read_raw",
        "subagent_workbench_tool_names",
    ],
    "additionalProperties": False,
}


def skip(reason: str, as_json: bool) -> None:
    if as_json:
        print(json.dumps({"verdict": "skipped", "reason": reason}))
    else:
        print(f"codex-subagent-check: SKIPPED — {reason}")
    sys.exit(0)


def find_magistr() -> Path | None:
    named = os.environ.get("MAGISTR_BIN")
    if named and Path(named).is_file():
        return Path(named)
    for profile in ("release", "debug"):
        candidate = REPO / "target" / profile / "magistr"
        if candidate.is_file():
            return candidate
    on_path = shutil.which("magistr")
    return Path(on_path) if on_path else None


def parse_events(jsonl: str) -> tuple[list[dict], list[str]]:
    parent_calls: list[dict] = []
    collab: list[str] = []
    for line in jsonl.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = event.get("item")
        if not isinstance(item, dict):
            continue
        if event.get("type") == "item.completed" and item.get("type") == "collab_tool_call":
            collab.append(str(item.get("tool")))
        if (
            event.get("type") == "item.completed"
            and item.get("type") == "mcp_tool_call"
            and item.get("server") == SERVER_NAME
        ):
            result = item.get("result") or {}
            content = result.get("content") or [{}]
            parent_calls.append(
                {
                    "tool": str(item.get("tool")),
                    "status": str(item.get("status")),
                    "isError": result.get("isError") is True or item.get("error") is not None,
                    "text": (content[0] or {}).get("text") or str(item.get("error") or ""),
                }
            )
    return parent_calls, collab


def parse_final_answer(text: str) -> dict | None:
    for candidate in (text.strip(), re.sub(r"^```(?:json)?\s*|\s*```$", "", text).strip()):
        try:
            parsed = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict) and isinstance(parsed.get("subagent_read_ok"), bool):
            return {
                "parent_read_ok": parsed.get("parent_read_ok") is True,
                "parent_read_raw": str(parsed.get("parent_read_raw", "")),
                "subagent_read_ok": parsed["subagent_read_ok"],
                "subagent_read_raw": str(parsed.get("subagent_read_raw", "")),
                "subagent_workbench_tool_names": [str(n) for n in parsed.get("subagent_workbench_tool_names") or []],
            }
    return None


def parse_launch_log(raw: str) -> dict:
    launches = sum(1 for line in raw.splitlines() if " LAUNCH " in line)
    exits = [int(m.group(1)) for m in re.finditer(r" EXIT pid=\d+ code=(\d+)", raw)]
    return {"launches": launches, "exits": exits, "exit75": exits.count(75), "raw": raw}


def main() -> int:
    argv = sys.argv[1:]
    as_json = "--json" in argv
    keep = "--keep" in argv
    timeout = 600
    if "--timeout-sec" in argv:
        timeout = int(argv[argv.index("--timeout-sec") + 1])

    codex = shutil.which("codex")
    if codex is None:
        skip("`codex` is not on PATH", as_json)
    auth_source = Path(os.environ.get("CODEX_HOME") or Path.home() / ".codex") / "auth.json"
    if not auth_source.exists():
        skip(f"no Codex auth to copy ({auth_source} does not exist)", as_json)
    magistr = find_magistr()
    if magistr is None:
        skip("no magistr binary (set MAGISTR_BIN or build target/release/magistr)", as_json)
    codex_version = subprocess.run([codex, "--version"], capture_output=True, text=True).stdout.strip()

    scratch = Path(tempfile.mkdtemp(prefix="magistr-codex-check-"))
    ws = scratch / "ws"
    codex_home = scratch / "codex-home"
    auth_copy = codex_home / "auth.json"
    wrapper = scratch / "workbench-mcp-wrapper.sh"
    launch_log = scratch / "launches.log"
    server_stderr = scratch / "server-stderr.log"
    schema_path = scratch / "output-schema.json"
    last_path = scratch / "last-message.txt"
    events_path = scratch / "events.jsonl"
    codex_stderr = scratch / "codex-stderr.log"

    (ws / "src").mkdir(parents=True)
    (ws / ".git").mkdir()
    (ws / FILE_UNDER_TEST).write_text(FILE_CONTENT)
    (ws / "package.json").write_text(json.dumps({"name": "harness", "version": "0.0.0"}))
    codex_home.mkdir(mode=0o700)
    wrapper.write_text(
        "\n".join(
            [
                "#!/bin/sh",
                f'echo "$(date +%H:%M:%S) LAUNCH pid=$$ ppid=$PPID" >> "{launch_log}"',
                f'"{magistr}" mcp-launch {SERVER_NAME} "{ws}" 2>>"{server_stderr}"',
                "code=$?",
                f'echo "$(date +%H:%M:%S) EXIT pid=$$ code=$code" >> "{launch_log}"',
                "exit $code",
                "",
            ]
        )
    )
    wrapper.chmod(0o755)
    launch_log.write_text("")
    server_stderr.write_text("")
    schema_path.write_text(json.dumps(OUTPUT_SCHEMA, indent=2))
    (codex_home / "config.toml").write_text(
        "\n".join(
            [
                f"[mcp_servers.{SERVER_NAME}]",
                f"command = {json.dumps(str(wrapper))}",
                "args = []",
                "startup_timeout_sec = 60",
                "tool_timeout_sec = 300",
                "required = true",
                "",
                f"[projects.{json.dumps(str(ws))}]",
                'trust_level = "trusted"',
                "",
            ]
        )
    )

    codex_exit: int | None = None
    events = ""
    harness_error: str | None = None
    try:
        shutil.copyfile(auth_source, auth_copy)
        auth_copy.chmod(stat.S_IRUSR | stat.S_IWUSR)
        env = dict(os.environ, CODEX_HOME=str(codex_home), WORKBENCH_NETWORK_FS="0")
        try:
            completed = subprocess.run(
                [
                    codex,
                    "exec",
                    "--json",
                    "--skip-git-repo-check",
                    "--dangerously-bypass-approvals-and-sandbox",
                    "-C",
                    str(ws),
                    "-o",
                    str(last_path),
                    "--output-schema",
                    str(schema_path),
                    PROMPT,
                ],
                cwd=ws,
                env=env,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=timeout,
            )
            codex_exit = completed.returncode
            events = completed.stdout
            events_path.write_text(events)
            codex_stderr.write_text(completed.stderr)
        except subprocess.TimeoutExpired as expired:
            harness_error = f"codex exec exceeded {timeout}s and was killed"
            events = expired.stdout.decode() if isinstance(expired.stdout, bytes) else (expired.stdout or "")
        except OSError as error:
            harness_error = f"codex exec could not run: {error}"
    finally:
        if auth_copy.exists():
            auth_copy.unlink()

    parent_calls, collab = parse_events(events)
    final_answer = parse_final_answer(last_path.read_text()) if last_path.exists() else None
    log = parse_launch_log(launch_log.read_text())

    reasons: list[str] = []
    verdict = "pass"
    if harness_error is not None or codex_exit != 0:
        verdict = "harness-error"
        reasons.append(harness_error or f"codex exec exited {codex_exit} (see {codex_stderr})")
    else:
        parent_read = next((c for c in parent_calls if c["tool"] == "read"), None)
        if parent_read is None or parent_read["isError"]:
            verdict = "fail"
            reasons.append(
                f"the parent's {SERVER_NAME}/read did not succeed (events: {parent_read['text'] if parent_read else 'no read call'})"
            )
        exercised = len(collab) > 0 or log["launches"] >= 2
        if not exercised:
            verdict = "inconclusive"
            reasons.append("no collab tool call and no second server launch — the subagent path was not exercised")
        elif final_answer is None:
            verdict = "inconclusive"
            reasons.append(f"the final answer did not follow the output schema (see {last_path})")
        elif not final_answer["subagent_read_ok"]:
            verdict = "fail"
            reasons.append(f"the subagent's {SERVER_NAME}/read failed: {final_answer['subagent_read_raw'][:200]}")
        if log["exit75"] > 0:
            verdict = "fail"
            reasons.append(f"{log['exit75']} server process(es) exited 75 — refused by the workspace lock (see {server_stderr})")
        if log["launches"] < 2:
            if verdict == "pass":
                verdict = "inconclusive"
            reasons.append(f"only {log['launches']} server launch(es) logged; a parent plus a subagent should show two")

    report = {
        "verdict": verdict,
        "reasons": reasons,
        "codexVersion": codex_version,
        "codexExit": codex_exit,
        "parentCalls": parent_calls,
        "collabTools": collab,
        "finalAnswer": final_answer,
        "launchLog": {k: v for k, v in log.items() if k != "raw"},
        "scratch": str(scratch),
    }
    if as_json:
        print(json.dumps(report, indent=2))
    else:
        print(f"codex-subagent-check: {verdict.upper()} (codex {codex_version}, exit {codex_exit})")
        calls = ", ".join(f"{c['tool']} {c['status']}{' ERROR' if c['isError'] else ''}" for c in parent_calls) or "none"
        print(f"  parent {SERVER_NAME} calls via --json events: {calls}")
        print(f"  collab tool calls (subagent evidence): {', '.join(collab) or 'none'}")
        if final_answer is None:
            print("  final answer: unparseable")
        else:
            print(
                f"  final answer: parent_read_ok={final_answer['parent_read_ok']} "
                f"subagent_read_ok={final_answer['subagent_read_ok']} "
                f"subagent_workbench_tool_names={json.dumps(final_answer['subagent_workbench_tool_names'])}"
            )
            print(f"  subagent raw: {json.dumps(final_answer['subagent_read_raw'][:300])}")
        print(
            f"  wrapper launch log: {log['launches']} launch(es), exit codes {log['exits']}, exit-75 count {log['exit75']}"
        )
        for reason in reasons:
            print(f"  - {reason}")
        print(f"  scratch: {scratch}")
    if not keep and verdict == "pass":
        shutil.rmtree(scratch, ignore_errors=True)
    return {"pass": 0, "fail": 1, "inconclusive": 1, "harness-error": 2}[verdict]


if __name__ == "__main__":
    sys.exit(main())
