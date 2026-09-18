// Real macOS accessibility bridge checks; no screen reader or screen capture.
// Build navigation_probe and composer_probe, then pass their debug/examples directory.
import AppKit
import ApplicationServices

struct ProbeFailure: Error, CustomStringConvertible {
    let description: String
}
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw ProbeFailure(description: message) }
}
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else {
        return nil
    }
    return value
}
func label(_ element: AXUIElement) -> String {
    (attribute(element, kAXTitleAttribute) as? String)
        ?? (attribute(element, kAXDescriptionAttribute) as? String) ?? ""
}
func descendants(_ element: AXUIElement, depth: Int = 0) -> [AXUIElement] {
    guard depth < 20 else { return [] }
    return [element] + (attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [])
        .flatMap { descendants($0, depth: depth + 1) }
}
func waitFor<T>(_ description: String, _ body: () -> T?) throws -> T {
    let deadline = Date().addingTimeInterval(8)
    repeat {
        if let result = body() { return result }
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    } while Date() < deadline
    throw ProbeFailure(description: "Timed out: \(description)")
}

enum Drive: String {
    case accessibility, keyboard, pointer
}

final class WindowProbe {
    let process = Process()
    var application: AXUIElement { AXUIElementCreateApplication(process.processIdentifier) }

    init(_ binary: URL) throws {
        process.executableURL = binary
        try process.run()
        do {
            _ = try waitFor("window for \(binary.lastPathComponent)") { self.window }
            NSRunningApplication(processIdentifier: process.processIdentifier)?.activate()
        } catch {
            if process.isRunning { process.terminate() }
            throw error
        }
    }
    deinit {
        // Only the fixture started by this probe is terminated on failure.
        if process.isRunning { process.terminate() }
    }
    var window: AXUIElement? {
        (attribute(application, kAXWindowsAttribute) as? [AXUIElement])?.first
    }
    var elements: [AXUIElement] {
        window.map { descendants($0) } ?? []
    }
    func find(_ name: String, role: String? = nil) throws -> AXUIElement {
        try waitFor("control \(name), role \(role ?? "any")") {
            self.elements.first {
                label($0) == name && (role == nil || attribute($0, kAXRoleAttribute) as? String == role)
            }
        }
    }
    func set(_ element: AXUIElement, _ name: String, _ value: CFTypeRef) throws {
        let result = AXUIElementSetAttributeValue(element, name as CFString, value)
        try require(result == .success, "Set \(name) on \(label(element)): \(result)")
    }
    func press(_ element: AXUIElement) throws {
        let result = AXUIElementPerformAction(element, kAXPressAction as CFString)
        try require(result == .success, "Press \(label(element)): \(result)")
    }
    func value(_ element: AXUIElement, equals expected: String) throws {
        do {
            _ = try waitFor("value \(expected.debugDescription)") {
                attribute(element, kAXValueAttribute) as? String == expected ? true : nil
            }
        } catch {
            let actual = (attribute(element, kAXValueAttribute) as? String)?.debugDescription ?? "unavailable"
            throw ProbeFailure(description: "\(error); actual: \(actual)")
        }
    }
    func focus(_ element: AXUIElement) throws {
        try set(element, kAXFocusedAttribute, kCFBooleanTrue)
        try focused(element)
    }
    func focused(_ element: AXUIElement) throws {
        _ = try waitFor("focus on \(label(element))") {
            guard let current = attribute(self.application, kAXFocusedUIElementAttribute) else {
                return nil as Bool?
            }
            return CFEqual(current, element) ? true : nil
        }
    }
    func key(_ code: CGKeyCode, flags: CGEventFlags = []) throws {
        for down in [true, false] {
            guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
                throw ProbeFailure(description: "Cannot create keyboard event")
            }
            event.flags = flags
            event.postToPid(process.processIdentifier)
        }
    }
    func result(_ name: String) throws -> AXUIElement {
        try waitFor("result containing \(name)") {
            self.elements.first {
                label($0).contains(name) && attribute($0, kAXRoleAttribute) as? String == kAXButtonRole
            }
        }
    }
    func command(_ name: String, via: Drive = .accessibility) throws {
        if via != .accessibility {
            try key(40, flags: .maskCommand) // Cmd-K.
        } else {
            try press(find("Commands", role: kAXButtonRole))
        }
        let query = try find("Commands", role: kAXTextFieldRole)
        try set(query, kAXValueAttribute, name as CFString)
        try value(query, equals: name)
        let row = try result(name)
        switch via {
        case .keyboard:
            try focus(query)
            try key(48) // Tab moves focus from the field to its result.
            try focused(row)
            try key(36) // Return activates that focused result.
        case .pointer:
            try click(row)
        case .accessibility:
            try press(row)
        }
        _ = try waitFor("closed command palette") {
            self.elements.contains {
                label($0) == "Commands" && attribute($0, kAXRoleAttribute) as? String == kAXTextFieldRole
            } ? nil : true
        }
        print("PASS: \(via) opened \(name)")
    }
    func click(_ element: AXUIElement) throws {
        NSRunningApplication(processIdentifier: process.processIdentifier)?.activate()
        _ = try waitFor("frontmost fixture") {
            NSWorkspace.shared.frontmostApplication?.processIdentifier == self.process.processIdentifier ? true : nil
        }
        guard let position = attribute(element, kAXPositionAttribute),
              let size = attribute(element, kAXSizeAttribute) else {
            throw ProbeFailure(description: "Missing bounds on \(label(element))")
        }
        var origin = CGPoint.zero
        var extent = CGSize.zero
        try require(AXValueGetValue(unsafeBitCast(position, to: AXValue.self), .cgPoint, &origin),
                    "Invalid position")
        try require(AXValueGetValue(unsafeBitCast(size, to: AXValue.self), .cgSize, &extent),
                    "Invalid size")
        try require(extent.width > 0 && extent.height > 0, "Empty control bounds")
        let point = CGPoint(x: origin.x + extent.width / 2, y: origin.y + extent.height / 2)
        var hit: AXUIElement?
        let status = AXUIElementCopyElementAtPosition(application, Float(point.x), Float(point.y), &hit)
        try require(status == .success && hit.map { CFEqual($0, element) } == true,
                    "Accessibility hit does not match \(label(element))")
        for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
            guard let event = CGEvent(mouseEventSource: nil, mouseType: type,
                                      mouseCursorPosition: point, mouseButton: .left) else {
                throw ProbeFailure(description: "Cannot create pointer event")
            }
            try require(NSWorkspace.shared.frontmostApplication?.processIdentifier == process.processIdentifier,
                        "Fixture must be frontmost for pointer delivery")
            event.flags = []
            event.post(tap: .cghidEventTap)
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        }
    }
    func resize() throws {
        let window = try waitFor("window") { self.window }
        var requested = CGSize(width: 1100, height: 720)
        guard let value = AXValueCreate(.cgSize, &requested) else {
            throw ProbeFailure(description: "Cannot create window size")
        }
        try set(window, kAXSizeAttribute, value)
        _ = try waitFor("resized window") {
            guard let value = attribute(window, kAXSizeAttribute) else { return nil as Bool? }
            var actual = CGSize.zero
            guard AXValueGetValue(unsafeBitCast(value, to: AXValue.self), .cgSize, &actual) else {
                return nil
            }
            return actual == requested ? true : nil
        }
    }
    func close() throws {
        let window = try waitFor("window") { self.window }
        guard let button = attribute(window, kAXCloseButtonAttribute) else {
            throw ProbeFailure(description: "Window has no close button")
        }
        try press(unsafeBitCast(button, to: AXUIElement.self))
        _ = try waitFor("fixture exit") { self.process.isRunning ? nil : true }
        try require(process.terminationStatus == 0, "Fixture assertions failed: \(process.terminationStatus)")
    }
}

do {
    try require(AXIsProcessTrusted(), "Grant Accessibility access to the terminal running this check")
    print("Native accessibility check: \(ProcessInfo.processInfo.operatingSystemVersionString)")
    let previousApplication = NSWorkspace.shared.frontmostApplication
    let previousPointer = CGEvent(source: nil)?.location
    defer {
        if let position = previousPointer { CGWarpMouseCursorPosition(position) }
        previousApplication?.activate()
    }
    let directory = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first
        ?? "crates/magistr-gui2/target/debug/examples")
    let probe = try WindowProbe(directory.appendingPathComponent("navigation_probe"))
    try probe.command("Retry delivery")
    try probe.resize()
    try probe.command("Recorded work", via: .keyboard)
    try probe.command("Conversation", via: .pointer)
    try probe.close()
    let composer = try WindowProbe(directory.appendingPathComponent("composer_probe"))
    let editor = try composer.find("Message Magistr", role: kAXTextAreaRole)
    try composer.set(editor, kAXValueAttribute, "Please review" as CFString)
    try composer.value(editor, equals: "Please review")
    try composer.focus(editor)
    try composer.key(36, flags: .maskShift) // Shift-Return inserts a newline.
    try composer.value(editor, equals: "Please review\n")
    try composer.set(editor, kAXValueAttribute, "Please review\nthis file" as CFString)
    try composer.value(editor, equals: "Please review\nthis file")
    try composer.press(composer.find("Add files", role: kAXButtonRole))
    try composer.press(composer.find("résumé draft.md", role: kAXGroupRole))
    let draft: String = try waitFor("attached file in the draft") {
        guard let text = attribute(editor, kAXValueAttribute) as? String,
              text.contains("@\"résumé draft.md\"") else { return nil }
        return text
    }
    try composer.command("Roadmap")
    try composer.command("Conversation")
    let retained = try composer.find("Message Magistr", role: kAXTextAreaRole)
    try composer.value(retained, equals: draft)
    let send = try composer.find("Send", role: kAXButtonRole)
    var actions: CFArray?
    try require(AXUIElementCopyActionNames(send, &actions) == .success,
                "Send must publish its actions")
    let advertised = actions as? [String] ?? []
    try require(advertised.contains(kAXPressAction) && !advertised.contains(kAXIncrementAction),
                "Send must advertise only supported actions")
    // macOS may acknowledge an unsupported selector; the fixture's exact
    // command-count assertions below prove that it did not submit anything.
    _ = AXUIElementPerformAction(send, kAXIncrementAction as CFString)
    try composer.press(send)
    let stop = try composer.find("Stop", role: kAXButtonRole)
    let pending = try composer.find("Message Magistr", role: kAXTextAreaRole)
    try composer.set(pending, kAXValueAttribute, "Keep this draft" as CFString)
    try composer.value(pending, equals: "Keep this draft")
    try composer.press(stop)
    _ = try composer.find("Send", role: kAXButtonRole)
    try composer.value(pending, equals: "Keep this draft")
    try composer.close()
    print("PASS: native accessibility actions, keyboard focus/editing, pointer bounds, attachments and Send/Stop")
} catch {
    fputs("Native accessibility check failed: \(error)\n", stderr)
    exit(1)
}
