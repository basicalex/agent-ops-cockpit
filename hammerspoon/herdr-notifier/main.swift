// herdr-notifier: posts macOS notifications for Herdr panes and handles clicks.
//
// Runs as the executable of a small .app bundle (one bundle per project, so the
// notification carries the project's icon). Two entry modes:
//
//   <bundle>/Contents/MacOS/herdr-notifier post --title T [--subtitle S] [--body B]
//        --socket PATH --pane ID [--id KEY] [--sound]
//     asks for notification permission if needed, posts the banner with the
//     socket path and pane id in userInfo, then exits.
//
//   launched with no arguments (macOS does this when the banner is clicked)
//     receives the click, sends pane.focus to the Herdr socket, activates the
//     terminal app, and exits. UNUserNotificationCenter removes the banner on click.

import AppKit
import Foundation
import UserNotifications

let terminalBundleID = ProcessInfo.processInfo.environment["HERDR_NOTIFIER_TERMINAL"] ?? "com.mitchellh.ghostty"

func log(_ s: String) {
    FileHandle.standardError.write((s + "\n").data(using: .utf8)!)
}

func herdrRequest(socketPath: String, json: String) -> String? {
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else { return nil }
    defer { close(fd) }
    var addr = sockaddr_un()
    addr.sun_family = sa_family_t(AF_UNIX)
    let pathBytes = Array(socketPath.utf8)
    guard pathBytes.count < MemoryLayout.size(ofValue: addr.sun_path) else { return nil }
    withUnsafeMutableBytes(of: &addr.sun_path) { raw in
        raw.copyBytes(from: pathBytes)
        raw[pathBytes.count] = 0
    }
    let len = socklen_t(MemoryLayout<sockaddr_un>.size)
    let rc = withUnsafePointer(to: &addr) { p in
        p.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, len) }
    }
    guard rc == 0 else { return nil }
    var tv = timeval(tv_sec: 3, tv_usec: 0)
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
    let payload = Array((json + "\n").utf8)
    guard write(fd, payload, payload.count) == payload.count else { return nil }
    var out = [UInt8]()
    var buf = [UInt8](repeating: 0, count: 4096)
    while true {
        let n = read(fd, &buf, buf.count)
        if n <= 0 { break }
        out.append(contentsOf: buf[0..<n])
        if out.contains(10) { break }
    }
    return String(decoding: out, as: UTF8.self)
}

func jsonString(_ s: String) -> String {
    let data = try! JSONSerialization.data(withJSONObject: [s])
    let arr = String(decoding: data, as: UTF8.self)
    return String(arr.dropFirst().dropLast())
}

func focusPane(socketPath: String, paneID: String) {
    let req = "{\"id\":\"notifier\",\"method\":\"pane.focus\",\"params\":{\"pane_id\":\(jsonString(paneID))}}"
    if let reply = herdrRequest(socketPath: socketPath, json: req) {
        log("pane.focus -> \(reply.trimmingCharacters(in: .whitespacesAndNewlines))")
    } else {
        log("pane.focus failed: no reply from \(socketPath)")
    }
    if let app = NSRunningApplication.runningApplications(withBundleIdentifier: terminalBundleID).first {
        app.activate(options: [.activateAllWindows])
    } else if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: terminalBundleID) {
        NSWorkspace.shared.openApplication(at: url, configuration: NSWorkspace.OpenConfiguration())
    }
}

func parseArgs(_ args: [String]) -> [String: String] {
    var out: [String: String] = [:]
    var i = 0
    while i < args.count {
        let a = args[i]
        if a.hasPrefix("--") {
            let key = String(a.dropFirst(2))
            if i + 1 < args.count, !args[i + 1].hasPrefix("--") {
                out[key] = args[i + 1]; i += 2
            } else {
                out[key] = "1"; i += 1
            }
        } else { i += 1 }
    }
    return out
}

final class ClickDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        UNUserNotificationCenter.current().delegate = self
        // Nothing arrived: exit rather than linger as a background process.
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) { exit(0) }
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter,
                                didReceive response: UNNotificationResponse,
                                withCompletionHandler completionHandler: @escaping () -> Void) {
        let info = response.notification.request.content.userInfo
        if response.actionIdentifier != UNNotificationDismissActionIdentifier,
           let sock = info["socket"] as? String, let pane = info["pane"] as? String {
            focusPane(socketPath: sock, paneID: pane)
        }
        completionHandler()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { exit(0) }
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter,
                                willPresent notification: UNNotification,
                                withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void) {
        completionHandler([.banner, .list, .sound])
    }
}

let argv = CommandLine.arguments
if argv.count > 1, argv[1] == "post" {
    let opts = parseArgs(Array(argv.dropFirst(2)))
    guard let title = opts["title"], let sock = opts["socket"], let pane = opts["pane"] else {
        log("usage: herdr-notifier post --title T [--subtitle S] [--body B] --socket PATH --pane ID [--id KEY] [--sound]")
        exit(2)
    }
    let center = UNUserNotificationCenter.current()
    let done = DispatchSemaphore(value: 0)
    var status: Int32 = 0
    center.requestAuthorization(options: [.alert, .sound]) { granted, error in
        if !granted {
            log("notifications not allowed for \(Bundle.main.bundleIdentifier ?? "?"): \(error?.localizedDescription ?? "denied")")
            status = 3; done.signal(); return
        }
        let content = UNMutableNotificationContent()
        content.title = title
        if let s = opts["subtitle"] { content.subtitle = s }
        if let b = opts["body"] { content.body = b }
        if opts["sound"] != nil { content.sound = .default }
        content.userInfo = ["socket": sock, "pane": pane]
        let id = opts["id"] ?? pane
        content.threadIdentifier = id
        let request = UNNotificationRequest(identifier: id, content: content, trigger: nil)
        center.add(request) { error in
            if let error = error { log("post failed: \(error.localizedDescription)"); status = 4 }
            done.signal()
        }
    }
    _ = done.wait(timeout: .now() + 10)
    exit(status)
}

// Ask for permission and exit; run once per bundle after it is created so
// macOS registers it and shows the permission prompt.
if argv.count > 1, argv[1] == "register" {
    let done = DispatchSemaphore(value: 0)
    UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { granted, _ in
        log("\(Bundle.main.bundleIdentifier ?? "?"): notifications \(granted ? "allowed" : "not allowed")")
        done.signal()
    }
    _ = done.wait(timeout: .now() + 30)
    exit(0)
}

if argv.count > 1, argv[1] == "withdraw" {
    let opts = parseArgs(Array(argv.dropFirst(2)))
    let center = UNUserNotificationCenter.current()
    if let id = opts["id"] {
        center.removeDeliveredNotifications(withIdentifiers: [id])
    } else {
        center.removeAllDeliveredNotifications()
    }
    Thread.sleep(forTimeInterval: 0.3)
    exit(0)
}

// No arguments: launched by a click on one of our notifications.
let app = NSApplication.shared
let delegate = ClickDelegate()
app.delegate = delegate
app.setActivationPolicy(.prohibited)
app.run()
