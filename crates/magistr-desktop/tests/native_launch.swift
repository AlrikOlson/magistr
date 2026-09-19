// Drive the production desktop process through macOS Accessibility.
// The HTTP peer is controlled by native_launch.py; this is not live-model evidence.
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
func waitFor<T>(_ name: String, _ body: () -> T?) throws -> T {
    let deadline = Date().addingTimeInterval(25)
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
    func find(_ name: String, _ role: String) throws -> AXUIElement {
        try waitFor(name) {
            descendants(window).first {
                label($0) == name && attribute($0, kAXRoleAttribute) as? String == role
            }
        }
    }
    func set(_ element: AXUIElement, _ text: String) throws {
        try require(AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString,
                                                text as CFString) == .success, "Set message")
        _ = try waitFor("message value") {
            attribute(element, kAXValueAttribute) as? String == text ? true : nil
        }
    }
    let editor = try find("Message Magistr", kAXTextAreaRole)
    try set(editor, "desktop native request")
    let send = try find("Send", kAXButtonRole)
    try require(AXUIElementPerformAction(send, kAXPressAction as CFString) == .success, "Send")
    _ = try waitFor("provider reply") {
        descendants(window).contains {
            (attribute($0, kAXValueAttribute) as? String)?.contains("desktop native response") == true
        } ? true : nil
    }
    try set(editor, "disconnect now")
    try require(AXUIElementPerformAction(try find("Send", kAXButtonRole),
                                         kAXPressAction as CFString) == .success, "Fail provider")
    _ = try waitFor("provider failure") {
        descendants(window).contains {
            (attribute($0, kAXValueAttribute) as? String)?.contains("503") == true
        } ? true : nil
    }
    let retry = "Keep this reconnect request"
    try set(editor, retry)
    try require(AXUIElementPerformAction(try find("Send", kAXButtonRole),
                                         kAXPressAction as CFString) == .success, "Retry")
    _ = try waitFor("unsupported restoration") {
        descendants(window).contains {
            (attribute($0, kAXValueAttribute) as? String)?.contains("does not support session resume") == true
        } ? true : nil
    }
    _ = try waitFor("restored request") {
        attribute(editor, kAXValueAttribute) as? String == retry ? true : nil
    }
    guard let close = attribute(window, kAXCloseButtonAttribute) else {
        throw Failure(description: "Missing window close control")
    }
    try require(AXUIElementPerformAction(unsafeBitCast(close, to: AXUIElement.self),
                                         kAXPressAction as CFString) == .success, "Close window")
    _ = try waitFor("settled desktop exit") { process.isRunning ? nil : true }
    try require(process.terminationStatus == 0, "Desktop exited \(process.terminationStatus)")
    print("PASS: native accessibility read a reply and provider failure, retried, retained the refused request and closed")
}
do { try exercise() }
catch { fputs("\(error)\n", stderr); exit(1) }

