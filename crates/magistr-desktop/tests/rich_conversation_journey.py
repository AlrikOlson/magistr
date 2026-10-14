"""Rich conversation assertions over the production window's native observer."""
import hashlib
import json
import re
from urllib.parse import urlsplit

from rich_conversation_fixture import (
    LATE_TEXT, LINK_LABEL, PROMPT, SELECTION_TEXT, STREAM_PREFIX,
    PROGRESS_TOOL_TITLE, TOOL_ERROR, TOOL_TITLE, source,
)


def exercise_rich_conversation(*, call, pid, evidence, results, send, wait,
                               text_has, rows, directory, control, link, api):
    """Native input, AX output and source bytes are independent proof channels."""
    observed = {}
    baseline = call("copy-button", pid, "Copy conversation")["text"]
    window = next(row["rect"] for row in rows() if row.get("AXRole") == "AXWindow" and row.get("rect", [0, 0, 0])[2] > 400)
    checks = results["checks"]

    def progress():
        path = control / "progress.json"
        return json.loads(path.read_text()) if path.exists() else {"chunks": 0}

    def stream_number(sample):
        values = "\n".join(sample["stream_values"])
        return max((int(value) for value in re.findall(r"Reading progress (\d+):", values)), default=0)

    def save():
        (evidence / "rich-observation.json").write_text(
            json.dumps(observed, ensure_ascii=False, indent=2) + "\n")

    send(PROMPT)
    wait("long rich reply arrives through the real owner", text_has("End of the original reading note."))
    wait("tool failure is visible", text_has(TOOL_TITLE + " · Failed"))
    observed["tool_keyboard"] = call("rich-keyboard", pid, TOOL_TITLE + " · Failed", "activate")
    wait("native keyboard expands tool error details", text_has(TOOL_ERROR))
    observed["selection_before"] = call("rich-select", pid, SELECTION_TEXT)
    save()
    (control / "start").write_text("selection is held")

    try:
        wait("other tool and reply update while native selection is held",
             lambda tree: text_has(TOOL_TITLE + " · Running")(tree) and text_has(STREAM_PREFIX + "0020:")(tree))
        observed["selection_during"] = call("rich-sample", pid, SELECTION_TEXT)
        wait("sustained streaming remains visible during selection",
             text_has(STREAM_PREFIX + "0040:"))
        observed["selection_after"] = call("rich-sample", pid, SELECTION_TEXT)
        save()
        held = [observed[key] for key in ("selection_before", "selection_during", "selection_after")]
        assert all(sample["selected_text"] == SELECTION_TEXT for sample in held), held
        assert all(not sample["pending_reply_visible"] for sample in held), held
        positions = [sample["anchor"]["rect"][1] for sample in held]
        assert max(positions) - min(positions) <= 1, positions
        assert stream_number(held[2]) > stream_number(held[1]) >= 20
        checks["native held selection preserves exact multilingual clipboard bytes"] = True
        checks["selected reply remains unchanged while other retained records update"] = True
        checks["selected reading position remains stable under sustained updates"] = True

        observed["latest_keyboard"] = call("rich-keyboard", pid, "Latest reply ↓", "activate")
        wait("native Latest clears selection and reveals the owner's pending reply revision",
             text_has(LATE_TEXT.strip()))
        observed["latest_before"] = call("rich-tail", pid)
        tail_number = int(re.search(r"Reading progress (\d+):", observed["latest_before"]["tail"]["AXValue"])[1]) + 20
        wait("native Latest resumes following new replies", text_has(f"{STREAM_PREFIX}{tail_number:04d}:"))
        observed["latest_after"] = call("rich-tail", pid)
        for sample in (observed["latest_before"], observed["latest_after"]):
            assert sample["selected_text"] == "", sample
            _, top, _, height = sample["viewport"]["rect"]
            _, y, _, text_height = sample["tail"]["rect"]
            assert top <= y <= y + text_height <= top + height, sample
        assert abs(observed["latest_before"]["tail"]["rect"][1] - observed["latest_after"]["tail"]["rect"][1]) <= 1, {"before": observed["latest_before"], "after": observed["latest_after"]}
        checks["native Latest clears held selection and keeps following later updates"] = True
        reading_anchor = "Reading section 12"
        call("rich-reveal", pid, reading_anchor)
        observed["reading_before"] = call("rich-sample", pid, reading_anchor)
        number = stream_number(observed["reading_before"]) + 20
        wait("stream continues while the reader stays above the newest reply",
             text_has(f"{STREAM_PREFIX}{number:04d}:"))
        observed["reading_after"] = call("rich-sample", pid, reading_anchor)
        save()
        before, after = observed["reading_before"], observed["reading_after"]
        assert after["selected_text"] == "", after["selected_text"]
        assert abs(before["anchor"]["rect"][1] - after["anchor"]["rect"][1]) <= 1
        assert stream_number(after) > stream_number(before)
        checks["native scroll reading position remains stable while replies grow"] = True
    finally:
        (control / "finish").write_text("native selection and reading observations complete")
        save()

    wait("rich stream settles with its final tool result",
         lambda tree: progress().get("stage") == "finished" and text_has(TOOL_TITLE + " · Failed")(tree))
    observed["stream"] = progress()
    canonical = source(link) + LATE_TEXT
    reply = call("rich-copy-reply", pid, SELECTION_TEXT)["text"]
    assert reply == canonical, (len(reply), len(canonical), reply[:200])
    checks["Copy reply preserves exact canonical Markdown source"] = True
    payload = lambda value: json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
    tool_source = (
        TOOL_TITLE + "\nStatus: Failed\n\nInput\n" +
        payload({"command": "cargo test --package reading-note", "directory": "."}) +
        "\n\nOutput\n" + payload({"exit_code": 1, "stderr": TOOL_ERROR}) +
        "\n\nResult\n" + TOOL_ERROR + "\nReview finished with an error." +
        "\n\nLocation\n" + str(directory / "sample.rs") + ":2"
    )
    tool_copy = call("copy-button", pid, "Copy tool details")["text"]
    assert tool_copy == tool_source, tool_copy
    checks["Copy tool details preserves final original fields"] = True
    streamed = "".join(f"{STREAM_PREFIX}{number:04d}: new result café 日本語 👩🏽‍💻.\n\n"
                       for number in range(1, observed["stream"]["chunks"] + 1))
    expected = "\n\n".join([baseline, "You\n" + PROMPT, "Command\n" + tool_source,
                            "Agent\n" + canonical,
                            "Command\n" + PROGRESS_TOOL_TITLE + "\nStatus: Finished",
                            "Agent\n" + streamed])
    conversation = call("copy-button", pid, "Copy conversation")["text"]
    assert conversation == expected, (len(conversation), len(expected), conversation[-200:])
    checks["Copy conversation preserves every exact source record once"] = True
    observed["canonical"] = {
        "reply_bytes": len(reply.encode()), "reply_sha256": hashlib.sha256(reply.encode()).hexdigest(),
        "conversation_bytes": len(conversation.encode()),
        "conversation_sha256": hashlib.sha256(conversation.encode()).hexdigest(),
    }
    tree = rows()
    assert text_has('<script>window.location = "https://example.invalid/never";</script>')(tree)
    assert not any(row.get("AXRole") == "AXLink" and
                   ("Unavailable action" in str(row) or "javascript:" in str(row)) for row in tree)
    assert not any(path.startswith("/rich-image/") for path in api.visits), api.visits
    checks["raw HTML and unsafe links remain text and remote images are not fetched"] = True
    call("rich-reveal", pid, SELECTION_TEXT)
    save()
    observed["link"] = call("rich-local-link", pid, LINK_LABEL, link)
    observed["link"]["request_observed"] = urlsplit(link).path in api.visits
    save()
    checks["rich reply link accepts native keyboard focus"] = True
    if observed["link"]["activated"]:
        assert urlsplit(link).path in api.visits, api.visits
        assert observed["link"]["closed_controlled_tab"]
        checks["native keyboard opens the controlled link and closes only its test tab"] = True
    else:
        print("UNAVAILABLE: native browser activation; " + observed["link"]["reason"], flush=True)

    for theme in ("Dark", "Light"):
        call("choose-option", pid, "", "Appearance", theme)
        call("rich-reveal", pid, "Release reading note")
        call("screenshot", pid, evidence / ("rich-" + theme.lower() + ".png"))
    call("resize", pid, 800, 680)
    call("rich-reveal", pid, "fn reading_note()")
    call("screenshot", pid, evidence / "rich-narrow.png")
    call("ax", pid, evidence / "rich-accessibility.json")
    call("resize", pid, window[2], window[3])
    save()
    results["rich_conversation"] = observed
    results["rich_observation_sha256"] = hashlib.sha256((evidence / "rich-observation.json").read_bytes()).hexdigest()
    print("PASS: native rich conversation selection, source copy, streaming and reading evidence", flush=True)
