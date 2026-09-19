// Start the roadmap loop from the production desktop window through macOS Accessibility.
// The provider is native_work.py's controlled HTTP peer; this is not live-model evidence.
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
func waitFor<T>(_ name: String, seconds: TimeInterval = 25, _ body: () -> T?) throws -> T {
    let deadline = Date().addingTimeInterval(seconds)
    repeat {
        if let value = body() { return value }
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    } while Date() < deadline
    throw Failure(description: "Timed out: \(name)")
}

func exercise() throws {
    try require(AXIsProcessTrusted(), "Accessibility permission is required")
    let previous = NSWorkspace.shared.frontmostApplication
    let process = Process()
    process.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
    process.arguments = ["--dir", CommandLine.arguments[2], "--provider", "ollama",
                         "--model", "desktop-test", "--reduce-motion"]
    process.standardOutput = FileHandle.standardOutput
    process.standardError = FileHandle.standardError
    try process.run()
    defer {
        if process.isRunning { process.terminate(); process.waitUntilExit() }
        previous?.activate()
    }
    let app = AXUIElementCreateApplication(process.processIdentifier)
    let window: AXUIElement = try waitFor("desktop window") {
        (attribute(app, kAXWindowsAttribute) as? [AXUIElement])?.first
    }
    func button(_ name: String) -> AXUIElement? {
        descendants(window).first {
            label($0) == name && attribute($0, kAXRoleAttribute) as? String == kAXButtonRole
        }
    }
    let draft = "Keep this draft"
    let editor: AXUIElement = try waitFor("message editor") {
        descendants(window).first {
            label($0) == "Message Magistr" && attribute($0, kAXRoleAttribute) as? String == kAXTextAreaRole
        }
    }
    try require(AXUIElementSetAttributeValue(editor, kAXValueAttribute as CFString,
                                            draft as CFString) == .success, "Set draft")
    _ = try waitFor("Start next item") { button("Start next item") }
    let start = try waitFor("Start loop") { button("Start loop") }
    try require(button("Abort") == nil, "the loop's verbs wait for a live loop")
    try require(AXUIElementPerformAction(start, kAXPressAction as CFString) == .success, "Start loop")
    _ = try waitFor("live loop verbs") { button("Abort") }
    try require(button("Start loop") == nil, "no second start beside a live loop")
    print("PASS: Start loop pressed through Accessibility; the loop's verbs replaced the starts")
    _ = try waitFor("the loop ended and the starts returned", seconds: 180) { button("Start loop") }
    _ = try waitFor("draft kept across the run") {
        descendants(window).contains {
            label($0) == "Message Magistr" && attribute($0, kAXValueAttribute) as? String == draft
        } ? true : nil
    }
    guard let close = attribute(window, kAXCloseButtonAttribute) else {
        throw Failure(description: "Missing window close control")
    }
    try require(AXUIElementPerformAction(unsafeBitCast(close, to: AXUIElement.self),
                                         kAXPressAction as CFString) == .success, "Close window")
    _ = try waitFor("settled desktop exit") { process.isRunning ? nil : true }
    try require(process.terminationStatus == 0, "Desktop exited \(process.terminationStatus)")
    print("PASS: the run ended, the conversation returned with its draft and the window closed")
}
do { try exercise() }
catch { fputs("\(error)\n", stderr); exit(1) }
