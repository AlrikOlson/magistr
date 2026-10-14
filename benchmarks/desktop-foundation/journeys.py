#!/usr/bin/env python3
"""Exercise the disposable desktop fixture through real macOS controls."""
import argparse
import json
from pathlib import Path
import time

from measure import launch, native, quit_fixture, read, wait_file, write

UNICODE = "résumé 東京 العربية Ελληνικά 👩🏽‍💻\nSecond line"


def tree(pid, output, name):
    path = output / (name + ".json")
    native("ax", pid, path)
    return read(path)


def exercise(output):
    output = Path(output)
    environment = native("environment", json_result=True)
    assert environment["accessibility"] and environment["screen_capture"]
    output.mkdir(parents=True, exist_ok=True)
    pid = None
    observations = []
    snapshot_path = output / "app/snapshot.json"

    def observe(name, passed, **evidence):
        observations.append({"name": name, "passed": passed, **evidence})

    def snapshot(predicate=lambda _: True):
        return wait_file(snapshot_path, predicate, timeout=3)

    try:
        pid, elapsed = launch(output / "app")
        native("resize", pid, 1280, 832)
        native("focus", pid, "Message")
        native("paste", pid, UNICODE)
        value = snapshot(lambda row: row["draft"] == UNICODE)
        native("key", pid, 0, 1 << 20)
        copied = native("copy", pid, json_result=True)["text"]
        observe("multilingual multiline editing and selection/copy", copied == UNICODE,
                expected=UNICODE, copied=copied, snapshot=value)
        tree(pid, output, "multilingual")
        native("screenshot", pid, output / "multilingual.png")

        # Collapse selection at its end, then extend it by one grapheme.
        native("key", pid, 124, 0)
        native("key", pid, 123, 1 << 17)
        partial = native("copy", pid, json_result=True)["text"]
        observe("keyboard partial selection", partial == "e", copied=partial)
        native("key", pid, 124, 0)
        native("key", pid, 36, 1 << 17)
        native("key", pid, 0, 0)
        time.sleep(.15)
        value = snapshot()
        observe("Shift Return inserts a line without sending",
                value["draft"] == UNICODE + "\na", snapshot=value)
        # Restore the same unsent request before checking workspace selection.
        native("key", pid, 0, 1 << 20)
        native("paste", pid, UNICODE)
        snapshot(lambda row: row["draft"] == UNICODE)

        native("press", pid, "Open workspace")
        picker = tree(pid, output, "workspace-picker")
        native("screenshot", pid, output / "workspace-picker.png")
        observe("open workspace presents a named chooser",
                any(row.get("AXRole") == "AXDialog" or row.get("AXSubrole") in ("AXDialog", "AXApplicationDialog")
                    or (row.get("AXRole") == "AXGroup" and row.get("AXTitle") == "Open workspace")
                    for row in picker), accessibility=picker)
        native("press", pid, "Source review · Review branch")
        value = snapshot(lambda row: row["workspace"] == 1)
        observe("choose workspace preserves the unsent request", value["draft"] == UNICODE,
                snapshot=value)

        native("press", pid, "Start work")
        before = snapshot(lambda row: row["running"] and row["received"] >= 80)
        native("key", pid, 18, 1 << 20)
        try:
            after = snapshot(lambda row: row["workspace"] == 0 and row["received"] > before["received"])
            keyboard_workspace = True
        except RuntimeError:
            keyboard_workspace = False
            native("press", pid, "Main branch")
            after = snapshot(lambda row: row["workspace"] == 0 and row["received"] > before["received"])
        observe("keyboard selects workspace during work", keyboard_workspace)
        observe("fixture stream continues across workspace selection",
                after["running"] and after["draft"] == UNICODE, before=before, after=after)
        started = time.monotonic()
        native("press", pid, "Stop")
        stopped = snapshot(lambda row: not row["running"])
        stop_ms = (time.monotonic() - started) * 1000
        time.sleep(.35)
        settled = snapshot()
        observe("stop settles the actual fixture producers",
                settled["received"] == stopped["received"] and not settled["running"],
                stopped=stopped, settled=settled, observer_round_trip_ms=stop_ms)

        native("press", pid, "Allow once")
        answer = wait_file(output / "app/journey.json", lambda row: row.get("answer") == "allow")
        value = snapshot(lambda row: not row["waiting"])
        observe("answer request preserves the draft", answer["draft"] == UNICODE,
                answer=answer, snapshot=value)

        native("press", pid, "Roadmap")
        snapshot(lambda row: row["roadmap"])
        native("key", pid, 2, 1 << 20)
        try:
            value = snapshot(lambda row: not row["roadmap"])
            keyboard_changes = True
        except RuntimeError:
            keyboard_changes = False
            native("press", pid, "Changes")
            value = snapshot(lambda row: not row["roadmap"])
        observe("keyboard opens changes after answering a request", keyboard_changes)
        native("screenshot", pid, output / "changes.png")
        changes = tree(pid, output, "changes")
        observe("review the source diff", not value["roadmap"], snapshot=value,
                accessible_source_changes=any(row.get("AXDescription") == "Source changes" for row in changes))

        native("key", pid, 15, 1 << 20)
        try:
            snapshot(lambda row: row["roadmap"] ,)
            keyboard_roadmap = True
        except RuntimeError:
            keyboard_roadmap = False
            native("press", pid, "Roadmap")
            snapshot(lambda row: row["roadmap"])
        observe("keyboard opens roadmap after answering a request", keyboard_roadmap)
        native("press", pid, "1 · Agent workspace · Ready")
        selected = snapshot(lambda row: row["item"] == 0)
        evidence = wait_file(output / "app/journey.json", lambda row: row.get("item") == 0)
        native("screenshot", pid, output / "roadmap.png")
        tree(pid, output, "roadmap")
        observe("inspect roadmap requirements and evidence",
                evidence["requirements"] and evidence["evidence"], snapshot=selected, record=evidence)

        native("press", pid, "Changes")
        snapshot(lambda row: not row["roadmap"])
        native("screenshot", pid, output / "dark.png")
        native("press", pid, "Change theme")
        value = snapshot(lambda row: row["light"])
        native("screenshot", pid, output / "light.png")
        observe("light theme", value["light"], snapshot=value)
        native("press", pid, "Reduce motion")
        value = snapshot(lambda row: not row["reduced_motion"])
        native("press", pid, "Reduce motion")
        value = snapshot(lambda row: row["reduced_motion"])
        observe("reduced motion preference", value["reduced_motion"], snapshot=value)
        native("resize", pid, 900, 800)
        native("screenshot", pid, output / "narrow.png")

        # Real native IME conversion is retained separately. Automatic activation
        # proved unreliable on this Mac; it is an unavailable automation capability.
        ime_observation = {
            "status": "unavailable",
            "reason": "Observer source selection did not reliably activate native composition; successful native conversion is retained separately.",
            "evidence": "docs/evidence/desktop-foundation/journeys/tauri/ime.json",
        }
        result = {"backend": "tauri", "launch_ms": elapsed, "checks": observations,
                  "all_passed": all(row["passed"] for row in observations),
                  "ime_automation": ime_observation,
                  "scope": "disposable fixture, not production workflow admission"}
        write(output / "journeys.json", result)
        print(json.dumps({"backend": "tauri", "checks": [
            {"name": row["name"], "passed": row["passed"]} for row in observations]}, ensure_ascii=False))
        return result
    except Exception as error:
        write(output / "journeys.json", {
            "backend": "tauri", "checks": observations, "all_passed": False,
            "unfinished": str(error), "scope": "disposable fixture"})
        raise
    finally:
        try:
            if pid:
                quit_fixture(pid)
        finally:
            native("restore-front", environment["front_pid"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = exercise(args.output)
    if not result["all_passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
