#!/usr/bin/env python3
"""Verify durable drafts and preferences by restarting one isolated native app."""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import sqlite3
import subprocess
import tempfile
import time
import tomllib

from attachment_fixture import fixture_files
from native_window import ROOT, SOURCE, project, run, source_hashes
from session_controls_fixture import peer_script, start_api


def text_has(value):
    return lambda tree: any(value in str(row.get(key, "")) for row in tree
                            for key in ("AXTitle", "AXDescription", "AXValue"))


def label(row):
    return row.get("AXDescription") or row.get("AXTitle", "")


def editor(value, name="Message the agent"):
    return lambda tree: any(row.get("AXRole") == "AXTextArea" and
                            label(row) == name and row.get("AXValue") == value and
                            row.get("enabled") for row in tree)


def button(name, enabled=True):
    return lambda tree: any(row.get("AXRole") == "AXButton" and label(row) == name
                            and row.get("enabled") == enabled for row in tree)


def pending(tree):
    return [label(row)[7:] for row in tree if row.get("AXRole") == "AXButton"
            and label(row).startswith("Remove ")]


class Journey:
    def __init__(self, binary, evidence, parent):
        self.binary, self.evidence, self.parent = binary, evidence, parent
        self.process = self.log = None
        self.launches = 0
        self.results = {"checks": {}, "restarts": []}
        self.driver = evidence / "draft-driver"
        run("swiftc", "-parse-as-library",
            *(SOURCE / ("tests/" + name) for name in
              ("native_window.swift", "rich_conversation.swift", "attachment.swift",
               "draft_persistence.swift")), "-o", self.driver, timeout=60)
        environment = self.call("environment")
        assert all(environment[key] for key in
                   ("accessibility", "screen_capture", "post_events")), environment
        self.results["environment"] = environment
        self.front = environment["front_pid"]
        self.first, self.worktree = parent / "first", parent / "feature"
        project(self.first, "Draft project")
        run("git", "-C", self.first, "worktree", "add", "-b", "feature", self.worktree)
        self.files = fixture_files(self.first)
        bundle = parent / "Magistr Draft Journey.app"
        self.executable = bundle / "Contents/MacOS/magistr-desktop"
        self.executable.parent.mkdir(parents=True)
        shutil.copy2(binary, self.executable)
        (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps({
            "CFBundleIdentifier": "dev.magistr.desktop.journey." + str(time.time_ns()),
            "CFBundleExecutable": "magistr-desktop", "CFBundleName": "Magistr Draft Journey",
            "CFBundlePackageType": "APPL", "NSHighResolutionCapable": True,
        }))
        run("codesign", "--force", "--sign", "-", bundle)
        tools = parent / "bin"
        tools.mkdir()
        peer = tools / "copilot"
        peer.write_text(peer_script(self.executable))
        peer.chmod(0o755)
        self.peers, self.wire = parent / "peers", parent / "wire"
        self.api = start_api()
        self.api.posts = []
        original_post = self.api.RequestHandlerClass.do_POST

        def observe_post(handler):
            self.api.posts.append(handler.path)
            return original_post(handler)

        self.api.RequestHandlerClass.do_POST = observe_post
        # Preserve the macOS home needed by WebKit; isolate every application writer.
        self.env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                        XDG_DATA_HOME=str(parent / "data"),
                        XDG_CONFIG_HOME=str(parent / "config"),
                        MAGISTR_HOME=str(parent / "state"), MAGISTR_WORKSPACE_NETWORK="0",
                        JOURNEY_PEERS=str(self.peers), JOURNEY_REFUSE_LOAD=str(parent / "refuse"),
                        JOURNEY_ATTACHMENTS=str(self.wire), OPENAI_API_KEY="native-fixture",
                        OPENAI_BASE_URL="http://127.0.0.1:" + str(self.api.server_port),
                        RUST_LOG="info")
        self.database = parent / "data/magistr/sessions.db"
        self.config = parent / "config/magistr/config.toml"

    @property
    def pid(self):
        return self.process.pid

    def call(self, mode, *values):
        try:
            output = run(self.driver, mode, *values, timeout=30)
            return json.loads(output.stdout) if output.stdout.strip() else None
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
            if self.process is not None and self.process.poll() is None:
                stamp = str(time.time_ns())
                for operation, suffix in (("ax", ".json"), ("screenshot", ".png")):
                    try:
                        run(self.driver, operation, self.pid,
                            self.evidence / (stamp + "-draft-failure" + suffix), timeout=10)
                    except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
                        pass
            raise AssertionError(f"Native {mode} failed: {error.stderr}") from error

    def rows(self):
        assert self.process.poll() is None, self.log_path.read_text()
        return self.call("ax", self.pid)

    def wait(self, name, predicate, timeout=12):
        deadline = time.monotonic() + timeout
        tree = []
        while time.monotonic() < deadline:
            tree = self.rows()
            if predicate(tree):
                self.results["checks"][name] = True
                print("PASS:", name, flush=True)
                return tree
            time.sleep(.05)
        (self.evidence / "draft-failure-ax.json").write_text(
            json.dumps(tree, ensure_ascii=False, indent=2))
        self.call("screenshot", self.pid, self.evidence / "draft-failure.png")
        raise AssertionError(name + "; see draft-failure-ax.json and draft launch log")

    def launch(self):
        self.launches += 1
        self.log_path = self.evidence / f"draft-launch-{self.launches}.log"
        self.log = self.log_path.open("w")
        self.process = subprocess.Popen(
            [str(self.executable), "--provider", "copilot", "--trust", "cautious"],
            env=self.env, cwd=self.first, stdout=self.log, stderr=self.log)
        self.call("activate", self.pid)
        self.wait(f"launch {self.launches} opens the chooser without resuming",
                  text_has("Choose a folder…"))

    def settle_peers(self):
        deadline = time.monotonic() + 12
        while True:
            live = []
            for path in self.peers.glob("*.json"):
                child = json.loads(path.read_text())["pid"]
                try:
                    os.kill(child, 0)
                    live.append(child)
                except ProcessLookupError:
                    pass
            if not live:
                return
            assert time.monotonic() < deadline, ("Fixture providers did not settle", live)
            time.sleep(.05)

    def stop(self, interrupted=False, quit_requested=False):
        process = self.process
        if interrupted:
            process.kill()
        elif not quit_requested:
            self.call("quit", process.pid)
        code = process.wait(timeout=15)
        assert code == (-9 if interrupted else 0), (code, self.log_path.read_text())
        self.log.close()
        self.log = None
        self.process = None
        self.settle_peers()
        if not interrupted:
            assert "could not record session cost" not in self.log_path.read_text()
            with sqlite3.connect(self.database) as connection:
                conversations = {row[0] for row in connection.execute("SELECT session_id FROM sessions")}
            ledger = json.loads((self.database.parent / "costs.json").read_text())
            assert {entry["session_id"] for entry in ledger} == conversations, ledger
            assert len(ledger) == len(conversations), ledger
            self.results["checks"]["concurrent Quit retains every conversation's spending record"] = True
        self.results["restarts"].append({
            "launch": self.launches, "exit": code,
            "kind": "interrupted" if interrupted else "native_quit",
        })

    def edit(self, value, name="Message the agent"):
        self.call("focus", self.pid, name)
        self.call("key", self.pid, 0, 1048576)
        self.call("paste", self.pid, value)

    def saved(self, value):
        self.wait("latest draft is durably saved: " + value[:35],
                  lambda tree: editor(value)(tree) and text_has("Draft saved")(tree))

    def rename(self, name):
        self.edit(name, "Conversation name")
        self.call("press", self.pid, "Save name")
        self.wait("conversation name belongs to its owner: " + name, button(name))

    def open_first(self):
        self.call("press", self.pid, "Open project")
        self.edit(str(self.first), "Project folder")
        self.call("press", self.pid, "Open")
        self.wait("the main working copy opens", lambda tree:
                  text_has(str(self.first))(tree) and editor("")(tree))

    def new(self, name, provider=None):
        self.call("press", self.pid, "New session")
        self.wait("new conversation is ready: " + name, button("Start conversation"))
        if provider:
            self.call("choose-option", self.pid, "New conversation", "Agent", provider)
        self.call("press", self.pid, "Start conversation")
        self.wait("new conversation editor opens: " + name, editor(""))
        self.rename(name)

    def history(self, name, value):
        self.call("press", self.pid, "Saved conversations")
        self.wait("saved conversations include " + name, button(name))
        self.call("press", self.pid, name)
        self.wait("saved text is readable without a provider: " + name,
                  editor(value, "Saved message"))

    def resume(self, name, value):
        self.history(name, value)
        self.call("press", self.pid, "Resume conversation")
        self.wait("resuming restores the exact draft: " + name, editor(value))

    def choose_file(self, name):
        self.call("press", self.pid, "Add files…")
        self.call("attachment-file", self.pid, self.first / name)
        self.wait("pending file appears: " + name, lambda tree: name in pending(tree))

    def prompts(self):
        return [json.loads(path.read_text()) for path in sorted(self.wire.glob("prompt-*.json"))]

    def no_prompts(self, expected=0):
        prompts = self.prompts()
        assert len(prompts) == expected, prompts
        assert not self.api.posts, self.api.posts

    def capture(self, name):
        self.call("screenshot", self.pid, self.evidence / (name + ".png"))
        self.call("ax", self.pid, self.evidence / (name + "-accessibility.json"))

    def durable(self, name):
        with sqlite3.connect(f"file:{self.database}?mode=ro", uri=True) as database:
            database.row_factory = sqlite3.Row
            row = database.execute(
                "SELECT s.session_id,s.workspace,d.revision,d.text,d.attachments "
                "FROM sessions s LEFT JOIN session_drafts d ON d.session_id=s.session_id "
                "WHERE s.name=?", (name,)).fetchone()
        assert row is not None, name
        return dict(row)

    def close(self):
        try:
            if self.process is not None and self.process.poll() is None:
                try:
                    self.stop()
                except Exception:
                    if self.process is not None and self.process.poll() is None:
                        self.process.kill()
                        self.process.wait(timeout=5)
                    raise
        finally:
            if self.log is not None:
                self.log.close()
            self.api.shutdown()
            self.api.server_close()
            self.call("restore-front", self.front)


def exercise(journey):
    j = journey
    first = "A first unsent message · 日本語 · café · 👩🏽‍💻\nKeep both lines."
    second = "A second conversation has its own draft."
    feature = "Notes for the feature working copy only."
    offline = "A saved message remains editable without agent resume."
    j.launch()
    j.no_prompts()
    j.open_first()
    j.rename("Main notes")
    j.edit(first)
    j.choose_file("changed-note.txt")
    j.choose_file("missing-note.txt")
    j.call("focus", j.pid, "Message the agent")
    j.call("attachment-paste-image", j.pid, j.first / "preview.png")
    attached = pending(j.wait("source files and clipboard image belong to the first draft",
                             lambda tree: len(pending(tree)) == 3))
    j.saved(first)
    j.new("Second notes")
    j.edit(second)
    j.saved(second)
    j.call("press", j.pid, "Working copy")
    j.call("press", j.pid, "feature")
    j.wait("the linked working copy opens independently",
           lambda tree: text_has(str(j.worktree))(tree) and editor("")(tree))
    j.rename("Feature notes")
    j.edit(feature)
    j.saved(feature)
    j.call("press", j.pid, "Main notes")
    j.wait("switching conversations retains exact text and attachments",
           lambda tree: editor(first)(tree) and pending(tree) == attached)
    j.new("Offline notes", "OpenAI")
    j.edit(offline)
    j.call("press", j.pid, "Appearance")
    j.call("press", j.pid, "Dark")
    j.call("press", j.pid, "Reduce motion")
    j.wait("saved appearance and motion are selected",
           lambda tree: any(row.get("AXRole") == "AXPopUpButton" and
                            label(row) == "Appearance" and row.get("AXValue") == "Dark"
                            for row in tree))
    j.saved(offline)
    config = tomllib.loads(j.config.read_text())
    assert config["theme"]["default"] == "dark", config
    assert config["accessibility"]["reduce_motion"] is True, config
    j.results["preferences"] = config
    # Quit with the final native edit still focused; no manual save or navigation follows.
    offline += "\nThe last edit was followed directly by Quit."
    j.no_prompts()
    j.results["immediate_quit"] = j.call("draft-last-edit-quit", j.pid, offline)
    j.stop(quit_requested=True)
    j.results["checks"]["native Quit preserves the focused final edit"] = True

    (j.first / "changed-note.txt").write_bytes(b"Changed while the app was closed.")
    (j.first / "missing-note.txt").unlink()
    provider_count = len(list(j.peers.glob("*.json")))
    j.launch()
    j.wait("appearance restores after relaunch without a command line override",
           lambda tree: any(row.get("AXRole") == "AXPopUpButton" and
                            label(row) == "Appearance" and row.get("AXValue") == "Dark"
                            for row in tree))
    j.results["motion_restored"] = j.call("draft-preferences", j.pid)
    assert j.results["motion_restored"]["reduce_motion"] is True
    assert len(list(j.peers.glob("*.json"))) == provider_count
    j.history("Offline notes", offline)
    j.wait("a non-resumable saved conversation stays editable", button("Resume conversation", False))
    edited_offline = offline + "\nEdited after restarting."
    j.edit(edited_offline, "Saved message")
    j.call("press", j.pid, "Save message")
    j.wait("editing saved text does not start a provider",
           lambda tree: j.durable("Offline notes")["text"] == edited_offline)
    assert len(list(j.peers.glob("*.json"))) == provider_count
    actions = ("Save message", "Discard saved draft")
    j.results["saved_message_actions"] = {
        name: j.call("rich-keyboard", j.pid, name) for name in actions}
    tree = j.wait("saved-message actions remain visible and enabled after editing",
                  lambda rows: all(button(name)(rows) for name in actions)
                  and editor(edited_offline, "Saved message")(rows))
    window = next(row["rect"] for row in tree if row.get("AXRole") == "AXWindow"
                  and row.get("rect", [0, 0, 0])[2] > 400)
    controls = [(name, "AXButton") for name in actions] + [
        ("Saved message", "AXTextArea"), ("Draft saved", "AXStaticText")]
    for name, role in controls:
        control = next(row for row in tree if row.get("AXRole") == role
                       and name in (label(row), row.get("AXValue")))
        x, y, width, height = control["rect"]
        assert width > 0 and height > 0 and window[0] <= x and window[1] <= y \
            and x + width <= window[0] + window[2] \
            and y + height <= window[1] + window[3], (name, control["rect"], window)
    j.capture("drafts-saved-message")
    j.call("key", j.pid, 53, 0)
    j.no_prompts()
    j.resume("Feature notes", feature)
    assert not pending(j.rows())
    assert j.durable("Feature notes")["workspace"] == str(j.worktree)
    j.resume("Second notes", second)
    assert not pending(j.rows())
    j.resume("Main notes", first)
    j.wait("source files and clipboard bytes restore with their conversation",
           lambda tree: pending(tree) == attached and editor(first)(tree))
    j.capture("drafts-restored-dark")
    j.call("press", j.pid, "Appearance")
    j.call("press", j.pid, "Light")
    j.capture("drafts-restored-light")
    bounds = next(row["rect"] for row in j.rows() if row.get("AXRole") == "AXWindow"
                  and row.get("rect", [0, 0, 0])[2] > 400)
    j.call("resize", j.pid, 800, 680)
    j.capture("drafts-restored-narrow")
    j.call("resize", j.pid, bounds[2], bounds[3])
    j.call("press", j.pid, "Send")
    j.wait("restored changed source refuses without losing the draft",
           lambda tree: text_has("changed after you attached it")(tree) and editor(first)(tree))
    j.no_prompts()
    j.call("press", j.pid, "Remove changed-note.txt")
    j.call("press", j.pid, "Send")
    j.wait("restored missing source refuses without losing the draft",
           lambda tree: text_has("is missing or cannot be read")(tree) and editor(first)(tree))
    j.no_prompts()
    j.call("press", j.pid, "Remove missing-note.txt")
    j.call("press", j.pid, "Send")
    j.wait("an explicit send clears the restored clipboard attachment",
           lambda tree: editor("")(tree) and not pending(tree) and len(j.prompts()) == 1)
    blocks = j.prompts()[0]["params"]["prompt"]
    assert blocks[0]["text"] == first, blocks
    images = [block for block in blocks if block["type"] == "image"]
    assert len(images) == 1 and base64.b64decode(images[0]["data"]) == j.files["preview.png"]
    j.results["checks"]["only explicit Send delivers exact restored clipboard bytes"] = True
    j.results["explicit_send"] = {"text_sha256": hashlib.sha256(first.encode()).hexdigest(),
                                 "image_sha256": hashlib.sha256(j.files["preview.png"]).hexdigest()}
    j.no_prompts(1)

    j.call("press", j.pid, "Second notes")
    j.wait("another resident draft survives an accepted send", editor(second))
    revisions = [f"Rapid saved revision {index:02d} · café 日本語" for index in range(32)]
    j.results["rapid_edits"] = j.call("draft-burst", j.pid, json.dumps(revisions, ensure_ascii=False))
    assert len({row["observed"] for row in j.results["rapid_edits"]["revisions"]}) > 1
    newest = revisions[-1]
    j.saved(newest)
    durable = j.durable("Second notes")
    assert durable["text"] == newest and durable["revision"] > 0, durable
    j.results["interrupted_revision"] = durable["revision"]
    j.stop(interrupted=True)
    j.launch()
    j.resume("Second notes", newest)
    j.results["checks"]["interrupted process restores the latest saved rapid revision"] = True
    j.no_prompts(1)

    failure_text = "Keep this editable while saving is unavailable."
    with sqlite3.connect(j.database, timeout=0) as blocker:
        blocker.execute("BEGIN IMMEDIATE")
        try:
            j.edit(failure_text)
            j.wait("failed persistence is visible without disabling the editor",
                   lambda tree: text_has("Your message is still open, but it could not be saved. Try again.")(tree) and editor(failure_text)(tree))
            failure_text += "\nEditing still works."
            j.edit(failure_text)
            j.wait("more editing survives a persistent save failure",
                   lambda tree: text_has("Your message is still open, but it could not be saved. Try again.")(tree) and editor(failure_text)(tree))
            j.call("quit", j.pid)
            j.wait("refused Quit keeps the unsaved message open and editable",
                   lambda tree: text_has("Could not close because changes are not saved.")(tree)
                   and editor(failure_text)(tree) and button("Retry save")(tree))
            failure_text += "\nQuit kept this message open."
            j.edit(failure_text)
            j.wait("editing continues after a refused Quit",
                   lambda tree: editor(failure_text)(tree) and button("Retry save")(tree))
            j.capture("drafts-save-failure")
        finally:
            blocker.rollback()
    j.call("press", j.pid, "Retry save")
    j.saved(failure_text)
    j.wait("successful retry clears save and Quit failure notices",
           lambda tree: not text_has("could not be saved")(tree)
           and not text_has("Could not close")(tree))
    assert j.durable("Second notes")["text"] == failure_text
    j.call("press", j.pid, "Discard draft")
    j.wait("explicit discard clears the current draft",
           lambda tree: editor("")(tree) and not j.durable("Second notes")["text"])
    j.history("Offline notes", edited_offline)
    j.call("press", j.pid, "Discard saved draft")
    j.wait("explicit discard clears saved text without a provider",
           lambda tree: not j.durable("Offline notes")["text"])
    j.call("key", j.pid, 53, 0)
    j.no_prompts(1)
    j.stop()
    j.launch()
    j.resume("Second notes", "")
    j.history("Offline notes", "")
    j.wait("discard stays durable for a non-resumable conversation",
           button("Resume conversation", False))
    j.call("key", j.pid, 53, 0)
    j.no_prompts(1)
    j.results["checks"]["explicit discard remains empty after another native Quit"] = True
    j.results["checks"]["opening editing and recovery send no unintended prompts"] = True
    j.results["prompt_count"] = len(j.prompts())
    j.results["api_posts"] = j.api.posts
    (j.evidence / "draft-prompts.json").write_text(
        json.dumps(j.prompts(), ensure_ascii=False, indent=2) + "\n")
    captures = [*j.evidence.glob("drafts-*.png"),
                *j.evidence.glob("drafts-*-accessibility.json"), j.evidence / "draft-prompts.json"]
    j.results["evidence_hashes"] = {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
                                  for path in captures}
    j.stop()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--evidence", type=Path, default=ROOT / ".magistr/desktop-native-drafts")
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="magistr-desktop-journey-") as temporary:
        journey = Journey(args.binary.resolve(), evidence, Path(temporary).resolve())
        try:
            exercise(journey)
            journey.results["source_hashes"] = source_hashes()
            journey.results["binary_sha256"] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
            journey.results["captured_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            (evidence / "draft-persistence-receipt.json").write_text(
                json.dumps(journey.results, ensure_ascii=False, indent=2) + "\n")
        finally:
            journey.close()


if __name__ == "__main__":
    main()
