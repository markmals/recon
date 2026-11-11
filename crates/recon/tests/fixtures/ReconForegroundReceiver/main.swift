import AppKit

// MARK: - Evidence

/// What reached the application the person at the Mac is using, written where an
/// out-of-process test can read it. The record has the same shape as the two fixtures', so one
/// reader serves all three.
///
/// This application exists to hold the user's place while Recon acts on a background target: it
/// stays frontmost, keeps the keyboard focus, and covers the targets. An interaction that took
/// the user's input would show up here as a key event, and one that took the user's focus would
/// show up as a key-window or activation change.
final class Evidence {
    static let shared = Evidence(toolkit: "receiver")

    private let url: URL
    private let toolkit: String
    private let started = Date()
    private var windows: [[String: Any]] = []
    private var events: [[String: Any]] = []
    private var sequence = 0

    init(toolkit: String) {
        self.toolkit = toolkit
        self.url = Evidence.evidenceURL()
        write()
    }

    static func evidenceURL() -> URL {
        let directory = Bundle.main.bundleURL.deletingLastPathComponent()
            .appendingPathComponent("evidence", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true)
        let name = Bundle.main.bundleURL.deletingPathExtension().lastPathComponent
        return directory.appendingPathComponent("\(name)-\(getpid()).json")
    }

    /// Where the test writes a command for this application to carry out. A negative control
    /// needs the frontmost application to change something about itself while Recon runs, and
    /// only the frontmost application can do that to itself.
    static func commandURL() -> URL {
        evidenceURL().deletingPathExtension().appendingPathExtension("command")
    }

    func publish(window: [String: Any]) {
        windows.append(window)
        write()
    }

    func record(_ kind: String, window: String, _ fields: [String: Any] = [:]) {
        sequence += 1
        var event: [String: Any] = [
            "sequence": sequence,
            "kind": kind,
            "window": window,
            "at": Date().timeIntervalSince(started),
        ]
        event.merge(fields) { _, replacement in replacement }
        events.append(event)
        write()
    }

    private func write() {
        let document: [String: Any] = [
            "pid": Int(getpid()),
            "toolkit": toolkit,
            "windows": windows,
            "events": events,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: document, options: [])
        else { return }
        try? data.write(to: url, options: .atomic)
    }
}

func modifierNames(_ flags: NSEvent.ModifierFlags) -> [String] {
    var names: [String] = []
    if flags.contains(.command) { names.append("cmd") }
    if flags.contains(.control) { names.append("ctrl") }
    if flags.contains(.option) { names.append("alt") }
    if flags.contains(.shift) { names.append("shift") }
    return names
}

// MARK: - Instrumented windows

final class ReceiverWindow: NSWindow {
    var label = ""

    override func becomeKey() {
        super.becomeKey()
        Evidence.shared.record("key-window", window: label, ["isKey": true])
    }

    override func resignKey() {
        super.resignKey()
        Evidence.shared.record("key-window", window: label, ["isKey": false])
    }
}

/// Everything the user's keyboard and mouse would do here. Nothing acts on it: the point is the
/// record, not a response.
final class ReceiverView: NSView {
    var label = ""

    override var acceptsFirstResponder: Bool { true }

    override var isOpaque: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.windowBackgroundColor.setFill()
        dirtyRect.fill()
    }

    override func keyDown(with event: NSEvent) {
        Evidence.shared.record("key-down", window: label, keyFields(event))
    }

    override func keyUp(with event: NSEvent) {
        Evidence.shared.record("key-up", window: label, keyFields(event))
    }

    override func flagsChanged(with event: NSEvent) {
        Evidence.shared.record(
            "flags", window: label, ["modifiers": modifierNames(event.modifierFlags)])
    }

    override func mouseDown(with event: NSEvent) { record("mouse-down", event) }
    override func mouseUp(with event: NSEvent) { record("mouse-up", event) }
    override func rightMouseDown(with event: NSEvent) { record("mouse-down", event) }
    override func otherMouseDown(with event: NSEvent) { record("mouse-down", event) }
    override func scrollWheel(with event: NSEvent) {
        Evidence.shared.record(
            "scroll", window: label,
            ["deltaX": event.scrollingDeltaX, "deltaY": event.scrollingDeltaY])
    }

    override func menu(for event: NSEvent) -> NSMenu? { nil }

    private func record(_ kind: String, _ event: NSEvent) {
        Evidence.shared.record(
            kind, window: label,
            [
                "button": event.buttonNumber,
                "count": event.clickCount,
                "modifiers": modifierNames(event.modifierFlags),
            ])
    }

    private func keyFields(_ event: NSEvent) -> [String: Any] {
        [
            "key": event.charactersIgnoringModifiers ?? "",
            "modifiers": modifierNames(event.modifierFlags),
            "keyCode": Int(event.keyCode),
            "repeat": event.isARepeat,
        ]
    }
}

// MARK: - The application

final class ReceiverDelegate: NSObject, NSApplicationDelegate {
    private var front: ReceiverWindow?
    private var secondary: ReceiverWindow?
    private var timer: Timer?
    private var lastCommand = ""

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = makeMainMenu()
        for state in [
            NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification,
        ] {
            NotificationCenter.default.addObserver(forName: state, object: nil, queue: nil) {
                note in
                Evidence.shared.record(
                    "active", window: "application",
                    ["active": note.name == NSApplication.didBecomeActiveNotification])
            }
        }

        let visible = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        // The front window covers the area both fixtures place their windows in: an operation
        // that needs the target visible, or that raises it, is then plain to see.
        let front = makeWindow(title: "Recon Receiver — Front", label: "front", frame: visible)
        let secondarySize = NSSize(width: 320, height: 200)
        let secondary = makeWindow(
            title: "Recon Receiver — Secondary", label: "secondary",
            frame: NSRect(
                x: visible.maxX - secondarySize.width - 40,
                y: visible.minY + 40,
                width: secondarySize.width,
                height: secondarySize.height))
        self.front = front
        self.secondary = secondary

        secondary.orderFront(nil)
        front.makeKeyAndOrderFront(nil)
        // This application is the one the person at the Mac is using, so it is the one that is
        // frontmost while every interaction runs.
        NSApp.activate(ignoringOtherApps: true)

        for window in [front, secondary] { publish(window) }

        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            self?.readCommand()
        }
    }

    /// One command per write. The test writes a word and waits for the matching record, so a
    /// control that did not happen cannot be mistaken for one that did.
    private func readCommand() {
        guard let text = try? String(contentsOf: Evidence.commandURL(), encoding: .utf8) else {
            return
        }
        let command = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard command != lastCommand, !command.isEmpty else { return }
        lastCommand = command
        switch command {
        case "key-window-secondary":
            secondary?.makeKey()
        case "key-window-front":
            front?.makeKey()
        case "activate":
            NSApp.activate(ignoringOtherApps: true)
        case "quit":
            NSApp.terminate(nil)
        default:
            Evidence.shared.record("command-unknown", window: "application", ["command": command])
            return
        }
        Evidence.shared.record("command", window: "application", ["command": command])
    }

    private func publish(_ window: ReceiverWindow) {
        Evidence.shared.publish(window: [
            "label": window.label,
            "title": window.title,
            "windowId": window.windowNumber,
            "contentWidth": window.contentView?.bounds.width ?? 0,
            "contentHeight": window.contentView?.bounds.height ?? 0,
            "controls": [[String: Any]](),
        ])
    }

    /// `frame` is the whole window, title bar included, so the given rectangle is exactly what
    /// the window covers on screen.
    private func makeWindow(title: String, label: String, frame: NSRect) -> ReceiverWindow {
        let window = ReceiverWindow(
            contentRect: frame,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.label = label
        window.isReleasedWhenClosed = false
        window.setFrame(frame, display: false)
        let view = ReceiverView(frame: NSRect(origin: .zero, size: frame.size))
        view.label = label
        view.autoresizingMask = [.width, .height]
        window.contentView = view
        window.initialFirstResponder = view
        window.makeFirstResponder(view)
        return window
    }

    private func makeMainMenu() -> NSMenu {
        let mainMenu = NSMenu()
        let applicationItem = NSMenuItem()
        let applicationMenu = NSMenu(title: "ReconForegroundReceiver")
        applicationMenu.addItem(
            withTitle: "Quit ReconForegroundReceiver",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        applicationItem.submenu = applicationMenu
        mainMenu.addItem(applicationItem)
        return mainMenu
    }
}

let application = NSApplication.shared
application.setActivationPolicy(.regular)
let delegate = ReceiverDelegate()
application.delegate = delegate
application.run()
