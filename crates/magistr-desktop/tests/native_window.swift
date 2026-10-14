// Native input and accessibility checks are restricted to disposable desktop journey apps.
import AppKit
import ApplicationServices
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import QuartzCore
import Carbon

enum Failure: Error { case message(String) }
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw Failure.message(message) }
}
func emit(_ value: Any) throws {
    let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    FileHandle.standardOutput.write(data); FileHandle.standardOutput.write(Data([10]))
}
func save(_ value: Any, _ path: String) throws {
    try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
        .write(to: URL(fileURLWithPath: path))
}
func attr(_ element: AXUIElement, _ key: String) -> CFTypeRef? {
    var result: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, key as CFString, &result) == .success else { return nil }
    return result
}
func string(_ element: AXUIElement, _ key: String) -> String { attr(element,key) as? String ?? "" }
func children(_ element: AXUIElement) -> [AXUIElement] {
    let items = (attr(element,"AXChildren") as? [AXUIElement] ?? []) + (attr(element,"AXWindows") as? [AXUIElement] ?? [])
    // App menus include global recent documents; window evidence does not need them.
    return items.filter { string($0,"AXRole") != "AXMenuBar" }
}
func nodes(_ root: AXUIElement) -> [AXUIElement] {
    var result: [AXUIElement] = [], queue = [root]
    // Modal focus can be exposed separately from the window's child list.
    if string(root,"AXRole") == "AXApplication", let raw=attr(root,"AXFocusedUIElement"),
       CFGetTypeID(raw) == AXUIElementGetTypeID() {
        var focus=raw as! AXUIElement
        for _ in 0..<20 {
            queue.append(focus)
            if string(focus,"AXRole") == "AXApplication" {break}
            guard let parent=attr(focus,"AXParent"), CFGetTypeID(parent) == AXUIElementGetTypeID() else {break}
            focus=parent as! AXUIElement
        }
    }
    var seen = Set<AXUIElement>()
    while !queue.isEmpty && result.count < 10_000 {
        let node = queue.removeFirst()
        if !seen.insert(node).inserted { continue }
        result.append(node); queue.append(contentsOf: children(node))
    }
    return result
}
func rectangle(_ element: AXUIElement) -> CGRect? {
    guard let rawPosition = attr(element,"AXPosition"), let rawSize = attr(element,"AXSize"),
          CFGetTypeID(rawPosition) == AXValueGetTypeID(), CFGetTypeID(rawSize) == AXValueGetTypeID() else { return nil }
    var position = CGPoint.zero, size = CGSize.zero
    guard AXValueGetValue(rawPosition as! AXValue, .cgPoint, &position),
          AXValueGetValue(rawSize as! AXValue, .cgSize, &size) else { return nil }
    return CGRect(origin: position, size: size)
}
func row(_ element: AXUIElement) -> [String:Any] {
    var result: [String:Any] = [:]
    for key in ["AXRole","AXSubrole","AXTitle","AXDescription","AXValue","AXHelp","AXSelectedText"] {
        if let value = attr(element,key) as? String { result[key] = value }
    }
    if let rect = rectangle(element) { result["rect"] = [rect.minX,rect.minY,rect.width,rect.height] }
    if let focused = attr(element,"AXFocused") as? Bool { result["focused"] = focused }
    if let enabled = attr(element,"AXEnabled") as? Bool { result["enabled"] = enabled }
    var actions: CFArray?
    if AXUIElementCopyActionNames(element,&actions) == .success { result["actions"] = actions as? [String] ?? [] }
    return result
}
func app(_ pid: Int32) throws -> NSRunningApplication {
    guard let application = NSRunningApplication(processIdentifier: pid) else {
        throw Failure.message("The disposable desktop app is not running")
    }
    let journey = application.bundleIdentifier?.hasPrefix("dev.magistr.desktop.journey.") == true
    // Installed-app checks keep the signed production bundle intact. Only the
    // exact disposable copy explicitly supplied by that harness accepts input.
    let installedCopy = ProcessInfo.processInfo.environment["MAGISTR_OBSERVER_APP"].map { path in
        application.bundleIdentifier == "dev.magistr.desktop" &&
        application.bundleURL?.resolvingSymlinksInPath() == URL(fileURLWithPath: path).resolvingSymlinksInPath()
    } ?? false
    try require(journey || installedCopy, "Input is restricted to disposable desktop test apps")
    return application
}
@MainActor func find(_ pid: Int32, _ label: String) async throws -> AXUIElement {
    for _ in 0..<100 {
    let matches=nodes(AXUIElementCreateApplication(pid)).filter { element in
        ["AXDescription","AXTitle","AXValue"].contains(where: {string(element,$0) == label})
    }
    let priority=["AXTextArea","AXTextField","AXButton","AXPopUpButton","AXComboBox","AXImage","AXLink"]
    if let element=matches.sorted(by:{
        (priority.firstIndex(of:string($0,"AXRole")) ?? 10) <
        (priority.firstIndex(of:string($1,"AXRole")) ?? 10)
    }).first {return element}
    try await pause(0.05)
    }
    throw Failure.message("No accessible element named " + label)
}
func windowAX(_ pid: Int32) throws -> AXUIElement {
    guard let windows = attr(AXUIElementCreateApplication(pid),"AXWindows") as? [AXUIElement],
          let window = windows.filter({rectangle($0).map{$0.width>400 && $0.height>300} ?? false}).max(by:{
              let left=rectangle($0)!,right=rectangle($1)!
              return left.width*left.height < right.width*right.height
          }) else { throw Failure.message("Fixture window is missing") }
    return window
}
func key(_ code: CGKeyCode, _ flags: CGEventFlags = []) {
    let source=CGEventSource(stateID:.hidSystemState)
    let modifiers:[(CGEventFlags,CGKeyCode)]=[(.maskControl,59),(.maskAlternate,58),(.maskShift,56),(.maskCommand,55),(.maskSecondaryFn,63)]
    var held=CGEventFlags()
    for (flag,code) in modifiers where flags.contains(flag) {
        held.insert(flag)
        let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:true)!
        event.type = .flagsChanged;event.flags=held;event.post(tap:.cghidEventTap)
    }
    for down in [true,false] {
        let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:down)!
        event.flags=flags;event.post(tap:.cghidEventTap)
    }
    for (flag,code) in modifiers.reversed() where flags.contains(flag) {
        held.remove(flag)
        let event=CGEvent(keyboardEventSource:source,virtualKey:code,keyDown:false)!
        event.type = .flagsChanged;event.flags=held;event.post(tap:.cghidEventTap)
    }
}
func click(_ point: CGPoint) {
    let original=CGEvent(source:nil)!.location
    let source = CGEventSource(stateID: .hidSystemState)
    CGWarpMouseCursorPosition(point)
    CGEvent(mouseEventSource:source,mouseType:.mouseMoved,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
    Thread.sleep(forTimeInterval:0.04)
    defer{Thread.sleep(forTimeInterval:0.2);CGWarpMouseCursorPosition(original)}
    for kind in [CGEventType.leftMouseDown,.leftMouseUp] {
        let event=CGEvent(mouseEventSource: source,mouseType: kind,mouseCursorPosition: point,mouseButton: .left)!
        event.flags=[];event.post(tap:.cghidEventTap)
    }
}
func pause(_ seconds: Double) async throws { try await Task.sleep(nanoseconds: UInt64(seconds*1e9)) }
@MainActor func waitForApplicationFocus(_ pid: Int32) throws {
    let application = try app(pid)
    let accessibility = AXUIElementCreateApplication(pid)
    var activation: AXError?
    var raised: AXError?
    let deadline = Date(timeIntervalSinceNow: 3)
    repeat {
        // AppKit refreshes process state on main run-loop turns. Keep this poll
        // synchronous so the observer services that loop before reading state.
        // A system-wide AXFocusedApplication query can block during native menu tracking.
        CFRunLoopRunInMode(.defaultMode, 0.01, true)
        if NSWorkspace.shared.frontmostApplication?.processIdentifier == pid { return }
        // A background observer's cooperative activate() request is not user intent.
        // Use AX only to activate the validated fixture when it is not foreground.
        if activation == nil, let window = try? windowAX(pid) {
            raised = AXUIElementPerformAction(window, "AXRaise" as CFString)
            activation = AXUIElementSetAttributeValue(accessibility, "AXFrontmost" as CFString, kCFBooleanTrue)
        }
        CFRunLoopRunInMode(.defaultMode, 0.05, true)
    } while Date() < deadline
    throw Failure.message("Disposable desktop app did not become foreground: finished=\(application.isFinishedLaunching), policy=\(application.activationPolicy.rawValue), activation=\(activation?.rawValue ?? -1), raised=\(raised?.rawValue ?? -1), front=\(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0)")
}
@MainActor func focus(_ pid: Int32) async throws {
    try waitForApplicationFocus(pid)
    try await pause(0.15)
}
@MainActor func press(_ pid: Int32, _ label: String) async throws {
    let element = try await find(pid,label)
    var nativeActions: CFArray?
    let actionRead = AXUIElementCopyActionNames(element, &nativeActions)
    if string(element, "AXRole") == "AXDisclosureTriangle", actionRead == .success,
       !(nativeActions as? [String] ?? []).contains("AXPress") {
        // WebKit summaries advertise no AXPress; Space activates their native disclosure.
        _ = try await keyboardTarget(pid, label, activate: true)
        return
    }
    let error = AXUIElementPerformAction(element,"AXPress" as CFString)
    if error != .success {
        guard let rect = rectangle(element) else { throw Failure.message("Cannot press " + label) }
        click(CGPoint(x:rect.midX,y:rect.midY))
    }
    try await pause(0.12)
}
@MainActor func chooseOption(_ pid: Int32, _ dialogTitle: String, _ label: String, _ value: String) async throws -> [String: Any] {
    let application = AXUIElementCreateApplication(pid)
    func popup() throws -> AXUIElement {
        let dialog = nodes(application).first { candidate in
            ["AXGroup", "AXDialog", "AXSheet"].contains(string(candidate, "AXRole")) &&
            ["AXTitle", "AXDescription"].contains { string(candidate, $0) == dialogTitle }
        }
        guard let dialog, let control = nodes(dialog).first(where: { candidate in
            string(candidate, "AXRole") == "AXPopUpButton" &&
            ["AXTitle", "AXDescription"].contains { string(candidate, $0) == label }
        }) else { throw Failure.message("No native " + label + " picker in " + dialogTitle) }
        return control
    }
    let control = try popup(), before = row(control)
    try require(AXUIElementPerformAction(control, "AXPress" as CFString) == .success,
                "The scoped native " + label + " picker did not accept AXPress")
    var observed: [[String: Any]] = []
    for attempt in 1...60 {
        let menus = nodes(application).filter { string($0, "AXRole") == "AXMenu" }
        let items = menus.flatMap { nodes($0) }.filter { string($0, "AXRole") == "AXMenuItem" }
        observed = items.map { row($0) }
        if let item = items.first(where: { candidate in
            ["AXTitle", "AXDescription", "AXValue"].contains { string(candidate, $0) == value }
        }) {
            let chosen = row(item)
            try require(AXUIElementPerformAction(item, "AXPress" as CFString) == .success,
                        "The open native menu option did not accept AXPress: " + value)
            for _ in 0..<40 {
                let current = try popup()
                if string(current, "AXValue") == value {
                    return ["before": before, "menu_option": chosen, "after": row(current),
                            "menu_observation_attempts": attempt]
                }
                try await pause(0.05)
            }
            throw Failure.message("The native picker did not commit " + value + "; current value: " + string(try popup(), "AXValue"))
        }
        try await pause(0.05)
    }
    let detail = String(data: try JSONSerialization.data(withJSONObject: observed), encoding: .utf8) ?? "[]"
    throw Failure.message("No open native menu option " + value + "; observed menu items: " + detail)
}
func clipboardSaved() -> [(NSPasteboard.PasteboardType,Data)] {
    let board=NSPasteboard.general
    return (board.types ?? []).compactMap { kind in board.data(forType:kind).map { (kind,$0) } }
}
func restoreClipboard(_ saved: [(NSPasteboard.PasteboardType,Data)]) {
    let board=NSPasteboard.general; board.clearContents()
    for (kind,data) in saved { board.setData(data,forType:kind) }
}
func sourceID(_ source: TISInputSource) -> String {
    guard let pointer=TISGetInputSourceProperty(source,kTISPropertyInputSourceID) else { return "" }
    return Unmanaged<CFString>.fromOpaque(pointer).takeUnretainedValue() as String
}
func inputSources() -> [TISInputSource] {
    TISCreateInputSourceList(nil,true).takeRetainedValue() as! [TISInputSource]
}
func selectSource(_ id: String) throws {
    guard let source=inputSources().first(where:{sourceID($0)==id}) else {throw Failure.message("Input source unavailable: "+id)}
    try require(TISSelectInputSource(source)==noErr,"Input source could not be selected")
}
@MainActor func captureWindow(_ pid: Int32) async throws -> SCWindow {
    let content=try await SCShareableContent.excludingDesktopWindows(true,onScreenWindowsOnly:true)
    guard let window=content.windows.first(where:{$0.owningApplication?.processID==pid && $0.frame.width>400}) else {
        throw Failure.message("No capturable fixture window")
    }
    return window
}
func configuration(_ window: SCWindow) -> SCStreamConfiguration {
    let config=SCStreamConfiguration()
    config.width=Int(window.frame.width*2); config.height=Int(window.frame.height*2)
    config.pixelFormat=kCVPixelFormatType_32BGRA
    config.minimumFrameInterval=CMTime(value:1,timescale:120)
    config.queueDepth=5; config.showsCursor=false; config.ignoreShadowsSingleWindow=true
    config.captureResolution = .best
    return config
}
@main @MainActor struct Driver {
 static func main() async {
    do {try await run()} catch {
        FileHandle.standardError.write(Data(("Desktop observer: "+String(describing:error)+"\n").utf8))
        exit(1)
    }
 }
 static func run() async throws {
    let args=Array(CommandLine.arguments.dropFirst())
    try require(!args.isEmpty,"Choose an observer operation")
    let mode=args[0]
    _ = NSApplication.shared
    _ = NSScreen.main
    if mode=="environment" {
        try emit(["screen_capture":CGPreflightScreenCaptureAccess(),"accessibility":AXIsProcessTrusted(),"post_events":CGPreflightPostEventAccess(),
                  "front_pid":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0,
                  "input_source":sourceID(TISCopyCurrentKeyboardInputSource().takeRetainedValue()),
                  "input_sources":inputSources().map{sourceID($0)},
                  "voiceover_running":NSRunningApplication.runningApplications(withBundleIdentifier:"com.apple.VoiceOver").count>0,
                  "display":NSScreen.screens.map {["width":$0.frame.width,"height":$0.frame.height,"scale":$0.backingScaleFactor,"max_fps":$0.maximumFramesPerSecond]}])
        return
    }
    if mode=="restore-front" {
        if let pid=Int32(args[1]) {NSRunningApplication(processIdentifier:pid)?.activate(options:[])}
        return
    }
    try require(args.count>=2,"An owned fixture PID is required")
    let pid=Int32(args[1])!
    if mode=="activate" {
        for _ in 0..<60 {
            if (try? app(pid)) != nil {break}
            try await pause(0.05)
        }
    }
    _ = try app(pid)
    if mode=="ax" {
        let tree=nodes(AXUIElementCreateApplication(pid)).map{row($0)}
        if args.count==3 {try save(tree,args[2])} else {try emit(tree)}
        return
    }
    if mode=="quit" {try await focus(pid);key(12,.maskCommand);try await pause(0.2);return}
    try await focus(pid)
    if mode=="activate" {return}
    if try await richOperation(mode,pid,args) {return}
    if try await attachmentOperation(mode,pid,args) {return}
    if try await draftOperation(mode,pid,args) {return}
    if mode=="press" {try await press(pid,args[2]);return}
    if mode=="choose-option" {try emit(try await chooseOption(pid,args[2],args[3],args[4]));return}
    if mode=="click" {
        let element=try await find(pid,args[2]);guard let rect=rectangle(element) else {throw Failure.message("Button has no bounds")}
        click(CGPoint(x:rect.midX,y:rect.midY));try await pause(0.15);return
    }
    if mode=="copy-button" {
        let saved=clipboardSaved();defer{restoreClipboard(saved)}
        NSPasteboard.general.clearContents();try await press(pid,args[2]);try await pause(0.25)
        try emit(["text":NSPasteboard.general.string(forType:.string) ?? ""]);return
    }
    if mode=="send-latency" {
        let started=CACurrentMediaTime();key(36)
        for _ in 0..<100 {
            if nodes(AXUIElementCreateApplication(pid)).contains(where:{string($0,"AXValue").contains(args[2]) || string($0,"AXDescription").contains(args[2])}) {
                try emit(["native_event_to_accessible_reply_ms":(CACurrentMediaTime()-started)*1000]);return
            }
            try await pause(0.02)
        }
        throw Failure.message("No visible reply within two seconds")
    }
    if mode=="focus" {
        let element=try await find(pid,args[2]);let error=AXUIElementSetAttributeValue(element,"AXFocused" as CFString,kCFBooleanTrue)
        let editable=["AXTextArea","AXTextField"].contains(string(element,"AXRole"))
        if editable || error != .success || attr(element,"AXFocused") as? Bool != true {
            if let rect=rectangle(element) {click(CGPoint(x:rect.midX,y:rect.midY))}
        }
        try await pause(0.2);return
    }
    if mode=="folder" {
        try require(nodes(AXUIElementCreateApplication(pid)).contains(where:{string($0,"AXRole")=="AXSheet"}),
                    "A native folder dialog must be open")
        let saved=clipboardSaved();defer{restoreClipboard(saved)}
        key(5,[.maskCommand,.maskShift]);try await pause(0.25)
        NSPasteboard.general.clearContents();NSPasteboard.general.setString(args[2],forType:.string)
        key(9,.maskCommand);try await pause(0.25);key(36);try await pause(0.3)
        key(36);try await pause(0.2);return
    }
    if mode=="paste" {
        let saved=clipboardSaved();defer{restoreClipboard(saved)}
        NSPasteboard.general.clearContents();NSPasteboard.general.setString(args[2],forType:.string)
        key(9,.maskCommand);try await pause(0.25);return
    }
    if mode=="copy" {
        let saved=clipboardSaved();defer{restoreClipboard(saved)}
        key(8,.maskCommand);try await pause(0.2)
        try emit(["text":NSPasteboard.general.string(forType:.string) ?? ""]);return
    }
    if mode=="key" {key(CGKeyCode(args[2])!,CGEventFlags(rawValue:UInt64(args[3])!));try await pause(0.1);return}
    if mode=="ime" {
        let previous=TISCopyCurrentKeyboardInputSource().takeRetainedValue()
        defer{TISSelectInputSource(previous)}
        guard let source=inputSources().first(where:{sourceID($0)==args[2]}) else {throw Failure.message("Requested IME unavailable")}
        let parentID=args[2].components(separatedBy:".Japanese").first!
        let parent=inputSources().first(where:{sourceID($0)==parentID})
        let sources=[parent,source].compactMap{$0}
        let enabled=sources.map {source in
            CFBooleanGetValue(Unmanaged<CFBoolean>.fromOpaque(TISGetInputSourceProperty(source,kTISPropertyInputSourceIsEnabled)!).takeUnretainedValue())
        }
        defer{
            TISSelectInputSource(previous)
            for (source,wasEnabled) in zip(sources,enabled).reversed() {
                if !wasEnabled {TISDisableInputSource(source)}
            }
        }
        for (source,wasEnabled) in zip(sources,enabled) {
            if !wasEnabled {try require(TISEnableInputSource(source)==noErr,"IME could not be enabled")}
        }
        try selectSource(args[2]);try await pause(0.5)
        let input=try await find(pid,"Message the agent")
        if let bounds=rectangle(input) {click(CGPoint(x:bounds.midX,y:bounds.midY))}
        try await pause(0.2)
        // 'nihon' through the real Japanese input source; space converts, Return commits.
        for code: CGKeyCode in [45,34,4,31,45] {key(code);try await pause(0.06)}
        let preedit=nodes(AXUIElementCreateApplication(pid)).map{row($0)}
        let composed=string(input,"AXValue").unicodeScalars.contains { (0x3040...0x30ff).contains($0.value) || (0x4e00...0x9fff).contains($0.value) }
        if composed {key(49);try await pause(0.12);key(36);try await pause(0.2)}
        else {key(53);try await pause(0.2)}
        try save(["source":args[2],"actual_source":sourceID(TISCopyCurrentKeyboardInputSource().takeRetainedValue()),"native_preedit":composed,"front_pid":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0,"preedit":preedit,"committed":nodes(AXUIElementCreateApplication(pid)).map{row($0)}],args[3]);return
    }
    if mode=="resize" {
        let window=try windowAX(pid);var size=CGSize(width:Double(args[2])!,height:Double(args[3])!)
        try require(AXUIElementSetAttributeValue(window,"AXSize" as CFString,AXValueCreate(.cgSize,&size)!) == .success,"Resize failed")
        try await pause(0.15);return
    }
    let window=try await captureWindow(pid),filter=SCContentFilter(desktopIndependentWindow:window),config=configuration(window)
    if mode=="screenshot" {
        let image=try await SCScreenshotManager.captureImage(contentFilter:filter,configuration:config)
        guard let data=NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:]) else {throw Failure.message("PNG encoding failed")}
        try data.write(to:URL(fileURLWithPath:args[2]));return
    }
    throw Failure.message("Unknown observer operation "+mode)
 }
}
