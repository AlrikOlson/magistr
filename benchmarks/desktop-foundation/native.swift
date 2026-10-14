// Native observer for the foundation comparison. Captures only fixture windows.
import AppKit
import ApplicationServices
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import QuartzCore
import Carbon
import Vision

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
    (attr(element,"AXChildren") as? [AXUIElement] ?? []) + (attr(element,"AXWindows") as? [AXUIElement] ?? [])
}
func nodes(_ root: AXUIElement) -> [AXUIElement] {
    var result: [AXUIElement] = [], queue = [root]
    var seen = Set<CFHashCode>()
    while !queue.isEmpty && result.count < 10_000 {
        let node = queue.removeFirst()
        if !seen.insert(CFHash(node)).inserted { continue }
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
    guard let application = NSRunningApplication(processIdentifier: pid),
          application.bundleIdentifier?.hasPrefix("dev.magistr.foundation.") == true else {
        throw Failure.message("Input is restricted to the disposable foundation apps")
    }
    return application
}
@MainActor func find(_ pid: Int32, _ label: String) async throws -> AXUIElement {
    for _ in 0..<100 {
    let matches=nodes(AXUIElementCreateApplication(pid)).filter { element in
        ["AXDescription","AXTitle","AXValue"].contains(where: {string(element,$0) == label})
    }
    let priority=["AXTextArea","AXTextField","AXButton","AXImage","AXLink"]
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
@MainActor func focus(_ pid: Int32) async throws {
    try app(pid).activate(options: [.activateAllWindows])
    try await pause(0.15)
}
@MainActor func press(_ pid: Int32, _ label: String) async throws {
    let element = try await find(pid,label)
    let error = AXUIElementPerformAction(element,"AXPress" as CFString)
    if error != .success {
        guard let rect = rectangle(element) else { throw Failure.message("Cannot press " + label) }
        click(CGPoint(x:rect.midX,y:rect.midY))
    }
    try await pause(0.12)
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
final class Observer: NSObject, SCStreamOutput {
    let lock=NSLock()
    var frames: [[String:Any]]=[]
    var point=CGPoint.zero
    var expected: Bool?
    var sent=0.0
    var latency: [String:Double]?
    var previousHash: UInt64=0
    func arm(_ review: Bool) {
        lock.lock(); expected=review; sent=CACurrentMediaTime(); latency=nil; lock.unlock()
    }
    func result() -> [String:Double]? {lock.lock(); defer{lock.unlock()}; return latency}
    func stream(_ stream: SCStream,didOutputSampleBuffer sample: CMSampleBuffer,of type: SCStreamOutputType) {
        guard type == .screen, let buffer=CMSampleBufferGetImageBuffer(sample),
              let attachments=CMSampleBufferGetSampleAttachmentsArray(sample,createIfNecessary:false) as? [[SCStreamFrameInfo:Any]],
              let info=attachments.first, let status=info[.status] as? Int,
              status==SCFrameStatus.complete.rawValue else {return}
        let arrived=CACurrentMediaTime()
        var base=mach_timebase_info_data_t(); mach_timebase_info(&base)
        let display=(info[.displayTime] as? NSNumber).map{$0.doubleValue*Double(base.numer)/Double(base.denom)/1e9} ?? CMTimeGetSeconds(CMSampleBufferGetPresentationTimeStamp(sample))
        CVPixelBufferLockBaseAddress(buffer,.readOnly); defer{CVPixelBufferUnlockBaseAddress(buffer,.readOnly)}
        guard let pointer=CVPixelBufferGetBaseAddress(buffer) else{return}
        let width=CVPixelBufferGetWidth(buffer),height=CVPixelBufferGetHeight(buffer),stride=CVPixelBufferGetBytesPerRow(buffer)
        let bytes=pointer.assumingMemoryBound(to:UInt8.self)
        lock.lock(); defer{lock.unlock()}
        var hash: UInt64=1469598103934665603
        // Only the fixture is sampled. A grid distinguishes changing from idle frames.
        for y in Swift.stride(from:height/8,to:height,by:max(1,height/16)) {
            for x in Swift.stride(from:width/8,to:width,by:max(1,width/16)) {
                for channel in 0..<3 {hash=(hash ^ UInt64(bytes[y*stride+x*4+channel])) &* 1099511628211}
            }
        }
        frames.append(["display_s":display,"callback_s":arrived,"changed":hash != previousHash])
        previousHash=hash
        let x=max(0,min(width-1,Int(point.x))),y=max(0,min(height-1,Int(point.y)))
        let pixel=y*stride+x*4,b=Double(bytes[pixel]),r=Double(bytes[pixel+2])
        let review=r>b*1.3 && r>150
        let main=b>r*1.15 && b>150
        if let wanted=expected, (wanted ? review : main),display>=sent {
            latency=["display_ms":(display-sent)*1000,"callback_ms":(arrived-sent)*1000]
            expected=nil
        }
    }
}
@MainActor @discardableResult func voiceoverEvidence(_ path: String) async throws -> [String:Any] {
    let content=try await SCShareableContent.excludingDesktopWindows(true,onScreenWindowsOnly:true)
    guard let window=content.windows.first(where:{
        $0.owningApplication?.bundleIdentifier=="com.apple.VoiceOver" &&
        $0.frame.width>300 && $0.frame.width<1000 && $0.frame.height>20 && $0.frame.height<300
    }) else {throw Failure.message("VoiceOver caption panel is unavailable")}
    let image=try await SCScreenshotManager.captureImage(contentFilter:SCContentFilter(desktopIndependentWindow:window),configuration:configuration(window))
    let target=URL(fileURLWithPath:path).deletingPathExtension().appendingPathExtension("png")
    try NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:])!.write(to:target)
    let request=VNRecognizeTextRequest();request.recognitionLevel = .accurate
    try VNImageRequestHandler(cgImage:image,options:[:]).perform([request])
    let text=(request.results ?? []).compactMap{$0.topCandidates(1).first?.string}.joined(separator:" ")
    let result: [String:Any]=["voiceover_pid":window.owningApplication!.processID,
        "caption":text,"image":target.lastPathComponent,"observation":"OCR of the actual VoiceOver caption panel"]
    try save(result,path);return result
}
@main @MainActor struct Driver {
 static func main() async {
    do {try await run()} catch {
        FileHandle.standardError.write(Data(("Foundation observer: "+String(describing:error)+"\n").utf8))
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
        try emit(["screen_capture":CGPreflightScreenCaptureAccess(),"accessibility":AXIsProcessTrusted(),
                  "front_pid":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0,
                  "input_source":sourceID(TISCopyCurrentKeyboardInputSource().takeRetainedValue()),
                  "input_sources":inputSources().map{sourceID($0)},
                  "voiceover_running":NSRunningApplication.runningApplications(withBundleIdentifier:"com.apple.VoiceOver").count>0,
                  "display":NSScreen.screens.map {["width":$0.frame.width,"height":$0.frame.height,"scale":$0.backingScaleFactor,"max_fps":$0.maximumFramesPerSecond]}])
        return
    }
    if mode=="launch" {
        try require(args.count==3,"launch bundle output")
        let options=NSWorkspace.OpenConfiguration()
        options.arguments=[args[2]];options.createsNewApplicationInstance=true
        let started=CACurrentMediaTime()
        let application=try await NSWorkspace.shared.openApplication(at:URL(fileURLWithPath:args[1]),configuration:options)
        try emit(["pid":application.processIdentifier,"launch_return_ms":(CACurrentMediaTime()-started)*1000])
        return
    }
    if mode=="voiceover-caption" {
        try await voiceoverEvidence(args[1]);return
    }
    if mode=="restore-front" {
        if let pid=Int32(args[1]) {NSRunningApplication(processIdentifier:pid)?.activate(options:[])}
        return
    }
    try require(args.count>=2,"An owned fixture PID is required")
    let pid=Int32(args[1])!;_ = try app(pid)
    if mode=="ax" {
        let tree=nodes(AXUIElementCreateApplication(pid)).map{row($0)}
        if args.count==3 {try save(tree,args[2])} else {try emit(tree)}
        return
    }
    if mode=="quit" {try app(pid).terminate();return}
    try await focus(pid)
    if mode=="voiceover-navigate" {
        var steps: [[String:Any]]=[]
        for (index,command) in args[3].split(separator:",").enumerated() {
            let parts=command.split(separator:"/");try require(parts.count==2,"Use key/flags pairs")
            key(CGKeyCode(parts[0])!,CGEventFlags(rawValue:UInt64(parts[1])!));try await pause(0.4)
            var evidence=try await voiceoverEvidence(args[2]+"-"+String(index)+".json")
            evidence["key"]=Int(parts[0])!;evidence["flags"]=UInt64(parts[1])!;steps.append(evidence)
        }
        try save(["steps":steps],args[2]+".json");return
    }
    if mode=="voiceover-walk" {
        var steps: [[String:Any]]=[],activated=false
        for index in 0..<Int(args[3])! {
            key(48);try await pause(0.35)
            var evidence=try await voiceoverEvidence(args[2]+"-"+String(index)+".json")
            evidence["step"]=index;evidence["key"]="Tab"
            evidence["focused"]=nodes(AXUIElementCreateApplication(pid)).filter{attr($0,"AXFocused") as? Bool == true}.map{row($0)}
            if !activated,let caption=evidence["caption"] as? String,caption.lowercased().contains("start work"),caption.lowercased().contains("button") {
                key(49,[.maskControl,.maskAlternate]);try await pause(0.35)
                evidence["activation"]=try await voiceoverEvidence(args[2]+"-activation.json");activated=true
            }
            steps.append(evidence)
        }
        try save(["steps":steps,"activated_start":activated],args[2]+".json");return
    }
    if mode=="press" {try await press(pid,args[2]);return}
    if mode=="focus" {
        let element=try await find(pid,args[2]);let error=AXUIElementSetAttributeValue(element,"AXFocused" as CFString,kCFBooleanTrue)
        let editable=["AXTextArea","AXTextField"].contains(string(element,"AXRole"))
        if editable || error != .success || attr(element,"AXFocused") as? Bool != true {
            if let rect=rectangle(element) {click(CGPoint(x:rect.midX,y:rect.midY))}
        }
        try await pause(0.2);return
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
        let input=try await find(pid,"Message")
        if let bounds=rectangle(input) {click(CGPoint(x:bounds.midX,y:bounds.midY))}
        try await pause(0.2)
        // 'nihon' through the real Japanese input source; space converts, Return commits.
        for code: CGKeyCode in [45,34,4,31,45] {key(code);try await pause(0.06)}
        let preedit=nodes(AXUIElementCreateApplication(pid)).map{row($0)}
        key(49);try await pause(0.12);key(36);try await pause(0.2)
        try save(["source":args[2],"actual_source":sourceID(TISCopyCurrentKeyboardInputSource().takeRetainedValue()),"front_pid":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0,"preedit":preedit,"committed":nodes(AXUIElementCreateApplication(pid)).map{row($0)}],args[3]);return
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
    let observer=Observer(),stream=SCStream(filter:filter,configuration:config,delegate:nil)
    try stream.addStreamOutput(observer,type:.screen,sampleHandlerQueue:DispatchQueue(label:"foundation.capture"))
    try await stream.startCapture();try await pause(0.3)
    if mode=="latency" || mode=="typing" {
        let marker=try await find(pid,mode=="typing" ? "Message empty" : "Main branch selected");guard let rect=rectangle(marker) else {throw Failure.message("Marker has no bounds")}
        observer.point=CGPoint(x:(rect.midX-window.frame.minX)*2,y:(rect.midY-window.frame.minY)*2)
        var samples: [[String:Double]]=[]
        for index in 0..<Int(args[2])! {
            let review=index%2==0;observer.arm(review)
            if mode=="typing" {key(review ? 0 : 51)} else {key(review ? 19 : 18,.maskCommand)}
            var found: [String:Double]?
            for _ in 0..<200 {try await pause(0.01);found=observer.result();if found != nil {break}}
            guard let result=found else {throw Failure.message("No visible response to workspace shortcut")}
            samples.append(result);try await pause(0.06)
        }
        try await stream.stopCapture()
        try save(["samples":samples,"frames":observer.frames,"sampling_fps":120,"window":[window.frame.minX,window.frame.minY,window.frame.width,window.frame.height]],args[3]);return
    }
    if mode=="motion" {
        let original=CGEvent(source:nil)!.location
        defer{CGWarpMouseCursorPosition(original)}
        CGWarpMouseCursorPosition(CGPoint(x:window.frame.minX+window.frame.width*0.8,y:window.frame.minY+window.frame.height*0.6))
        let scrollStart=CACurrentMediaTime()
        for _ in 0..<120 {
            CGEvent(scrollWheelEvent2Source:nil,units:.pixel,wheelCount:1,wheel1:-24,wheel2:0,wheel3:0)?.post(tap:.cghidEventTap)
            try await pause(1/120)
        }
        try await pause(0.15)
        let resizeStart=CACurrentMediaTime(),ax=try windowAX(pid)
        var sizes: [[String:Any]]=[]
        for index in 0..<60 {
            var size=CGSize(width:1280+Double(index%20)*4,height:832+Double(index%20)*2)
            let error=AXUIElementSetAttributeValue(ax,"AXSize" as CFString,AXValueCreate(.cgSize,&size)!)
            try await pause(1/60)
            sizes.append(["error":error.rawValue,"requested":[size.width,size.height],"actual":rectangle(ax).map{[$0.width,$0.height]} ?? []])
        }
        try await pause(0.15);let end=CACurrentMediaTime()
        try await stream.stopCapture()
        try save(["frames":observer.frames,"sizes":sizes,"scroll_start_s":scrollStart,"resize_start_s":resizeStart,"end_s":end,"sampling_fps":120],args[2]);return
    }
    throw Failure.message("Unknown observer operation "+mode)
 }
}