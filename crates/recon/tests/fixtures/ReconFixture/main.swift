import AppKit

// MARK: - Evidence

/// What actually reached this application, written where an out-of-process test can read it.
///
/// Every handler records what it was given rather than what it expected: the button, the click
/// count, the modifiers, the wheel deltas and the offset they produced, and the key events
/// around a chord. A test asserts on this record, so a posted event that ran no handler, a
/// handler that ran twice, and a handler that ran in the other window are all visible.
///
/// The file is rewritten whole after every record, atomically, so a reader sees either the
/// record before an event or the record after it. Every call happens on the main thread, the
/// only thread AppKit calls these handlers on.
final class Evidence {
    static let shared = Evidence(toolkit: "appkit")

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

    /// Beside the built bundle, under `target/fixtures/evidence`, named for this incarnation.
    /// The test knows the directory it built the fixture into and the pid it launched, so
    /// neither side has to agree on a temporary directory or an environment variable.
    static func evidenceURL() -> URL {
        let directory = Bundle.main.bundleURL.deletingLastPathComponent()
            .appendingPathComponent("evidence", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true)
        let name = Bundle.main.bundleURL.deletingPathExtension().lastPathComponent
        return directory.appendingPathComponent("\(name)-\(getpid()).json")
    }

    /// Publishes one window's identity and the window-local geometry of its controls, so a test
    /// can address a control by name without guessing where the fixture drew it.
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

/// The modifier names Recon's own chord and click grammar uses, so a test compares like with
/// like rather than decoding AppKit's bit mask.
func modifierNames(_ flags: NSEvent.ModifierFlags) -> [String] {
    var names: [String] = []
    if flags.contains(.command) { names.append("cmd") }
    if flags.contains(.control) { names.append("ctrl") }
    if flags.contains(.option) { names.append("alt") }
    if flags.contains(.shift) { names.append("shift") }
    return names
}

func buttonName(_ event: NSEvent) -> String {
    switch event.buttonNumber {
    case 0: return "left"
    case 1: return "right"
    case 2: return "middle"
    default: return "button-\(event.buttonNumber)"
    }
}

/// A mouse event as the test reads it: which control, which button, how many clicks, which
/// modifiers, and where in the window's own coordinates — measured from the top left of the
/// content area, the origin every published control rectangle shares.
func mouseFields(_ event: NSEvent, control: String, in view: NSView) -> [String: Any] {
    var fields: [String: Any] = [
        "control": control,
        "button": buttonName(event),
        "count": event.clickCount,
        "modifiers": modifierNames(event.modifierFlags),
    ]
    if let content = view.window?.contentView {
        let point = content.convert(event.locationInWindow, from: nil)
        fields["x"] = point.x
        fields["y"] = content.bounds.height - point.y
    }
    return fields
}

// MARK: - Instrumented controls

/// A window that reports when it becomes or stops being its application's key window. A
/// background application's own key window may change, and this is how a test sees that it
/// changed in the selected window and nowhere else.
final class InstrumentedWindow: NSWindow {
    var label = ""
    /// Every addressable control of this window, in points from the top left of its content
    /// area. Measured from the live view tree rather than computed, so a border, a scroller or
    /// a scrolled position cannot put the published rectangle somewhere the control is not.
    var placements: [(name: String, rect: NSRect)] = []

    override func becomeKey() {
        super.becomeKey()
        Evidence.shared.record("key-window", window: label, ["isKey": true])
    }

    override func resignKey() {
        super.resignKey()
        Evidence.shared.record("key-window", window: label, ["isKey": false])
    }
}

/// A button that advertises `AXPress` and records which mechanism reached it. An accessibility
/// press and a mouse click both end in the same action, so the action alone cannot tell a test
/// which one Recon chose.
final class PressButton: NSButton {
    var label = ""
    var name = ""

    override func accessibilityPerformPress() -> Bool {
        Evidence.shared.record("ax-press", window: label, ["control": name])
        return super.accessibilityPerformPress()
    }

    override func mouseDown(with event: NSEvent) {
        Evidence.shared.record(
            "mouse-down", window: label, mouseFields(event, control: name, in: self))
        // NSButton runs its own tracking loop and never calls `mouseUp(with:)`, so the press
        // ends in the action below rather than in a matching override here.
        super.mouseDown(with: event)
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    @objc func fired(_ sender: Any?) {
        Evidence.shared.record("action", window: label, ["control": name])
    }
}

/// A plain target for coordinate clicks. It records every button, click count, modifier set and
/// window-local point it is given, and it opens no menu: a context menu would reorder windows
/// on the desktop the test is watching.
final class ClickCanvas: NSView {
    var label = ""
    var name = ""

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override var isOpaque: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.controlBackgroundColor.setFill()
        dirtyRect.fill()
    }

    override func mouseDown(with event: NSEvent) { record("mouse-down", event) }
    override func mouseUp(with event: NSEvent) { record("mouse-up", event) }
    override func rightMouseDown(with event: NSEvent) { record("mouse-down", event) }
    override func rightMouseUp(with event: NSEvent) { record("mouse-up", event) }
    override func otherMouseDown(with event: NSEvent) { record("mouse-down", event) }
    override func otherMouseUp(with event: NSEvent) { record("mouse-up", event) }

    override func menu(for event: NSEvent) -> NSMenu? { nil }

    private func record(_ kind: String, _ event: NSEvent) {
        Evidence.shared.record(kind, window: label, mouseFields(event, control: name, in: self))
    }
}

/// A scroll view that records the wheel event it was given and the offset that event produced.
/// Units, axes and directions are facts about the event; movement is a fact about the scroll
/// view, and the two are recorded separately because a delivered event that moved nothing is
/// exactly what this fixture exists to make visible.
final class ScrollRegion: NSScrollView {
    var label = ""
    var name = ""

    override func scrollWheel(with event: NSEvent) {
        super.scrollWheel(with: event)
        let offset = contentView.bounds.origin
        Evidence.shared.record(
            "scroll", window: label,
            [
                "control": name,
                "deltaX": event.scrollingDeltaX,
                "deltaY": event.scrollingDeltaY,
                "precise": event.hasPreciseScrollingDeltas,
                "modifiers": modifierNames(event.modifierFlags),
                "offsetX": offset.x,
                "offsetY": offset.y,
            ])
    }

    func observeOffset() {
        contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(
            forName: NSView.boundsDidChangeNotification, object: contentView, queue: nil
        ) { [weak self] _ in
            self?.recordOffset()
        }
        recordOffset()
    }

    private func recordOffset() {
        let offset = contentView.bounds.origin
        Evidence.shared.record(
            "scroll-offset", window: label,
            ["control": name, "offsetX": offset.x, "offsetY": offset.y])
    }
}

/// The window's own key handling: one action for `cmd+b`, and the key events around it.
///
/// `performKeyEquivalent(with:)` is offered to this view by its own window, so a chord AppKit
/// routed to the other window's key handling never reaches it. The key events are recorded
/// too: a chord sent as a physical press ends with a key up that carries no modifier, and a
/// chord that left a modifier held does not.
final class ChordResponder: NSView {
    var label = ""

    override var acceptsFirstResponder: Bool { true }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if flags == .command, event.charactersIgnoringModifiers?.lowercased() == "b" {
            Evidence.shared.record(
                "chord", window: label, ["chord": "cmd+b", "repeat": event.isARepeat])
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    override func keyDown(with event: NSEvent) {
        Evidence.shared.record("key-down", window: label, keyFields(event))
    }

    override func keyUp(with event: NSEvent) {
        Evidence.shared.record("key-up", window: label, keyFields(event))
    }

    override func flagsChanged(with event: NSEvent) {
        Evidence.shared.record(
            "flags", window: label,
            ["modifiers": modifierNames(event.modifierFlags), "keyCode": Int(event.keyCode)])
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

/// A flipped document, so that a larger offset means the content moved down — the direction
/// Recon's positive `--dy` names.
final class ScrollDocument: NSView {
    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.textBackgroundColor.setFill()
        dirtyRect.fill()
        NSColor.separatorColor.setStroke()
        let path = NSBezierPath()
        for line in stride(from: CGFloat(0), through: bounds.height, by: 40) {
            path.move(to: NSPoint(x: 0, y: line))
            path.line(to: NSPoint(x: bounds.width, y: line))
        }
        for line in stride(from: CGFloat(0), through: bounds.width, by: 40) {
            path.move(to: NSPoint(x: line, y: 0))
            path.line(to: NSPoint(x: line, y: bounds.height))
        }
        path.stroke()
    }
}

// MARK: - Layout

let contentSize = NSSize(width: 520, height: 480)

/// A content-area rectangle written from the top left, which is the direction every published
/// coordinate is measured in, converted to the bottom-left rectangle AppKit lays out with.
func placed(x: CGFloat, y: CGFloat, width: CGFloat, height: CGFloat) -> NSRect {
    NSRect(x: x, y: contentSize.height - y - height, width: width, height: height)
}

/// Where a live view is, in points from the top left of the content area.
func measured(_ view: NSView, in content: NSView) -> NSRect {
    let rect = content.convert(view.bounds, from: view)
    return NSRect(
        x: rect.minX, y: content.bounds.height - rect.maxY,
        width: rect.width, height: rect.height)
}

/// Everything a window is instrumented with, built the same way for both windows so that a
/// test can send the same operation to either one and compare the two records.
func instrument(window: InstrumentedWindow, label: String) {
    guard let content = window.contentView else { return }

    let chord = ChordResponder(frame: content.bounds)
    chord.label = label
    chord.autoresizingMask = [.width, .height]
    content.addSubview(chord)

    var controls: [(name: String, view: NSView)] = []

    for (name, title, x, enabled) in [
        ("record", "Record", CGFloat(24), true),
        ("disabled", "Disabled", CGFloat(152), false),
        // Two controls a selector cannot tell apart. An operation that picked one of them
        // anyway would act on a window position the caller never chose.
        ("twin-1", "Twin", CGFloat(280), true),
        ("twin-2", "Twin", CGFloat(400), true),
    ] {
        let button = PressButton(title: title, target: nil, action: nil)
        button.frame = placed(x: x, y: 20, width: 112, height: 32)
        button.label = label
        button.name = name
        button.target = button
        button.action = #selector(PressButton.fired(_:))
        button.isEnabled = enabled
        content.addSubview(button)
        controls.append((name, button))
    }

    let canvas = ClickCanvas(frame: placed(x: 24, y: 104, width: 472, height: 120))
    canvas.label = label
    canvas.name = "canvas"
    canvas.setAccessibilityRole(.group)
    canvas.setAccessibilityLabel("Canvas")
    content.addSubview(canvas)
    controls.append(("canvas", canvas))

    let outer = scrollRegion(
        label: label, name: "scroll-outer",
        frame: placed(x: 24, y: 236, width: 472, height: 220))
    content.addSubview(outer)
    controls.append(("scroll-outer", outer))

    // The nested region sits inside the outer region's document, so a point inside it addresses
    // both: only the innermost scroll view may move, and the record says which one did. It is
    // placed clear of the outer region's own centre, which is the point that addresses the
    // outer region alone.
    let nested = scrollRegion(
        label: label, name: "scroll-nested",
        frame: NSRect(x: 780, y: 520, width: 160, height: 80))
    outer.documentView?.addSubview(nested)
    controls.append(("scroll-nested", nested))

    window.makeFirstResponder(chord)
    window.initialFirstResponder = chord

    // The nested region is published as the part of it the outer region actually shows: a point
    // in the clipped part addresses the outer region, not the nested one.
    let visible = measured(outer.contentView, in: content)
    window.placements = controls.map { control in
        let rect = measured(control.view, in: content)
        let published = control.name == "scroll-nested" ? rect.intersection(visible) : rect
        return (control.name, published)
    }
}

/// A scroll view whose document is larger than it is in both directions, parked in the middle
/// so that every axis and every direction has somewhere to go.
func scrollRegion(label: String, name: String, frame: NSRect) -> ScrollRegion {
    let scroll = ScrollRegion(frame: frame)
    scroll.label = label
    scroll.name = name
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = true
    scroll.autohidesScrollers = false
    scroll.scrollerStyle = .legacy
    scroll.borderType = .bezelBorder
    scroll.documentView = ScrollDocument(frame: NSRect(x: 0, y: 0, width: 1200, height: 1200))
    scroll.contentView.scroll(to: NSPoint(x: 500, y: 500))
    scroll.reflectScrolledClipView(scroll.contentView)
    scroll.observeOffset()
    return scroll
}

// MARK: - The application

final class ReconFixtureDelegate: NSObject, NSApplicationDelegate {
    private var openWindows: [NSWindow] = []
    private var statusItem: NSStatusItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = makeMainMenu()
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        item.button?.title = "ReconFixture"
        let statusMenu = NSMenu()
        statusMenu.addItem(withTitle: "Fixture Status Item", action: nil, keyEquivalent: "")
        item.menu = statusMenu
        statusItem = item

        // Inside a background target AppKit may still report itself active; the record says so
        // rather than the test inferring it.
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

        // The window ordered front last stays the application's main window, and observers
        // expect that to be the main window rather than the inspector. The fixture never
        // activates itself, so `open -g` leaves the frontmost application untouched.
        let inspector = makeInspectorWindow()
        let main = makeMainWindow()
        for window in [inspector, main] {
            openWindows.append(window)
            window.orderFront(nil)
        }
        for window in [inspector, main] {
            publish(window)
        }
    }

    /// The window's identity and geometry, published once both windows are ordered in: a window
    /// number is assigned then, and the test matches it against the window `recon ui windows`
    /// reports.
    private func publish(_ window: InstrumentedWindow) {
        let controls = window.placements.map { placement in
            [
                "name": placement.name,
                "x": placement.rect.minX,
                "y": placement.rect.minY,
                "width": placement.rect.width,
                "height": placement.rect.height,
            ] as [String: Any]
        }
        Evidence.shared.publish(window: [
            "label": window.label,
            "title": window.title,
            "windowId": window.windowNumber,
            "contentWidth": contentSize.width,
            "contentHeight": contentSize.height,
            "controls": controls,
        ])
    }
}

func makeMainWindow() -> InstrumentedWindow {
    let window = makeWindow(title: "Recon Fixture — Main", column: 0, label: "main")
    guard let content = window.contentView else { return window }

    let value = NSTextField(string: "fixture value")
    value.frame = placed(x: 24, y: 64, width: 260, height: 24)
    content.addSubview(value)

    let enabled = NSButton(checkboxWithTitle: "Enabled", target: nil, action: nil)
    enabled.frame = placed(x: 300, y: 64, width: 160, height: 24)
    content.addSubview(enabled)

    window.placements += [
        ("value", measured(value, in: content)),
        ("enabled", measured(enabled, in: content)),
    ]
    return window
}

func makeInspectorWindow() -> InstrumentedWindow {
    makeWindow(title: "Recon Fixture — Inspector", column: 1, label: "inspector")
}

/// Two windows of the same size, side by side inside the screen's visible area, so that both
/// are on the current Space and one foreground window can cover them.
func makeWindow(title: String, column: Int, label: String) -> InstrumentedWindow {
    let window = InstrumentedWindow(
        contentRect: NSRect(origin: .zero, size: contentSize),
        styleMask: [.titled, .closable, .miniaturizable, .resizable],
        backing: .buffered,
        defer: false
    )
    window.title = title
    window.label = label
    window.isReleasedWhenClosed = false
    let visible = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
    let frame = window.frameRect(forContentRect: NSRect(origin: .zero, size: contentSize))
    window.setFrameOrigin(
        NSPoint(
            x: visible.minX + 20 + CGFloat(column) * (frame.width + 20),
            y: visible.maxY - frame.height - 20))
    instrument(window: window, label: label)
    return window
}

func makeMainMenu() -> NSMenu {
    let mainMenu = NSMenu()

    let applicationItem = NSMenuItem()
    let applicationMenu = NSMenu(title: "ReconFixture")
    applicationMenu.addItem(
        withTitle: "Quit ReconFixture",
        action: #selector(NSApplication.terminate(_:)),
        keyEquivalent: "q"
    )
    applicationItem.submenu = applicationMenu
    mainMenu.addItem(applicationItem)

    let fixtureItem = NSMenuItem()
    let fixtureMenu = NSMenu(title: "Fixture")
    fixtureMenu.addItem(withTitle: "First Item", action: nil, keyEquivalent: "")
    fixtureMenu.addItem(withTitle: "Second Item", action: nil, keyEquivalent: "")
    fixtureItem.submenu = fixtureMenu
    mainMenu.addItem(fixtureItem)

    return mainMenu
}

let application = NSApplication.shared
application.setActivationPolicy(.regular)
let delegate = ReconFixtureDelegate()
application.delegate = delegate
application.run()
