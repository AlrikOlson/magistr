// Switch projects inside the production desktop window through macOS Accessibility, the
// pointer and the keyboard. The provider is native_switch.py's controlled HTTP peer.
import AppKit
import ApplicationServices

struct Failure: Error, CustomStringConvertible { let description: String }
func require(_ value: Bool, _ message: String) throws {
    if !value { throw Failure(description: message) }
}
func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
    return value
}
func descendants(_ element: AXUIElement, _ depth: Int = 0) -> [AXUIElement] {
    guard depth < 20 else { return [] }
    return [element] + (attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [])
        .flatMap { descendants($0, depth + 1) }
}
func label(_ element: AXUIElement) -> String {
    (attribute(element, kAXTitleAttribute) as? String)
        ?? (attribute(element, kAXDescriptionAttribute) as? String) ?? ""
}
func waitFor<T>(_ name: String, seconds: TimeInterval = 30, _ body: () -> T?) throws -> T {
    let deadline = Date().addingTimeInterval(seconds)
    repeat {
        if let value = body() { return value }
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    } while Date() < deadline
    throw Failure(description: "Timed out: \(name)")
}

func desktop(_ dir: String) throws -> Process {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
    process.arguments = ["--dir", dir, "--provider", "ollama", "--model", "desktop-test",
                         "--reduce-motion"]
    process.standardOutput = FileHandle.standardOutput
    process.standardError = FileHandle.standardError
    try process.run()
    return process
}

func exercise() throws {
    try require(AXIsProcessTrusted(), "Accessibility permission is required")
    let previous = NSWorkspace.shared.frontmostApplication
    let pointer = CGEvent(source: nil)?.location
    let (first, second, held) = (CommandLine.arguments[2], CommandLine.arguments[3],
                                 CommandLine.arguments[4])
    // Another window already hosts the third project.
    let holder = try desktop(held)
    let process = try desktop(first)
    defer {
        for running in [process, holder] where running.isRunning {
            running.terminate(); running.waitUntilExit()
        }
        if let pointer { CGWarpMouseCursorPosition(pointer) }
        previous?.activate()
    }
    _ = try waitFor("the holding window") {
        (attribute(AXUIElementCreateApplication(holder.processIdentifier), kAXWindowsAttribute)
            as? [AXUIElement])?.first
    }
    let app = AXUIElementCreateApplication(process.processIdentifier)
    let window: AXUIElement = try waitFor("desktop window") {
        (attribute(app, kAXWindowsAttribute) as? [AXUIElement])?.first
    }
    func find(_ role: String, _ matches: @escaping (String) -> Bool, _ name: String) throws -> AXUIElement {
        try waitFor(name) {
            descendants(window).first {
                attribute($0, kAXRoleAttribute) as? String == role && matches(label($0))
            }
        }
    }
    func named(_ name: String, _ role: String) throws -> AXUIElement {
        try find(role, { $0 == name }, name)
    }
    func press(_ element: AXUIElement) throws {
        try require(AXUIElementPerformAction(element, kAXPressAction as CFString) == .success,
                    "Press \(label(element))")
    }
    func key(_ code: CGKeyCode) throws {
        for down in [true, false] {
            guard let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down) else {
                throw Failure(description: "Cannot create key event")
            }
            event.postToPid(process.processIdentifier)
        }
    }
    func front() throws {
        // Accessibility may bring an application forward whatever is active; a plain
        // activation request is declined while the person is using another application.
        _ = try waitFor("frontmost desktop") { () -> Bool? in
            AXUIElementSetAttributeValue(app, kAXFrontmostAttribute as CFString, kCFBooleanTrue)
            AXUIElementPerformAction(window, kAXRaiseAction as CFString)
            return NSWorkspace.shared.frontmostApplication?.processIdentifier
                == process.processIdentifier ? true : nil
        }
    }
    func click(_ element: AXUIElement) throws {
        try front()
        guard let position = attribute(element, kAXPositionAttribute),
              let size = attribute(element, kAXSizeAttribute) else {
            throw Failure(description: "Missing project control bounds")
        }
        var point = CGPoint.zero
        var extent = CGSize.zero
        try require(AXValueGetValue(unsafeBitCast(position, to: AXValue.self), .cgPoint, &point)
                    && AXValueGetValue(unsafeBitCast(size, to: AXValue.self), .cgSize, &extent),
                    "Invalid project control bounds")
        point.x += extent.width / 2
        point.y += extent.height / 2
        for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
            guard let event = CGEvent(mouseEventSource: nil, mouseType: type,
                                      mouseCursorPosition: point, mouseButton: .left) else {
                throw Failure(description: "Cannot create pointer event")
            }
            event.post(tap: .cghidEventTap)
        }
    }
    func home() throws {
        // On the home page `projects` is where the window already is, not a link.
        let link = descendants(window).first {
            attribute($0, kAXRoleAttribute) as? String == "AXLink" && label($0) == "projects"
        }
        if let link { try press(link) }
        _ = try find("AXLink", { $0.hasPrefix("Open project ") }, "the home page")
    }
    func draftIs(_ text: String, homeOf current: String? = nil) throws {
        // From the home page the way back to the conversation is the open project.
        if let current { try press(named("Open project " + current, "AXLink")) }
        try press(named("Conversation", kAXRadioButtonRole))
        let editor = try named("Message Magistr", kAXTextAreaRole)
        _ = try waitFor("draft \(text)") {
            attribute(editor, kAXValueAttribute) as? String == text ? true : nil
        }
    }
    let names = [first, second, held].map { URL(fileURLWithPath: $0).lastPathComponent }
    let draft = "Keep this draft"
    let editor = try named("Message Magistr", kAXTextAreaRole)
    try require(AXUIElementSetAttributeValue(editor, kAXValueAttribute as CFString,
                                            draft as CFString) == .success, "Set draft")

    // Refused: the held project says why on its control, and the first project stays.
    try home()
    try press(find("AXLink", { $0.hasPrefix("Open project \(names[2])") }, "the held project"))
    let refused = try find("AXLink", { $0.hasPrefix("Open project \(names[2]). ") }, "refusal in words")
    try require(label(refused).contains("Another Magistr session has this project open"),
                "Unexpected refusal: \(label(refused))")
    try require((attribute(window, kAXTitleAttribute) as? String)?.hasPrefix(names[0] + " ") == true,
                "the window still names the first project")
    try draftIs(draft, homeOf: names[0])
    print("PASS: a project another window holds was refused in words; the first project stayed open")

    // Switched, three ways, there and back and there again; the draft comes along.
    var target = 1
    for method in ["accessibility", "keyboard", "pointer"] {
        try home()
        let open = try named("Open project " + names[target], "AXLink")
        switch method {
        case "pointer": try click(open)
        case "keyboard":
            // Focus then Return, addressed to this process. The window need not be the
            // key window for either; the switch itself is the witness that both landed.
            try require(AXUIElementSetAttributeValue(open, kAXFocusedAttribute as CFString,
                                                     kCFBooleanTrue) == .success, "Focus project")
            RunLoop.current.run(until: Date().addingTimeInterval(0.3))
            try key(36)
        default: try press(open)
        }
        // The new project's roadmap is shown; its name leads the window's path.
        // The window itself names the project it has open.
        _ = try waitFor("the window moved to \(names[target])", seconds: 60) {
            (attribute(window, kAXTitleAttribute) as? String)?.hasPrefix(names[target] + " ") == true ? true : nil
        }
        try draftIs(draft)
        print("PASS: switched to \(names[target]) by \(method) with the draft kept")
        target = target == 1 ? 0 : 1
    }

    // The window is on the second project: its host answers.
    let message = "sent in the second project"
    let composer = try named("Message Magistr", kAXTextAreaRole)
    try require(AXUIElementSetAttributeValue(composer, kAXValueAttribute as CFString,
                                            message as CFString) == .success, "Set message")
    try press(named("Send", kAXButtonRole))
    _ = try waitFor("reply from the second project") {
        descendants(window).contains {
            (attribute($0, kAXValueAttribute) as? String)?.contains("reply from the second project") == true
        } ? true : nil
    }
    guard let close = attribute(window, kAXCloseButtonAttribute) else {
        throw Failure(description: "Missing window close control")
    }
    try require(AXUIElementPerformAction(unsafeBitCast(close, to: AXUIElement.self),
                                         kAXPressAction as CFString) == .success, "Close window")
    _ = try waitFor("settled desktop exit") { process.isRunning ? nil : true }
    try require(process.terminationStatus == 0, "Desktop exited \(process.terminationStatus)")
    holder.terminate()
    holder.waitUntilExit()
    print("PASS: the second project's host answered and the window closed cleanly")
}
do { try exercise() }
catch { fputs("\(error)\n", stderr); exit(1) }
