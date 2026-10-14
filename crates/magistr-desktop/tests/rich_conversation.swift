// Native rich-text observations. No page JavaScript or application test commands.
import AppKit
import ApplicationServices

func matchingText(_ pid: Int32, _ text: String) throws -> AXUIElement {
    let candidates = nodes(AXUIElementCreateApplication(pid)).filter {
        string($0, "AXRole") == "AXStaticText" && string($0, "AXValue").contains(text)
    }
    guard let item = candidates.min(by: { string($0, "AXValue").count < string($1, "AXValue").count }) else {
        throw Failure.message("No native text containing " + text)
    }
    return item
}

func nativeWheel(_ point: CGPoint, _ vertical: Int32) {
    let original = CGEvent(source: nil)!.location
    defer { CGWarpMouseCursorPosition(original) }
    CGWarpMouseCursorPosition(point)
    CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: point, mouseButton: .left)?
        .post(tap: .cghidEventTap)
    CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: vertical, wheel2: 0, wheel3: 0)?
        .post(tap: .cghidEventTap)
}

@MainActor func revealText(_ pid: Int32, _ label: String) async throws -> AXUIElement {
    for _ in 0..<16 {
        let target = try matchingText(pid, label)
        let transcript = try await find(pid, "Messages")
        guard let targetRect = rectangle(target), let viewport = rectangle(transcript) else {
            throw Failure.message("Native text or Messages has no bounds")
        }
        let desired = viewport.minY + min(120, viewport.height / 3)
        let distance = desired - targetRect.minY
        if abs(distance) < 6 { return target }
        nativeWheel(CGPoint(x: viewport.midX, y: viewport.midY), Int32(max(-5000, min(5000, distance))))
        try await pause(0.12)
        if let moved = rectangle(try matchingText(pid, label)),
           moved.minY >= viewport.minY + 4 && moved.maxY <= viewport.maxY - 4 {
            return try matchingText(pid, label)
        }
    }
    throw Failure.message("Native scrolling did not reveal " + label)
}

func textBounds(_ element: AXUIElement, _ range: CFRange) -> CGRect? {
    var range = range, result: CFTypeRef?
    guard let parameter = AXValueCreate(.cfRange, &range),
          AXUIElementCopyParameterizedAttributeValue(element, "AXBoundsForRange" as CFString, parameter, &result) == .success,
          let result, CFGetTypeID(result) == AXValueGetTypeID() else { return nil }
    var rect = CGRect.zero
    guard AXValueGetValue(result as! AXValue, .cgRect, &rect) else { return nil }
    return rect
}

func nativeDrag(_ start: CGPoint, _ end: CGPoint) {
    let original = CGEvent(source: nil)!.location
    defer { CGWarpMouseCursorPosition(original) }
    let source = CGEventSource(stateID: .hidSystemState)
    CGWarpMouseCursorPosition(start)
    for (type, point) in [(CGEventType.mouseMoved, start), (.leftMouseDown, start)] {
        CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: point, mouseButton: .left)?
            .post(tap: .cghidEventTap)
    }
    for step in 1...12 {
        let part = Double(step) / 12
        let point = CGPoint(x: start.x + (end.x-start.x)*part, y: start.y + (end.y-start.y)*part)
        CGEvent(mouseEventSource: source, mouseType: .leftMouseDragged, mouseCursorPosition: point, mouseButton: .left)?
            .post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.015)
    }
    CGEvent(mouseEventSource: source, mouseType: .leftMouseUp, mouseCursorPosition: end, mouseButton: .left)?
        .post(tap: .cghidEventTap)
}

@MainActor func selectedClipboard() async throws -> String {
    let saved = clipboardSaved()
    defer { restoreClipboard(saved) }
    NSPasteboard.general.clearContents()
    key(8, .maskCommand)
    try await pause(0.15)
    return NSPasteboard.general.string(forType: .string) ?? ""
}

@MainActor func selectNativeText(_ pid: Int32, _ text: String) async throws {
    let element = try await revealText(pid, text)
    let value = string(element, "AXValue") as NSString
    let range = value.range(of: text)
    guard range.location != NSNotFound, let rect = rectangle(element) else {
        throw Failure.message("The selection phrase has no native bounds")
    }
    let first = textBounds(element, CFRange(location: range.location, length: 1))
    let last = textBounds(element, CFRange(location: range.location + range.length - 1, length: 1))
    if let first, let last, first.height > 0, last.height > 0 {
        nativeDrag(CGPoint(x: first.minX, y: first.midY), CGPoint(x: last.maxX, y: last.midY))
    } else {
        // The paragraph is one native text run; drag its visible first line.
        try require(value as String == text, "Native range bounds are unavailable for a partial paragraph")
        nativeDrag(CGPoint(x: rect.minX, y: rect.midY), CGPoint(x: rect.maxX, y: rect.midY))
    }
    try await pause(0.15)
    let copied = try await selectedClipboard()
    try require(copied == text, "Native selection differs: " + String(reflecting: copied))
}

@MainActor func readingSample(_ pid: Int32, _ anchor: String) async throws -> [String: Any] {
    let tree = nodes(AXUIElementCreateApplication(pid))
    let item = try matchingText(pid, anchor)
    return ["anchor": row(item),
            "selected_text": try await selectedClipboard(),
            "selections": tree.filter { !string($0, "AXSelectedText").isEmpty }.map { row($0) },
            "stream_values": tree.map { string($0, "AXValue") }.filter { $0.contains("Reading progress ") },
            "tool_values": tree.map { string($0, "AXValue") + string($0, "AXTitle") + string($0, "AXDescription") }
                .filter { $0.contains("Review local build") },
            "pending_reply_visible": tree.contains { string($0, "AXValue").contains("Selected reply received an update.") }]
}

func ancestor(_ element: AXUIElement, containing label: String) -> AXUIElement? {
    var candidate = element
    for _ in 0..<12 {
        if nodes(candidate).contains(where: { node in
            ["AXTitle", "AXDescription", "AXValue"].contains(where: { key in string(node, key) == label })
        }) { return candidate }
        guard let raw = attr(candidate, "AXParent"), CFGetTypeID(raw) == AXUIElementGetTypeID() else { return nil }
        candidate = raw as! AXUIElement
    }
    return nil
}

@MainActor func keyboardTarget(_ pid: Int32, _ label: String, activate: Bool) async throws -> [String: Any] {
    let target = try await find(pid, label)
    _ = AXUIElementPerformAction(target, "AXScrollToVisible" as CFString)
    try require(AXUIElementSetAttributeValue(target, "AXFocused" as CFString, kCFBooleanTrue) == .success,
                "Native keyboard target cannot receive focus: " + label)
    try await pause(0.1)
    let focused = attr(AXUIElementCreateApplication(pid), "AXFocusedUIElement") as! AXUIElement?
    try require(focused.map { CFEqual($0, target) } == true || attr(target, "AXFocused") as? Bool == true,
                "Native keyboard target did not receive focus: " + label)
    let before = row(target)
    if activate {
        key(string(target, "AXRole") == "AXLink" ? 36 : 49)
        try await pause(0.2)
    }
    return ["focused": before, "activation": activate ? "native keyboard" : "focus only"]
}

func newestStreamLeaf(_ root: AXUIElement, remaining: inout Int, depth: Int = 0) -> AXUIElement? {
    guard remaining > 0, depth < 16 else { return nil }
    remaining -= 1
    if string(root, "AXRole") == "AXStaticText", string(root, "AXValue").hasPrefix("Reading progress ") {
        return root
    }
    for child in (attr(root, "AXChildren") as? [AXUIElement] ?? []).reversed() {
        if let leaf = newestStreamLeaf(child, remaining: &remaining, depth: depth + 1) { return leaf }
    }
    return nil
}

@MainActor func newestStreamSample(_ pid: Int32) async throws -> [String: Any] {
    let transcript = try await find(pid, "Messages")
    let selected = try await selectedClipboard()
    // Traverse only the newest article/body/leaf. The full tree can span several stream revisions.
    for attempt in 1...20 {
        var remaining = 128
        guard let tail = newestStreamLeaf(transcript, remaining: &remaining) else {
            throw Failure.message("No native streaming tail within the bounded Messages traversal")
        }
        let value = string(tail, "AXValue"), sampled = row(tail), viewport = row(transcript)
        remaining = 128
        if let current = newestStreamLeaf(transcript, remaining: &remaining),
           CFEqual(tail, current), string(current, "AXValue") == value {
            return ["tail": sampled, "viewport": viewport, "selected_text": selected,
                    "coherent_sample_attempts": attempt]
        }
        try await pause(0.01)
    }
    throw Failure.message("The native streaming tail changed throughout its geometry sample")
}

@MainActor func controlledBrowserAddress(_ browser: NSRunningApplication, _ destination: String) async throws -> [String: Bool] {
    let browserPID = browser.processIdentifier
    guard NSWorkspace.shared.frontmostApplication?.processIdentifier == browserPID else {
        return ["readable": false, "matches": false]
    }
    let application = AXUIElementCreateApplication(browserPID), saved = clipboardSaved()
    let focused = attr(application, "AXFocusedUIElement") as! AXUIElement?
    let selection = focused.flatMap { attr($0, "AXSelectedTextRange") }
    defer { restoreClipboard(saved) }
    NSPasteboard.general.clearContents()
    key(37, .maskCommand)
    try await pause(0.15)
    key(8, .maskCommand)
    try await pause(0.15)
    let value = NSPasteboard.general.string(forType: .string)
    let stillFront = NSWorkspace.shared.frontmostApplication?.processIdentifier == browserPID
    // Never retain or expose another tab's address. Escape and restore its prior focus/selection.
    if stillFront {
        key(53)
        try await pause(0.05)
        if let focused {
            _ = AXUIElementSetAttributeValue(focused, "AXFocused" as CFString, kCFBooleanTrue)
            if let selection { _ = AXUIElementSetAttributeValue(focused, "AXSelectedTextRange" as CFString, selection) }
        }
    }
    return ["readable": stillFront && value?.isEmpty == false,
            "matches": stillFront && value == destination]
}

@MainActor func localLink(_ pid: Int32, _ label: String, _ destination: String) async throws -> [String: Any] {
    guard let url = URL(string: destination), url.scheme == "http", url.host == "127.0.0.1",
          url.path.hasPrefix("/rich-link/"), let handler = NSWorkspace.shared.urlForApplication(toOpen: url),
          let identity = Bundle(url: handler)?.bundleIdentifier else {
        throw Failure.message("Only the controlled localhost link can be activated")
    }
    let supported = ["com.apple.Safari", "com.google.Chrome", "com.google.Chrome.canary", "org.mozilla.firefox"]
    if !supported.contains(identity) {
        var result = try await keyboardTarget(pid, label, activate: false)
        result["reason"] = "The default browser has no verified native cleanup path: " + identity
        result["activated"] = false
        return result
    }
    let keyboard = try await keyboardTarget(pid, label, activate: true)
    var browser: NSRunningApplication?
    for _ in 0..<50 {
        if let front = NSWorkspace.shared.frontmostApplication, front.bundleIdentifier == identity {
            browser = front
            break
        }
        try await pause(0.1)
    }
    var result: [String: Any] = ["activated": false, "keyboard": keyboard, "browser": identity,
                                "activation_sent": true, "closed_controlled_tab": false]
    guard let browser else {
        result["reason"] = "The default browser did not become foreground after native link activation"
        try await focus(pid)
        return result
    }
    try await pause(0.6)
    let marker = "Native link reached: " + url.lastPathComponent
    let marked = nodes(AXUIElementCreateApplication(browser.processIdentifier)).contains { element in
        ["AXTitle", "AXValue", "AXDescription"].contains { string(element, $0).contains(marker) }
    }
    let address = try await controlledBrowserAddress(browser, destination)
    result["destination_observation"] = address
    result["controlled_marker_visible"] = marked
    guard address["matches"] == true else {
        result["reason"] = "The browser's active address could not be verified as the controlled destination"
        try await focus(pid)
        return result
    }
    // An exact address match is required immediately before this unique test tab may close.
    try require(NSWorkspace.shared.frontmostApplication?.processIdentifier == browser.processIdentifier,
                "The controlled browser tab lost focus before cleanup")
    key(13, .maskCommand)
    try await pause(0.3)
    let after = try await controlledBrowserAddress(browser, destination)
    try await focus(pid)
    try require(after["matches"] != true, "The controlled browser tab remained active after native close")
    result["activated"] = true
    result["closed_controlled_tab"] = true
    result["after_close_observation"] = after
    return result
}

@MainActor func richOperation(_ mode: String, _ pid: Int32, _ args: [String]) async throws -> Bool {
    switch mode {
    case "rich-reveal":
        try emit(row(try await revealText(pid, args[2])))
    case "rich-select":
        try await selectNativeText(pid, args[2])
        try emit(try await readingSample(pid, args[2]))
    case "rich-sample":
        try emit(try await readingSample(pid, args[2]))
    case "rich-tail":
        try emit(try await newestStreamSample(pid))
    case "rich-keyboard":
        try emit(try await keyboardTarget(pid, args[2], activate: args.count > 3 && args[3] == "activate"))
    case "rich-local-link":
        try emit(try await localLink(pid, args[2], args[3]))
    case "rich-copy-reply":
        let text = try matchingText(pid, args[2])
        guard let article = ancestor(text, containing: "Copy reply"),
              let button = nodes(article).first(where: { node in
                  string(node, "AXRole") == "AXButton" &&
                  ["AXTitle", "AXDescription"].contains(where: { key in string(node, key) == "Copy reply" })
              }) else { throw Failure.message("No Copy reply belongs to the requested text") }
        let saved = clipboardSaved()
        defer { restoreClipboard(saved) }
        NSPasteboard.general.clearContents()
        try require(AXUIElementPerformAction(button, "AXPress" as CFString) == .success, "Copy reply was not activated")
        try await pause(0.25)
        try emit(["text": NSPasteboard.general.string(forType: .string) ?? ""])
    default:
        return false
    }
    return true
}
