// Native controls for the opt-in live desktop journey (native_live.py).
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
    guard depth < 24 else { return [] }
    return [element] + (attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [])
        .flatMap { descendants($0, depth + 1) }
}
func label(_ element: AXUIElement) -> String {
    (attribute(element, kAXTitleAttribute) as? String)
        ?? (attribute(element, kAXDescriptionAttribute) as? String) ?? ""
}
func waitFor<T>(_ name: String, _ body: () -> T?) throws -> T {
    let deadline = Date().addingTimeInterval(45)
    repeat {
        if let value = body() { return value }
        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    } while Date() < deadline
    throw Failure(description: "Timed out: \(name)")
}
func main() throws {
    try require(AXIsProcessTrusted(), "Accessibility permission is required")
    let previous = NSWorkspace.shared.frontmostApplication
    let pointer = CGEvent(source: nil)?.location
    let process = Process()
    process.executableURL = URL(fileURLWithPath: CommandLine.arguments[1])
    process.arguments = ["--dir", CommandLine.arguments[2], "--provider", "codex",
                         "--model", CommandLine.arguments[3], "--trust", "cautious", "--reduce-motion"]
    process.standardOutput = FileHandle.standardError
    process.standardError = FileHandle.standardError
    try process.run()
    defer {
        if process.isRunning { process.terminate(); process.waitUntilExit() }
        if let pointer { CGWarpMouseCursorPosition(pointer) }
        previous?.activate()
    }
    let app = AXUIElementCreateApplication(process.processIdentifier)
    let window: AXUIElement = try waitFor("desktop window") {
        NSRunningApplication(processIdentifier: process.processIdentifier)?.activate()
        return (attribute(app, kAXWindowsAttribute) as? [AXUIElement])?.first
    }
    NSRunningApplication(processIdentifier: process.processIdentifier)?.activate()
    func elements() -> [AXUIElement] { descendants(window) }
    func find(_ name: String, _ role: String?, _ last: Bool = false) throws -> AXUIElement {
        try waitFor(name) {
            let matches = elements().filter { label($0) == name &&
                (role == nil || attribute($0, kAXRoleAttribute) as? String == role) }
            return last ? matches.last : matches.first
        }
    }
    func press(_ element: AXUIElement) throws {
        try require(AXUIElementPerformAction(element, kAXPressAction as CFString) == .success,
                    "Press \(label(element))")
    }
    func reply(_ value: [String: Any]) throws {
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data("\n".utf8))
    }
    try reply(["ready": true, "pid": process.processIdentifier])
    while let line = readLine() {
        do {
            let command = try JSONSerialization.jsonObject(with: Data(line.utf8)) as! [String: Any]
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
            let op = command["op"] as? String ?? ""
            let name = command["name"] as? String ?? ""
            let role = command["role"] as? String
            switch op {
            case "snapshot":
                let rows: [[String: Any]] = elements().map {
                    ["name": label($0), "role": attribute($0, kAXRoleAttribute) as? String ?? "",
                     "value": attribute($0, kAXValueAttribute) as? String ?? "",
                     "enabled": attribute($0, kAXEnabledAttribute) as? Bool ?? false]
                }
                try reply(["ok": true, "rows": rows])
                continue
            case "set":
                let element = try find(name, role)
                let value = command["value"] as? String ?? ""
                try require(AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString,
                                                         value as CFString) == .success, "Set \(name)")
                _ = try waitFor("value") {
                    attribute(element, kAXValueAttribute) as? String == value ? true : nil
                }
            case "press": try press(find(name, role))
            case "key":
                if !name.isEmpty {
                    let element = try find(name, role)
                    try require(AXUIElementSetAttributeValue(element,
                        kAXFocusedAttribute as CFString, kCFBooleanTrue) == .success, "Focus \(name)")
                    _ = try waitFor("keyboard focus") {
                        attribute(app, kAXFocusedUIElementAttribute).map { CFEqual($0, element) } == true ? true : nil
                    }
                }
                for down in [true, false] {
                    guard let event = CGEvent(keyboardEventSource: nil,
                        virtualKey: CGKeyCode(command["code"] as? Int ?? 36), keyDown: down) else {
                        throw Failure(description: "Cannot create key event")
                    }
                    event.flags = CGEventFlags(rawValue: command["flags"] as? UInt64 ?? 0)
                    event.postToPid(process.processIdentifier)
                }
            case "click":
                let element = try find(name, role, command["last"] as? Bool ?? false)
                NSRunningApplication(processIdentifier: process.processIdentifier)?.activate()
                _ = try waitFor("frontmost desktop") {
                    NSWorkspace.shared.frontmostApplication?.processIdentifier == process.processIdentifier ? true : nil
                }
                guard let position = attribute(element, kAXPositionAttribute),
                      let size = attribute(element, kAXSizeAttribute) else {
                    throw Failure(description: "Missing bounds on \(name)")
                }
                var point = CGPoint.zero
                var extent = CGSize.zero
                try require(AXValueGetValue(unsafeBitCast(position, to: AXValue.self), .cgPoint, &point)
                    && AXValueGetValue(unsafeBitCast(size, to: AXValue.self), .cgSize, &extent),
                    "Invalid bounds on \(name)")
                point.x += extent.width / 2
                point.y += extent.height / 2
                var hit: AXUIElement?
                try require(AXUIElementCopyElementAtPosition(app, Float(point.x), Float(point.y), &hit) == .success
                    && hit.map { CFEqual($0, element) } == true, "Hit target differs from \(name)")
                for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
                    try require(NSWorkspace.shared.frontmostApplication?.processIdentifier == process.processIdentifier,
                                "Desktop must stay frontmost for pointer input")
                    let event = CGEvent(mouseEventSource: nil, mouseType: type,
                                        mouseCursorPosition: point, mouseButton: .left)
                    event?.post(tap: .cghidEventTap)
                    RunLoop.current.run(until: Date().addingTimeInterval(0.05))
                }
            case "close":
                guard let close = attribute(window, kAXCloseButtonAttribute) else {
                    throw Failure(description: "Missing close control")
                }
                try press(unsafeBitCast(close, to: AXUIElement.self))
                _ = try waitFor("desktop exit") { process.isRunning ? nil : true }
                try require(process.terminationStatus == 0, "Desktop exited \(process.terminationStatus)")
                try reply(["ok": true])
                return
            default: throw Failure(description: "Unknown operation \(op)")
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.1))
            try reply(["ok": true])
        } catch { try reply(["ok": false, "error": String(describing: error)]) }
    }
}
do { try main() }
catch { fputs("\(error)\n", stderr); exit(1) }

