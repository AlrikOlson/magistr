// Real macOS file panels, clipboard images and file-url drags for disposable fixtures.
import AppKit
import ApplicationServices

@MainActor func attachmentPanel(_ pid: Int32) throws -> AXUIElement {
    guard let panel = nodes(AXUIElementCreateApplication(pid)).first(where: {
        string($0, "AXRole") == "AXSheet" ||
        (string($0, "AXRole") == "AXWindow" && string($0, "AXTitle") == "Add files")
    }) else { throw Failure.message("The native Add files panel is not open") }
    return panel
}

func attachmentFile(_ path: String) throws -> URL {
    let url = URL(fileURLWithPath: path).standardizedFileURL.resolvingSymlinksInPath()
    try require(url.pathComponents.contains(where: { $0.hasPrefix("magistr-desktop-journey-") }),
                "Native attachment input is restricted to disposable journey files")
    try require(FileManager.default.fileExists(atPath: url.path), "The fixture file is missing")
    return url
}

@MainActor final class AttachmentDragSource: NSImageView, NSDraggingSource {
    let file: URL
    let start: CGPoint
    let destination: CGPoint
    var receivedMouseDown = false
    var mouseDownWindow = 0
    var started = false
    var moved = 0
    var pasteboardTypes: [String] = []
    var pasteboardFileURL = ""
    var completed: NSDragOperation?
    var deadline: DispatchWorkItem?

    init(frame: NSRect, file: URL, start: CGPoint, destination: CGPoint) {
        self.file = file; self.start = start; self.destination = destination
        super.init(frame: frame)
        image = NSWorkspace.shared.icon(forFile: file.path)
        imageScaling = .scaleProportionallyUpOrDown
    }
    required init?(coder: NSCoder) { nil }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) {
        receivedMouseDown = true
        mouseDownWindow = event.windowNumber
        let item = NSDraggingItem(pasteboardWriter: file as NSURL)
        item.setDraggingFrame(bounds, contents: image)
        let session = beginDraggingSession(with: [item], event: event, source: self)
        session.animatesToStartingPositionsOnCancelOrFail = false
    }
    func draggingSession(_ session: NSDraggingSession,
                         sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation { .copy }
    func draggingSession(_ session: NSDraggingSession, willBeginAt point: NSPoint) {
        started = true
        pasteboardTypes = session.draggingPasteboard.types?.map { $0.rawValue } ?? []
        pasteboardFileURL = session.draggingPasteboard.string(forType: .fileURL) ?? ""
        let start = start, destination = destination
        // AppKit starts the real drag on a later run-loop turn, not at beginDraggingSession.
        DispatchQueue.global().async { attachmentDragEvents(from: start, to: destination) }
        let deadline = DispatchWorkItem { key(53) }
        self.deadline = deadline
        DispatchQueue.global().asyncAfter(deadline: .now() + 8, execute: deadline)
    }
    func draggingSession(_ session: NSDraggingSession, movedTo point: NSPoint) { moved += 1 }
    func draggingSession(_ session: NSDraggingSession, endedAt point: NSPoint,
                         operation: NSDragOperation) {
        completed = operation
        deadline?.cancel()
    }
    func ignoreModifierKeys(for session: NSDraggingSession) -> Bool { true }
}

func attachmentDragEvents(from start: CGPoint, to end: CGPoint) {
    let source = CGEventSource(stateID: .hidSystemState)
    for step in 1...20 {
        let fraction = Double(step) / 20
        let point = CGPoint(x: start.x + (end.x - start.x) * fraction,
                            y: start.y + (end.y - start.y) * fraction)
        CGEvent(mouseEventSource: source, mouseType: .leftMouseDragged,
                mouseCursorPosition: point, mouseButton: .left)?.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.025)
    }
    Thread.sleep(forTimeInterval: 0.25)
    CGEvent(mouseEventSource: source, mouseType: .leftMouseUp,
            mouseCursorPosition: end, mouseButton: .left)?.post(tap: .cghidEventTap)
}

@MainActor func showAttachmentSource(_ window: NSWindow, at point: NSPoint) throws {
    window.orderFrontRegardless()
    let deadline = Date(timeIntervalSinceNow: 3)
    var number = 0
    repeat {
        CFRunLoopRunInMode(.defaultMode, 0.05, true)
        window.displayIfNeeded()
        // This uses AppKit's mouse-down rules, including transparent/ignoring windows.
        // Enclosing WindowServer rectangles alone do not identify the input owner.
        number = NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: 0)
        if window.isVisible && number == window.windowNumber &&
           NSApp.window(withWindowNumber: number) === window { return }
    } while Date() < deadline
    throw Failure.message("Native drag source is not visible at its input point: window=\(number), expected=\(window.windowNumber), frame=\(window.frame), point=\(point)")
}

@MainActor func attachmentDrop(_ pid: Int32, _ path: String, _ input: AXUIElement) throws -> [String: Any] {
    let url = try attachmentFile(path)
    guard let destination = rectangle(input), let bounds = rectangle(try windowAX(pid)),
          let screen = NSScreen.screens.first else {
        throw Failure.message("The attachment drop target has no native bounds")
    }
    let end = CGPoint(x: destination.midX, y: destination.midY)
    let start = CGPoint(x: bounds.minX + 70, y: bounds.minY + 100)
    // This observer normally has no windows. Register its temporary drag source
    // with AppKit before asking WindowServer to deliver real mouse events to it.
    try require(NSApp.setActivationPolicy(.accessory), "The native drag source could not become an accessory app")
    NSApp.finishLaunching()
    let window = NSWindow(contentRect: NSRect(x: start.x - 32,
        y: screen.frame.maxY - start.y - 32, width: 64, height: 64),
        styleMask: [.borderless], backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    window.level = .floating
    let source = AttachmentDragSource(frame: NSRect(x: 0, y: 0, width: 64, height: 64),
                                      file: url, start: start, destination: end)
    window.contentView = source
    defer { window.close() }
    try showAttachmentSource(window, at: NSPoint(x: start.x, y: screen.frame.maxY - start.y))
    let original = CGEvent(source: nil)!.location
    defer {
        source.deadline?.cancel()
        if source.completed == nil {
            CGEvent(mouseEventSource: nil, mouseType: .leftMouseUp,
                    mouseCursorPosition: start, mouseButton: .left)?.post(tap: .cghidEventTap)
        }
        CGWarpMouseCursorPosition(original)
    }
    CGWarpMouseCursorPosition(start)
    CGEvent(mouseEventSource: CGEventSource(stateID: .hidSystemState),
            mouseType: .leftMouseDown, mouseCursorPosition: start,
            mouseButton: .left)?.post(tap: .cghidEventTap)
    let deadline = Date().addingTimeInterval(10)
    while source.completed == nil && Date() < deadline {
        if let event = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.02),
                                     inMode: .default, dequeue: true) { NSApp.sendEvent(event) }
        CFRunLoopRunInMode(.defaultMode, 0.02, true)
    }
    try require(source.receivedMouseDown && source.mouseDownWindow == window.windowNumber &&
                source.started && source.moved > 0 && source.completed == .copy,
                "Native file drag did not complete: mouse_down=\(source.receivedMouseDown), started=\(source.started), moved=\(source.moved), operation=\(source.completed?.rawValue ?? 0)")
    window.orderOut(nil)
    try waitForApplicationFocus(pid)
    return ["source": url.lastPathComponent, "operation": source.completed!.rawValue,
            "source_window": window.windowNumber, "source_window_hit_test": true,
            "pasteboard_types": source.pasteboardTypes, "pasteboard_file_url": source.pasteboardFileURL,
            "native_mouse_down": source.receivedMouseDown, "native_mouse_down_window": source.mouseDownWindow,
            "drag_started": source.started,
            "native_drag_moves": source.moved, "destination": row(input)]
}

@MainActor func attachmentOperation(_ mode: String, _ pid: Int32,
                                    _ args: [String]) async throws -> Bool {
    if mode == "attachment-file" {
        let url = try attachmentFile(args[2])
        let panel = try attachmentPanel(pid)
        let before = row(panel)
        let saved = clipboardSaved(); defer { restoreClipboard(saved) }
        key(5, [.maskCommand, .maskShift]); try await pause(0.25)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(url.path, forType: .string)
        key(9, .maskCommand); try await pause(0.25)
        key(36); try await pause(0.35)
        key(36); try await pause(0.3)
        try emit(["panel": before, "selected": url.lastPathComponent])
        return true
    }
    if mode == "attachment-cancel" {
        let panel = try attachmentPanel(pid)
        guard let button = nodes(panel).first(where: {
            string($0, "AXRole") == "AXButton" &&
            [string($0, "AXTitle"), string($0, "AXDescription")].contains("Cancel")
        }) else { throw Failure.message("The native file panel has no Cancel button") }
        let before = row(panel)
        try require(AXUIElementPerformAction(button, "AXPress" as CFString) == .success,
                    "Native file selection could not be cancelled")
        try await pause(0.2)
        try emit(["panel": before, "cancelled": true])
        return true
    }
    if mode == "attachment-paste-image" {
        let url = try attachmentFile(args[2]), data = try Data(contentsOf: url)
        let saved = clipboardSaved(); defer { restoreClipboard(saved) }
        NSPasteboard.general.clearContents()
        try require(NSPasteboard.general.setData(data, forType: .png), "PNG pasteboard write failed")
        key(9, .maskCommand); try await pause(0.4)
        try emit(["png_bytes": data.count, "types": NSPasteboard.general.types?.map { $0.rawValue } ?? []])
        return true
    }
    if mode == "attachment-drop" {
        let input = try await find(pid, "Message the agent")
        // Keep the native source lifetime synchronous: AppKit launch/run-loop control
        // must finish before Swift async-main is allowed to drain its outer loop.
        try emit(try attachmentDrop(pid, args[2], input))
        return true
    }
    return false
}
