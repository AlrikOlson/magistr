"""Summarize paired Harbor trials without turning errors into task failures."""

import argparse
import json
from collections import Counter
from datetime import datetime
from pathlib import Path


def elapsed(timing):
    if not timing or not timing.get("started_at") or not timing.get("finished_at"):
        return None
    return round(
        (
            datetime.fromisoformat(timing["finished_at"])
            - datetime.fromisoformat(timing["started_at"])
        ).total_seconds(),
        3,
    )


def classify(result):
    error = result.get("exception_info")
    rewards = (result.get("verifier_result") or {}).get("rewards")
    if error:
        kind = error.get("exception_type", "")
        return "timeout" if "Timeout" in kind else "run_error"
    if not rewards or "reward" not in rewards:
        return "not_scored"
    if rewards["reward"] == 1:
        return "passed"
    if rewards["reward"] == 0:
        return "failed"
    return "nonbinary_reward"


def trial_row(path, arm):
    result = json.loads(path.read_text())
    items = {}
    output = path.parent / "agent" / "codex.txt"
    if output.exists():
        for line in output.read_text().splitlines():
            try:
                event = json.loads(line)
            except ValueError:
                continue
            item = event.get("item") or {}
            if item.get("id"):
                items[item["id"]] = item
    calls = Counter()
    errors = Counter()
    error_codes = Counter()
    incomplete = Counter()
    for item in items.values():
        if item.get("type") == "mcp_tool_call":
            name = item.get("server", "") + "/" + item.get("tool", "")
            calls[name] += 1
            envelope = (item.get("result") or {}).get("structured_content") or {}
            if (
                item.get("error")
                or item.get("status") == "failed"
                or envelope.get("ok") is False
            ):
                errors[name] += 1
                code = (envelope.get("error") or {}).get(
                    "code", "transport_or_tool_error"
                )
                error_codes[code] += 1
            if item.get("status") == "in_progress":
                incomplete[name] += 1
    return {
        "arm": arm,
        "task": result["task_name"],
        "trial": result["trial_name"],
        "status": classify(result),
        "rewards": (result.get("verifier_result") or {}).get("rewards"),
        "exception_type": (result.get("exception_info") or {}).get("exception_type"),
        "agent_seconds": elapsed(result.get("agent_execution")),
        "setup_seconds": elapsed(result.get("agent_setup")),
        "usage": result.get("agent_result"),
        "mcp_calls": dict(sorted(calls.items())),
        "mcp_errors": dict(sorted(errors.items())),
        "mcp_error_codes": dict(sorted(error_codes.items())),
        "mcp_incomplete": dict(sorted(incomplete.items())),
        "command_calls": sum(
            i.get("type") == "command_execution" for i in items.values()
        ),
        "result_path": str(path),
    }


def summarize(root):
    rows = []
    manifest = json.loads((root / "manifest.json").read_text())
    for job in manifest["scored_jobs"]:
        paths = list(root.glob(job["result_glob"]))
        if len(paths) == 1:
            row = trial_row(paths[0], job["arm"])
            if row["task"] != job["task"]:
                raise ValueError("Result task does not match the frozen schedule")
        else:
            row = {
                "arm": job["arm"],
                "task": job["task"],
                "status": "missing_result" if not paths else "duplicate_results",
                "mcp_calls": {},
                "mcp_errors": {},
                "usage": None,
                "result_paths": [str(p) for p in paths],
            }
        row["position"] = job["position"]
        rows.append(row)
    aggregate = {}
    for arm in ["baseline", "magistr"]:
        trials = [r for r in rows if r["arm"] == arm]
        counts = Counter(r["status"] for r in trials)
        aggregate[arm] = {
            "trials": len(trials),
            "statuses": dict(counts),
            "graded_without_error": counts["passed"] + counts["failed"],
            "mcp_calls": sum(sum(r["mcp_calls"].values()) for r in trials),
        }
    return {"arms": aggregate, "trials": rows, "leaderboard_comparable": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    args = parser.parse_args()
    summary = summarize(args.results.resolve())
    (args.results / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
