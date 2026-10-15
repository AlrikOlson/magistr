// Background macOS WebKit regression: production UI, scripted admission, no OS input or visible window.
import AppKit
import CryptoKit
import WebKit

enum Failure: Error { case message(String) }
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let source = URL(fileURLWithPath: #filePath)
let ui = source.deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("ui")
let output = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? ".magistr/desktop-project-opening")
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)

let configuration = WKWebViewConfiguration()
// Only local, bundled fixture assets are loaded; there is no network or real application bridge.
configuration.preferences.setValue(true, forKey: "allowFileAccessFromFileURLs")
configuration.userContentController.addUserScript(WKUserScript(source: """
window.__errors=[];
window.addEventListener("error",event=>window.__errors.push(event.message));
window.__TAURI__={core:{Channel:class{},async invoke(command,args={}){
  switch(command){
    case "overview":return {directory:"/Projects/procgenfart",reduce_motion:false,sessions:[],projects:[],attachment_limits:{file_bytes:8388608}};
    case "preferences":return {revision:1,preferences:{theme:"dark",reduce_motion:false}};
    case "subscribe_catalog":args.channel.onmessage({stream:1,sequence:1,sessions:[]});return 1;
    case "preview_project":return window.__review
      ? {kind:"setup",review:1,directory:args.directory,setup_directory:args.directory,files:[
          {path:"AGENTS.md",action:"Create file",ownership:"Managed by Magistr",selectable:true,before:null,after:"# Project instructions",diff:null}
        ]}
      : {kind:"ready",directory:args.directory};
    case "open_session":return new Promise((resolve,reject)=>window.__failOpening=reject);
    default:return;
  }
}}};
window.__until=async predicate=>{
  const deadline=performance.now()+3000;
  while(!predicate()){
    if(performance.now()>deadline)throw Error("The expected opening state did not arrive.");
    await new Promise(resolve=>setTimeout(resolve,20));
  }
};
window.__measure=ids=>{
  const dialog=document.getElementById("project-picker"),bounds=dialog.getBoundingClientRect();
  const controls=ids.map(id=>{
    const node=document.getElementById(id),rect=node.getBoundingClientRect();
    return {id,rect:rect.toJSON(),visible:rect.width>0&&rect.height>0&&rect.top>=bounds.top&&rect.bottom<=bounds.bottom&&rect.left>=bounds.left&&rect.right<=bounds.right};
  });
  return {dialog:bounds.toJSON(),controls,open:dialog.open,active:document.activeElement.id,errors:window.__errors};
};
""", injectionTime: .atDocumentStart, forMainFrameOnly: true))
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: 1280, height: 820), configuration: configuration)
let window = NSWindow(contentRect: web.frame, styleMask: [.titled], backing: .buffered, defer: false)
window.contentView = web

@MainActor final class Journey: NSObject, WKNavigationDelegate {
    var observations: [[String: Any]] = []

    func javascript(_ body: String) async throws -> Any? {
        try await web.callAsyncJavaScript(body, arguments: [:], in: nil, contentWorld: .page)
    }
    func capture(_ name: String) async throws {
        let image = try await web.takeSnapshot(configuration: nil)
        guard let tiff = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:]) else {
            throw Failure.message("Could not capture the project dialog.")
        }
        try png.write(to: output.appendingPathComponent(name + ".png"))
    }
    func check(_ name: String, _ ids: [String]) async throws {
        let encoded = String(data: try JSONSerialization.data(withJSONObject: ids), encoding: .utf8)!
        let result = try await javascript("return window.__measure(" + encoded + ");") as! [String: Any]
        observations.append(["name": name, "measurement": result])
        try await capture(name)
        let bounds = result["dialog"] as! [String: Any]
        let controls = result["controls"] as! [[String: Any]]
        guard result["open"] as? Bool == true, (bounds["height"] as? Double ?? 0) > 200,
              (bounds["y"] as? Double ?? -1) >= 0,
              (bounds["bottom"] as? Double ?? .infinity) <= web.bounds.height,
              controls.allSatisfy({ $0["visible"] as? Bool == true }),
              (result["errors"] as? [String])?.isEmpty == true else {
            throw Failure.message("Project dialog or recovery controls are clipped: \(result)")
        }
    }
    func run() async throws {
        _ = try await javascript("await window.__until(()=>!!window.__failOpening);")
        try await check("opening-dark", ["picker-title", "picker-status", "picker-submit", "picker-cancel"])
        _ = try await javascript("""
        window.__failOpening("Your saved message could not be read. Try again.");
        await window.__until(()=>!document.getElementById("picker-failure").hidden);
        """)
        try await check("error-dark", ["picker-error", "picker-error-message", "picker-submit", "picker-cancel"])
        window.setContentSize(NSSize(width: 760, height: 550))
        _ = try await javascript("document.documentElement.dataset.theme='light';")
        try await check("error-minimum-light", ["picker-error", "picker-error-message", "picker-submit", "picker-cancel"])
        _ = try await javascript("""
        document.getElementById("picker-cancel").click();
        await window.__until(()=>!document.getElementById("project-picker").open);
        if(document.activeElement.id!=="open-project"||document.querySelector("dialog[open]"))throw Error("Cancel did not restore the project chooser.");
        window.__review=true;
        document.getElementById("open-project").click();
        document.getElementById("picker-submit").click();
        await window.__until(()=>!document.getElementById("setup-preview").hidden);
        """)
        try await check("review-minimum-light", ["picker-title", "setup-count", "picker-submit", "picker-cancel"])
        _ = try await javascript("""
        document.getElementById("change-folder").click();
        await window.__until(()=>document.getElementById("setup-preview").hidden);
        """)
        try await check("choose-again-minimum-light", ["picker-title", "project-folder", "picker-submit", "picker-cancel"])
    }
    func receipt(_ passed: Bool, _ error: String? = nil) throws {
        let assets = try FileManager.default.contentsOfDirectory(at: ui, includingPropertiesForKeys: nil)
        var hashes: [String: String] = [:]
        for file in assets + [source] {
            hashes[file.lastPathComponent] = SHA256.hash(data: try Data(contentsOf: file)).map { String(format: "%02x", $0) }.joined()
        }
        var value: [String: Any] = [
            "passed": passed, "platform": ProcessInfo.processInfo.operatingSystemVersionString,
            "method": "Hidden macOS WKWebView; production assets; scripted commands and DOM actions; no native input.",
            "source": hashes, "observations": observations
        ]
        if let error { value["error"] = error }
        try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
            .write(to: output.appendingPathComponent("receipt.json"))
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        Task { @MainActor in
            do {
                try await run()
                try receipt(true)
                print("PASS: opening, failure, compact recovery, setup review and return to folder entry in macOS WebKit.")
                exit(0)
            } catch {
                try? receipt(false, String(describing: error))
                print("FAIL: \(error)")
                exit(1)
            }
        }
    }
}
let journey = MainActor.assumeIsolated { Journey() }
web.navigationDelegate = journey
web.loadFileURL(ui.appendingPathComponent("index.html"), allowingReadAccessTo: ui)
DispatchQueue.main.asyncAfter(deadline: .now() + 20) {
    try? journey.receipt(false, "The native WebKit scenario timed out.")
    print("FAIL: The native WebKit scenario timed out.")
    exit(1)
}
app.run()
