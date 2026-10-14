// Rapid native edits for draft durability checks in disposable journey apps.
import AppKit
import ApplicationServices
import QuartzCore

// Each key event owns its UTF-16 payload. Queued input never reads a later clipboard value.
func typeDraftUnicode(_ value: String) throws {
    let source = CGEventSource(stateID: .hidSystemState)
    var chunks: [[UniChar]] = [], chunk: [UniChar] = []
    // Keep each native text event small without splitting a surrogate pair.
    for scalar in value.unicodeScalars {
        let units = Array(String(scalar).utf16)
        if chunk.count + units.count > 16 { chunks.append(chunk); chunk = [] }
        chunk.append(contentsOf: units)
    }
    if !chunk.isEmpty { chunks.append(chunk) }
    for units in chunks {
        for down in [true, false] {
            guard let event = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: down)
            else { throw Failure.message("A native draft text event could not be created") }
            event.flags = []
            event.setIntegerValueField(.keyboardEventAutorepeat, value: 0)
            units.withUnsafeBufferPointer {
                event.keyboardSetUnicodeString(stringLength: $0.count, unicodeString: $0.baseAddress)
            }
            event.post(tap: .cghidEventTap)
        }
    }
}

@MainActor func draftOperation(_ mode: String, _ pid: Int32,
                               _ args: [String]) async throws -> Bool {
    if mode == "draft-preferences" {
        guard let checkbox = nodes(AXUIElementCreateApplication(pid)).first(where: {
            string($0, "AXRole") == "AXCheckBox" &&
            [string($0, "AXTitle"), string($0, "AXDescription")].contains("Reduce motion")
        }) else { throw Failure.message("Reduce motion must be a native accessible checkbox") }
        guard let value = attr(checkbox, "AXValue") as? NSNumber else {
            throw Failure.message("The motion preference has no native checked value")
        }
        try emit(["reduce_motion": value.boolValue, "control": row(checkbox)])
        return true
    }
    if mode == "draft-last-edit-quit" {
        try require(args.count == 3 && args[2].utf8.count <= 4096,
                    "A bounded final draft is required")
        let input = try await find(pid, "Message the agent")
        guard let bounds = rectangle(input) else {
            throw Failure.message("The draft editor has no native bounds")
        }
        click(CGPoint(x: bounds.midX, y: bounds.midY))
        let saved = clipboardSaved(); defer { restoreClipboard(saved) }
        NSPasteboard.general.clearContents()
        try require(NSPasteboard.general.setString(args[2], forType: .string),
                    "The final draft could not reach the native clipboard")
        key(0, .maskCommand)
        let started = CACurrentMediaTime()
        key(9, .maskCommand)
        try await pause(0.025)
        key(12, .maskCommand)
        let elapsed = (CACurrentMediaTime() - started) * 1000
        try await pause(0.2)
        try emit(["paste_to_quit_ms": elapsed, "expected_text": args[2]])
        return true
    }
    guard mode == "draft-burst" else { return false }
    guard args.count == 3, let data = args[2].data(using: .utf8),
          let values = try JSONSerialization.jsonObject(with: data) as? [String],
          !values.isEmpty, values.count <= 100, values.allSatisfy({ $0.utf8.count <= 4096 })
    else { throw Failure.message("A bounded list of draft revisions is required") }
    let input = try await find(pid, "Message the agent")
    guard let bounds = rectangle(input) else {
        throw Failure.message("The draft editor has no native bounds")
    }
    click(CGPoint(x: bounds.midX, y: bounds.midY))
    let started = CACurrentMediaTime()
    var observations: [[String: Any]] = []
    for value in values {
        key(0, .maskCommand)
        if value.isEmpty { key(51) } else { try typeDraftUnicode(value) }
        try await pause(0.025)
        observations.append(["expected": value, "observed": string(input, "AXValue"),
                             "elapsed_ms": (CACurrentMediaTime() - started) * 1000])
    }
    for _ in 0..<80 {
        if string(input, "AXValue") == values.last! { break }
        try await pause(0.025)
    }
    try require(string(input, "AXValue") == values.last!,
                "The native editor did not receive the final rapid revision")
    try emit(["input": "native_unicode_events", "revisions": observations, "final": row(input),
              "elapsed_ms": (CACurrentMediaTime() - started) * 1000])
    return true
}
