// Records what a person at this Mac would notice while a scenario runs: app activations,
// the frontmost window changing owner, Space switches, the pointer moving and clipboard writes.
// It only observes; it posts no input and needs no Accessibility or Screen Recording permission.
//
//   swiftc -O scripts/interruption-monitor.swift -o /tmp/interruption-monitor
//   /tmp/interruption-monitor OUT.json   # runs until SIGINT/SIGTERM, then writes OUT.json
import AppKit
import CoreGraphics
import Foundation

struct Event: Encodable {
    let t: Double
    let kind: String
    let detail: String
    /// The person used the keyboard or mouse within the last second, so the event may be theirs.
    let personActive: Bool
}

let outPath = CommandLine.arguments.dropFirst().first ?? "/dev/stdout"
let start = Date()
var events: [Event] = []

func now() -> Double { Date().timeIntervalSince(start) }
/// Hardware input types only. The catch-all "any input" counter also resets on background
/// pid-posted clicks, so it would mistake the agent's own clicks for the person.
let personInputTypes: [CGEventType] = [.keyDown, .flagsChanged, .mouseMoved, .leftMouseDown, .rightMouseDown, .otherMouseDown, .leftMouseDragged, .scrollWheel]
func secondsSincePersonInput() -> Double {
    personInputTypes.map { CGEventSource.secondsSinceLastEventType(.hidSystemState, eventType: $0) }.min() ?? 0
}
func record(_ kind: String, _ detail: String) {
    events.append(Event(t: (now() * 1000).rounded() / 1000, kind: kind, detail: detail, personActive: secondsSincePersonInput() < 1))
}
func describe(_ app: NSRunningApplication?) -> String {
    guard let app else { return "none" }
    return "\(app.bundleIdentifier ?? app.localizedName ?? "?") pid \(app.processIdentifier)"
}
func cursor() -> CGPoint { NSEvent.mouseLocation }
func frontWindowOwner() -> String {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else { return "unknown" }
    for info in list where (info[kCGWindowLayer as String] as? Int) == 0 {
        let pid = info[kCGWindowOwnerPID as String] as? Int ?? -1
        let name = info[kCGWindowOwnerName as String] as? String ?? "?"
        return "\(name) pid \(pid)"
    }
    return "none"
}

let initialFront = describe(NSWorkspace.shared.frontmostApplication)
let initialCursor = cursor()
let initialChangeCount = NSPasteboard.general.changeCount
var lastCursor = initialCursor
var moving = false
var lastChangeCount = initialChangeCount
var lastFrontWindow = frontWindowOwner()
let initialFrontWindow = lastFrontWindow

let center = NSWorkspace.shared.notificationCenter
center.addObserver(forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main) { note in
    let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
    record("activation", describe(app))
}
center.addObserver(forName: NSWorkspace.activeSpaceDidChangeNotification, object: nil, queue: .main) { _ in
    record("space", "active Space changed")
}

let timer = Timer(timeInterval: 0.02, repeats: true) { _ in
    let point = cursor()
    let moved = abs(point.x - lastCursor.x) > 0.5 || abs(point.y - lastCursor.y) > 0.5
    if moved && !moving {
        record("cursor", "from \(Int(lastCursor.x)),\(Int(lastCursor.y)) to \(Int(point.x)),\(Int(point.y))")
    }
    moving = moved
    lastCursor = point
    let count = NSPasteboard.general.changeCount
    if count != lastChangeCount {
        record("clipboard", "changeCount \(lastChangeCount) -> \(count)")
        lastChangeCount = count
    }
    let front = frontWindowOwner()
    if front != lastFrontWindow {
        record("front_window", "\(lastFrontWindow) -> \(front)")
        lastFrontWindow = front
    }
}
RunLoop.main.add(timer, forMode: .common)

func finish() -> Never {
    let finalCursor = cursor()
    var counts: [String: Int] = [:]
    for event in events { counts[event.kind, default: 0] += 1 }
    let total = events.count
    let report: [String: Any] = [
        "durationSeconds": (now() * 100).rounded() / 100,
        "interruptions": total,
        "counts": counts,
        "personActiveEvents": events.filter(\.personActive).count,
        "initial": ["frontmostApp": initialFront, "frontWindow": initialFrontWindow, "cursor": [initialCursor.x, initialCursor.y], "clipboardChangeCount": initialChangeCount],
        "final": ["frontmostApp": describe(NSWorkspace.shared.frontmostApplication), "frontWindow": frontWindowOwner(), "cursor": [finalCursor.x, finalCursor.y], "clipboardChangeCount": NSPasteboard.general.changeCount],
        "restored": [
            "frontmostApp": describe(NSWorkspace.shared.frontmostApplication) == initialFront,
            "cursor": abs(finalCursor.x - initialCursor.x) <= 0.5 && abs(finalCursor.y - initialCursor.y) <= 0.5,
        ],
        "events": events.map { ["t": $0.t, "kind": $0.kind, "detail": $0.detail, "personActive": $0.personActive] },
    ]
    let data = try! JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
    FileManager.default.createFile(atPath: outPath, contents: data)
    exit(0)
}

var signalSources: [DispatchSourceSignal] = []
for sig in [SIGINT, SIGTERM] {
    signal(sig, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: sig, queue: .main)
    source.setEventHandler { finish() }
    source.resume()
    signalSources.append(source)
}
// Ready marker for the wrapper: observation has started.
FileManager.default.createFile(atPath: outPath + ".ready", contents: Data())
RunLoop.main.run()
