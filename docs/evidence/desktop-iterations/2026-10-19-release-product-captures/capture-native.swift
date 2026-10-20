// Read-only capture of this task's isolated Magistr preview; no input or activation.
import AppKit
import ScreenCaptureKit
import CoreMedia
enum CaptureError: Error { case invalidTarget, missingWindow }
@main @MainActor struct Capture {
  static func main() async throws {
    _ = NSApplication.shared
    _ = NSScreen.main
    let args=Array(CommandLine.arguments.dropFirst())
    guard args.count==2,let pid=Int32(args[0]),
      let app=NSRunningApplication(processIdentifier:pid),
      app.bundleIdentifier=="dev.magistr.desktop.release-capture" else {throw CaptureError.invalidTarget}
    if args[1] == "--quit" {
      guard app.terminate() else {throw CaptureError.invalidTarget}
      return
    }
    let content=try await SCShareableContent.excludingDesktopWindows(true,onScreenWindowsOnly:false)
    guard let window=content.windows.first(where:{$0.owningApplication?.processID==pid && $0.frame.width>700 && $0.frame.height>500}) else {throw CaptureError.missingWindow}
    let config=SCStreamConfiguration()
    config.width=Int(window.frame.width*2);config.height=Int(window.frame.height*2)
    config.showsCursor=false;config.ignoreShadowsSingleWindow=true;config.captureResolution = .best
    let image=try await SCScreenshotManager.captureImage(contentFilter:SCContentFilter(desktopIndependentWindow:window),configuration:config)
    let url=URL(fileURLWithPath:args[1])
    guard !FileManager.default.fileExists(atPath:url.path) else {throw CaptureError.invalidTarget}
    try NSBitmapImageRep(cgImage:image).representation(using:.png,properties:[:])!.write(to:url)
    let receipt:[String:Any]=["pid":pid,"windowId":window.windowID,"width":image.width,"height":image.height,
      "viewport":["width":window.frame.width,"height":window.frame.height],"capturedAt":ISO8601DateFormatter().string(from:Date()),
      "renderer":"native macOS WKWebView; ScreenCaptureKit window capture","foregroundInput":false,
      "frontPid":NSWorkspace.shared.frontmostApplication?.processIdentifier ?? 0]
    try JSONSerialization.data(withJSONObject:receipt,options:[.prettyPrinted,.sortedKeys]).write(to:url.deletingPathExtension().appendingPathExtension("json"))
    print(String(data:try JSONSerialization.data(withJSONObject:receipt),encoding:.utf8)!)
  }
}
