#!/usr/bin/env python3
"""Exercise bounded work through the real native window and its actual owners."""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import time

from draft_persistence_journey import Journey, button, text_has, label
from native_window import ROOT, source_hashes, native_arguments
from process_roles import Rpc


def seed(journey):
    with Rpc(["mcp", "--dir", str(journey.first)], journey.first, env=journey.env) as rpc:
        rpc.request("initialize", {"protocolVersion": "2025-03-26", "capabilities": {},
                                  "clientInfo": {"name": "work-journey", "version": "1"}})
        rpc.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        args = {
            "id": "sample", "name": "Improve the sample", "title": "Improve the sample function.",
            "description": "Exercise the work owner in an isolated project.",
            "acceptance": ["The sample function is reviewed."], "priority": 1}
        result = rpc.request("tools/call", {"name": "execute", "arguments": {
            "source": "return magistr.call('roadmap_add_chunk', " + json.dumps(args) + ");"}})
        assert not result.get("isError"), result


def exercise(journey, work):
    journey.launch()
    journey.open_first()
    journey.wait("one item and bounded run starts are available", button("Start 10 items"))
    journey.call("press", journey.pid, "Start 10 items")
    journey.wait("admitted run reports real running state", text_has("Working on item 1."))
    journey.call("press", journey.pid, "Pause after this item")
    journey.wait("pause is shown as a future boundary", text_has("Finishing this item, then pausing."))
    journey.capture("work-running-dark")
    first = next(label(row) for row in journey.rows()
                 if row.get("AXRole") == "AXButton" and label(row).endswith("session 1"))
    journey.new("Other conversation")
    journey.call("press", journey.pid, "Work on next item")
    journey.wait("another resident refuses before starting or clearing work",
                 text_has("Work is already running"))
    assert not (work / "started-2.json").exists()
    journey.call("press", journey.pid, first)
    journey.wait("navigation preserves the requested pause", text_has("Finishing this item, then pausing."))
    (work / "release-1").touch()
    journey.wait("actual boundary reports paused work", text_has("Work paused."), timeout=30)
    assert not (work / "started-2.json").exists()
    journey.capture("work-paused-dark")
    journey.call("press", journey.pid, "One more item")
    journey.wait("step starts precisely the next item", text_has("Working on item 2."))
    (work / "release-2").touch()
    journey.wait("step pauses again after that item", text_has("Work paused."), timeout=30)
    assert not (work / "started-3.json").exists()
    journey.call("choose-option", journey.pid, "", "Appearance", "Light")
    journey.capture("work-paused-light")
    journey.call("press", journey.pid, "Continue work")
    journey.wait("resume starts the next item", text_has("Working on item 3."))
    journey.call("press", journey.pid, "Finish item and stop")
    journey.wait("stop is shown without cancelling the reply", text_has("Finishing this item, then stopping."))
    assert not (work / "ended-3").exists()
    (work / "release-3").touch()
    journey.wait("stop ends after the current item", text_has("Work ended."), timeout=30)
    journey.wait("next launch waits for an available conversation", button("Work on next item"), timeout=30)
    assert (work / "ended-3").read_text() == "end_turn"
    assert not (work / "started-4.json").exists()
    journey.call("press", journey.pid, "Work on next item")
    journey.wait("one item starts a fresh owned run", text_has("Working on item 4."))
    (work / "release-4").touch()
    journey.wait("single-item run ends at its real limit", text_has("Work ended."), timeout=30)
    assert not (work / "started-5.json").exists()
    journey.capture("work-ended-light")
    journey.wait("next single item can be admitted", button("Work on next item"))
    journey.call("press", journey.pid, "Work on next item")
    journey.wait("reply cancellation has an active reply", text_has("Working on item 5."))
    journey.call("press", journey.pid, "Stop")
    journey.wait("cancelling a reply settles its work", text_has("Work ended."), timeout=30)
    assert (work / "ended-5").read_text() == "cancelled"
    journey.results["checks"]["reply cancellation differs from stop after item"] = True
    journey.capture("work-reply-cancelled")
    journey.stop()
    journey.results["checks"]["native Quit settles all provider children"] = True


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--evidence", type=Path, default=ROOT / ".magistr/desktop-work")
    args = native_arguments(parser)
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="magistr-work-journey-") as temporary:
        parent = Path(temporary)
        journey = Journey(args.binary.resolve(), evidence, parent)
        work = parent / "work"
        work.mkdir()
        journey.env["JOURNEY_WORK"] = str(work)
        # Keep each fixture attempt progressing so step tests reach another boundary.
        path = journey.first / ".magistr/project.json"
        config = json.loads(path.read_text())
        config["quality_gate"]["check"] = "git -c user.name=Journey -c user.email=journey@example.invalid -c commit.gpgsign=false commit --allow-empty -qm work"
        path.write_text(json.dumps(config))
        journey.env["MAGISTR_GOAL"] = "off"
        try:
            seed(journey)
            exercise(journey, work)
            journey.results["source_hashes"] = source_hashes()
            journey.results["captures"] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                           for path in evidence.glob("work-*.png")}
            journey.results["captured_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            (evidence / "results.json").write_text(json.dumps(journey.results, indent=2))
        finally:
            journey.close()


if __name__ == "__main__":
    main()
