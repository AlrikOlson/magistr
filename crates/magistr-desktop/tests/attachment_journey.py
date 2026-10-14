"""End-to-end attachment checks through native macOS input and real ACP traffic."""
import base64
import hashlib
import json

from attachment_fixture import PREFIX, fixture_files


def exercise_attachments(*, call, pid, evidence, results, send, draft, wait,
                         text_has, rows, directory, control):
    files = fixture_files(directory)
    checks = results["checks"]
    observed = {"picker": [], "delivered": {}, "refused": []}
    wire = {}
    window = next(row["rect"] for row in rows()
                  if row.get("AXRole") == "AXWindow" and row.get("rect", [0, 0, 0])[2] > 400)

    def save():
        (evidence / "attachment-observation.json").write_text(
            json.dumps(observed, ensure_ascii=False, indent=2) + "\n")
        (evidence / "attachment-prompts.json").write_text(
            json.dumps(wire, ensure_ascii=False, indent=2) + "\n")

    def label(row):
        return row.get("AXDescription") or row.get("AXTitle", "")

    def pending(tree):
        return [label(row)[7:] for row in tree
                if row.get("AXRole") == "AXButton" and label(row).startswith("Remove ")]

    def exact_draft(value):
        return lambda tree: any(row.get("AXRole") == "AXTextArea" and
                                row.get("AXValue") == value for row in tree)

    def button(name, enabled=True):
        return lambda tree: any(row.get("AXRole") == "AXButton" and
                                name in (row.get("AXTitle"), row.get("AXDescription")) and
                                row.get("enabled") == enabled for row in tree)

    def prompt_for(name):
        messages = []
        for path in control.glob("prompt-*.json"):
            message = json.loads(path.read_text())
            blocks = message.get("params", {}).get("prompt", [])
            if blocks and blocks[0].get("text") == PREFIX + name:
                messages.append(message)
        return messages

    def choose(name, expected=True):
        call("press", pid, "Add files…")
        wait("native attachment picker opens for " + name,
             lambda tree: any(row.get("AXRole") == "AXSheet" or
                              (row.get("AXRole") == "AXWindow" and row.get("AXTitle") == "Add files")
                              for row in tree))
        path = directory / name
        observed["picker"].append(call("attachment-file", pid, path))
        if expected:
            wait("selected attachment appears: " + name, lambda tree: name in pending(tree))
        save()

    def remove(name):
        call("press", pid, "Remove " + name)
        wait("attachment removal settles: " + name, lambda tree: name not in pending(tree))

    def delivered(name, text_names=(), image_data=()):
        wait("the ACP peer receives " + name, text_has("Native attachment accepted: " + name))
        wait("accepted input clears only its submitted files: " + name,
             lambda tree: not pending(tree) and exact_draft("")(tree))
        messages = prompt_for(name)
        assert len(messages) == 1, (name, len(messages))
        blocks = messages[0]["params"]["prompt"]
        texts = [block["text"] for block in blocks if block.get("type") == "text"]
        images = [block for block in blocks if block.get("type") == "image"]
        assert texts[0] == PREFIX + name, texts[0]
        for filename in text_names:
            expected = "Attached file: " + filename + "\n\n" + files[filename].decode()
            assert texts.count(expected) == 1, (name, filename, [len(value) for value in texts])
        assert len(images) == len(image_data), (name, len(images), len(image_data))
        for block, expected in zip(images, image_data):
            assert block["mimeType"] == "image/png", block["mimeType"]
            assert base64.b64decode(block["data"], validate=True) == expected, name
        assert len(blocks) == 1 + len(text_names) + 2 * len(image_data), (name, len(blocks))
        wire[name] = messages[0]
        observed["delivered"][name] = {
            "text_files": {filename: {"bytes": len(files[filename]),
                "sha256": hashlib.sha256(files[filename]).hexdigest()} for filename in text_names},
            "images": [{"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
                       for data in image_data],
            "wire_blocks": len(blocks),
        }
        save()

    def refused(name, message, expected):
        wait("attachment refusal explains " + name, text_has(message))
        tree = wait("attachment refusal retains the draft and files: " + name,
                    lambda tree: exact_draft(PREFIX + name)(tree) and pending(tree) == list(expected))
        assert not prompt_for(name), name
        observed["refused"].append({"case": name, "files": pending(tree), "peer_prompts": 0})
        save()

    draft("Keep my draft while choosing files")
    before = len(list(control.glob("prompt-*.json")))
    call("press", pid, "Add files…")
    wait("attachment chooser is a native modal",
         lambda tree: any(row.get("AXRole") == "AXSheet" for row in tree))
    observed["cancel"] = call("attachment-cancel", pid)
    wait("cancelling native file selection preserves the draft",
         lambda tree: exact_draft("Keep my draft while choosing files")(tree) and not pending(tree))
    assert len(list(control.glob("prompt-*.json"))) == before
    checks["native attachment picker cancellation sends nothing and preserves the draft"] = True

    choose("preview.png")
    choose("reading-note.txt")
    wait("the selected image exposes an accessible preview", text_has("Preview of preview.png"))

    def preview_sample(tree):
        summary = next(row for row in tree if row.get("AXRole") == "AXDisclosureTriangle" and
                       label(row) == "Preview reading-note.txt")
        excerpt = next((row.get("AXValue", "") for row in tree
                        if row.get("AXRole") == "AXStaticText" and
                        "Native attachment reading note" in row.get("AXValue", "")), "")
        return {"summary": summary, "excerpt_visible": bool(excerpt), "excerpt_sample": excerpt[:160]}

    preview_before = preview_sample(rows())
    assert not preview_before["excerpt_visible"]
    call("press", pid, "Preview reading-note.txt")
    preview_open = preview_sample(wait("the selected text exposes its native disclosure",
                                      text_has("Native attachment reading note")))
    call("press", pid, "Preview reading-note.txt")
    preview_closed = preview_sample(wait("the text attachment preview closes through its native control",
        lambda tree: not text_has("Native attachment reading note")(tree)))
    observed["text_preview"] = {"before": preview_before, "opened": preview_open, "closed": preview_closed}
    checks["native text attachment disclosure opens and closes"] = True
    save()
    for theme in ("Dark", "Light"):
        call("choose-option", pid, "", "Appearance", theme)
        call("screenshot", pid, evidence / ("attachments-" + theme.lower() + ".png"))
    call("resize", pid, 800, 680)
    call("screenshot", pid, evidence / "attachments-narrow.png")
    call("ax", pid, evidence / "attachments-accessibility.json")
    call("resize", pid, window[2], window[3])
    call("press", pid, "First project session 3")
    wait("another session has no files from the selected conversation",
         lambda tree: not pending(tree) and text_has("First project session 3")(tree))
    call("press", pid, "Release review")
    wait("switching back restores the addressed draft and its attachments",
         lambda tree: pending(tree) == ["preview.png", "reading-note.txt"] and
                      exact_draft("Keep my draft while choosing files")(tree))
    checks["native session switching keeps attachments with their owning draft"] = True
    draft(PREFIX + "files")
    call("press", pid, "Send")
    delivered("files", ("reading-note.txt",), (files["preview.png"],))
    checks["native picker sends original PNG and complete CRLF Unicode text bytes to ACP"] = True

    for name, action, message, case in (
        ("changed-note.txt", lambda path: path.write_bytes(b"New bytes after selection"),
         "changed after you attached it", "changed"),
        ("missing-note.txt", lambda path: path.unlink(),
         "is missing or cannot be read", "missing"),
    ):
        choose(name)
        action(directory / name)
        draft(PREFIX + case)
        call("press", pid, "Send")
        refused(case, message, (name,))
        remove(name)
    checks["native send refuses changed and missing selected files without losing input"] = True

    draft("Keep my draft after choosing an unavailable file")
    for name, message in (
        ("too-large.txt", "is too large"),
        ("binary.bin", "choose a text file"),
    ):
        choose(name, expected=False)
        wait("native selection explains " + name, text_has(message))
        assert not pending(rows()), name
        assert exact_draft("Keep my draft after choosing an unavailable file")(rows())
    checks["native picker refuses oversized and non-image binary files without truncation"] = True

    draft(PREFIX + "clipboard")
    call("focus", pid, "Message the agent")
    observed["clipboard"] = call("attachment-paste-image", pid, directory / "preview.png")
    wait("native PNG clipboard paste stages one image", lambda tree: len(pending(tree)) == 1)
    observed["clipboard"]["staged_name"] = pending(rows())[0]
    call("press", pid, "Send")
    delivered("clipboard", image_data=(files["preview.png"],))
    checks["native PNG clipboard paste reaches ACP as the original image bytes"] = True

    draft(PREFIX + "drop")
    observed["drop"] = call("attachment-drop", pid, directory / "drop-note.txt")
    wait("native file-url drag stages the selected source file",
         lambda tree: pending(tree) == ["drop-note.txt"])
    call("press", pid, "Send")
    delivered("drop", ("drop-note.txt",))
    checks["native file drop routes through source admission and preserves exact bytes"] = True

    send("hang attachment cancellation")
    wait("work remains cancellable while the next attachment is staged", text_has("hanging until cancel"))
    choose("preview.png")
    draft("Keep this attachment after stopping")
    call("press", pid, "Stop")
    wait("native Stop keeps the unsent attachment with its draft",
         lambda tree: text_has("Stopped")(tree) and pending(tree) == ["preview.png"] and
                      exact_draft("Keep this attachment after stopping")(tree))
    remove("preview.png")
    checks["native cancellation preserves unsent attachment drafts"] = True

    unsupported = control / "without-images"
    unsupported.write_text("Negotiate an agent that cannot receive images")
    try:
        send("fail attachment capability")
        wait("the existing session exposes real reconnect after provider failure", button("Reconnect"))
        call("press", pid, "Reconnect")
        wait("reconnect negotiates unsupported image capability",
             lambda tree: button("Reconnect", False)(tree) and any(
                 not json.loads(path.read_text())["images"]
                 for path in control.glob("negotiation-*.json")))
        choose("preview.png")
        choose("reading-note.txt")
        draft(PREFIX + "unsupported")
        call("press", pid, "Send")
        refused("unsupported", "does not accept images", ("preview.png", "reading-note.txt"))
        call("screenshot", pid, evidence / "attachments-refused.png")
        checks["unsupported images are refused before ACP and retain text plus selected files"] = True
        remove("preview.png")
        call("press", pid, "Send")
        delivered("unsupported", ("reading-note.txt",))
        checks["removing an unsupported image lets the retained text attachment send"] = True
    finally:
        unsupported.unlink(missing_ok=True)
        save()
    draft("")
    observed["native_accessibility_sha256"] = hashlib.sha256(
        (evidence / "attachments-accessibility.json").read_bytes()).hexdigest()
    save()
    results["attachments"] = observed
    results["attachment_observation_sha256"] = hashlib.sha256(
        (evidence / "attachment-observation.json").read_bytes()).hexdigest()
    results["attachment_prompts_sha256"] = hashlib.sha256(
        (evidence / "attachment-prompts.json").read_bytes()).hexdigest()
    print("PASS: native attachment selection, paste, drop, refusal and exact ACP content", flush=True)
