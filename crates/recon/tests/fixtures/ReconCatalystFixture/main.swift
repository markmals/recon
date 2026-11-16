import Foundation
import UIKit

// MARK: - Evidence

/// What actually reached this Mac Catalyst application, written where an out-of-process test
/// can read it. The record has the same shape as the AppKit fixture's, so one test reads both
/// and a Catalyst row cannot quietly become an AppKit row.
///
/// The file is rewritten whole after every record, atomically. Every call happens on the main
/// thread, the only thread UIKit calls these handlers on.
final class Evidence {
    static let shared = Evidence(toolkit: "catalyst")

    private let url: URL
    private let toolkit: String
    private let started = Date()
    private var windows: [String: [String: Any]] = [:]
    private var events: [[String: Any]] = []
    private var sequence = 0

    init(toolkit: String) {
        self.toolkit = toolkit
        self.url = Evidence.evidenceURL()
        write()
    }

    /// Beside the built bundle, under `target/fixtures/evidence`, named for this incarnation.
    static func evidenceURL() -> URL {
        let directory = Bundle.main.bundleURL.deletingLastPathComponent()
            .appendingPathComponent("evidence", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true)
        let name = Bundle.main.bundleURL.deletingPathExtension().lastPathComponent
        return directory.appendingPathComponent("\(name)-\(getpid()).json")
    }

    /// One window's identity and the window-local geometry of its controls. A scene publishes
    /// itself once its host window exists, and republishes nothing: the fixture never moves or
    /// resizes a window after that.
    func publish(window: [String: Any]) {
        guard let label = window["label"] as? String else { return }
        windows[label] = window
        write()
    }

    func published(_ label: String) -> Bool { windows[label] != nil }

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
            "windows": Array(windows.values),
            "events": events,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: document, options: [])
        else { return }
        try? data.write(to: url, options: .atomic)
    }
}

/// The modifier names Recon's own chord and click grammar uses.
func modifierNames(_ flags: UIKeyModifierFlags) -> [String] {
    var names: [String] = []
    if flags.contains(.command) { names.append("cmd") }
    if flags.contains(.control) { names.append("ctrl") }
    if flags.contains(.alternate) { names.append("alt") }
    if flags.contains(.shift) { names.append("shift") }
    return names
}

/// Which button a UIKit event carries. UIKit names the primary and secondary buttons and
/// numbers the rest, so the raw mask is recorded too: a button the system never delivers and a
/// button it delivers under another name are different findings.
func buttonName(_ mask: UIEvent.ButtonMask) -> String {
    if mask.contains(.primary) { return "left" }
    if mask.contains(.secondary) { return "right" }
    if mask.rawValue & (1 << 2) != 0 { return "middle" }
    return "button-mask-\(mask.rawValue)"
}

/// The WindowServer window a `UIWindow` is hosted in, read the way Recon's helper reads it:
/// through the scene's AppKit bridge. Zero means UIKit has not attached a host window yet.
func hostWindowNumber(of window: UIWindow) -> Int {
    guard window.responds(to: NSSelectorFromString("nsWindowProxy")),
        let proxy = window.value(forKey: "nsWindowProxy") as? NSObject,
        proxy.responds(to: NSSelectorFromString("attachedWindow")),
        let host = proxy.value(forKey: "attachedWindow") as? NSObject,
        host.responds(to: NSSelectorFromString("windowNumber")),
        let number = host.value(forKey: "windowNumber") as? Int
    else { return 0 }
    return number
}

// MARK: - Instrumented controls

/// A button that records both the accessibility activation and the touch that reached it.
final class PressButton: UIButton {
    var label = ""
    var name = ""

    override func accessibilityActivate() -> Bool {
        Evidence.shared.record("ax-press", window: label, ["control": name])
        return super.accessibilityActivate()
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) {
        Evidence.shared.record(
            "mouse-down", window: label, touchFields(touches, event, control: name))
        super.touchesBegan(touches, with: event)
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) {
        Evidence.shared.record(
            "mouse-up", window: label, touchFields(touches, event, control: name))
        super.touchesEnded(touches, with: event)
    }

    @objc func fired(_ sender: Any?) {
        Evidence.shared.record("action", window: label, ["control": name])
    }
}

/// The target for coordinate clicks. A background Catalyst window takes its first click only over
/// a control, so the canvas is a button: a plain view there never receives the click at all. A
/// Catalyst button ends a press without calling `touchesBegan`, so the record comes from its
/// control events, which carry the event's button mask, modifiers and tap count. Its
/// context-menu interaction records a secondary click without presenting a menu: a menu would
/// reorder windows on the desktop the test is watching.
final class ClickCanvas: UIButton {
    var label = ""
    var name = ""

    func observeClicks() {
        addTarget(self, action: #selector(pressed(_:event:)), for: .touchDown)
        addTarget(self, action: #selector(released(_:event:)), for: .touchUpInside)
        addInteraction(UIContextMenuInteraction(delegate: self))
    }

    @objc private func pressed(_ sender: Any?, event: UIEvent) {
        Evidence.shared.record(
            "mouse-down", window: label,
            touchFields(event.allTouches ?? [], event, control: name))
    }

    @objc private func released(_ sender: Any?, event: UIEvent) {
        Evidence.shared.record(
            "mouse-up", window: label,
            touchFields(event.allTouches ?? [], event, control: name))
    }

    override func contextMenuInteraction(
        _ interaction: UIContextMenuInteraction,
        configurationForMenuAtLocation location: CGPoint
    ) -> UIContextMenuConfiguration? {
        Evidence.shared.record(
            "context-menu", window: label,
            ["control": name, "x": location.x, "y": location.y])
        return nil
    }
}

/// A touch as the test reads it: which control, which button, how many taps, which modifiers,
/// and where in the window's own coordinates.
func touchFields(_ touches: Set<UITouch>, _ event: UIEvent?, control: String) -> [String: Any] {
    var fields: [String: Any] = ["control": control]
    if let event {
        fields["button"] = buttonName(event.buttonMask)
        fields["buttonMask"] = event.buttonMask.rawValue
        fields["modifiers"] = modifierNames(event.modifierFlags)
    }
    if let touch = touches.first {
        fields["count"] = touch.tapCount
        let point = touch.location(in: touch.window)
        fields["x"] = point.x
        fields["y"] = point.y
    }
    return fields
}

/// A scroll view that reports its own movement. UIKit does not hand a scroll view the wheel
/// event, so the record is the offset the wheel produced, named by region.
final class ScrollRegion: UIScrollView, UIScrollViewDelegate {
    var label = ""
    var name = ""

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        Evidence.shared.record(
            "scroll-offset", window: label,
            ["control": name, "offsetX": contentOffset.x, "offsetY": contentOffset.y])
    }
}

/// A window that reports when UIKit makes it key. UIKit's key window is the target's own, and a
/// Catalyst chord is delivered only after the helper changes it inside the target.
final class InstrumentedWindow: UIWindow {
    var label = ""

    override func becomeKey() {
        super.becomeKey()
        Evidence.shared.record("key-window", window: label, ["isKey": true])
    }

    override func resignKey() {
        super.resignKey()
        Evidence.shared.record("key-window", window: label, ["isKey": false])
    }

    /// Every mouse press UIKit turns into a touch is hit-tested here first, with its button mask,
    /// even where the control under it ends the press without calling `touchesBegan`. UIKit
    /// hit-tests one press more than once, so a test reads presence, not a count.
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        if event?.type == .touches {
            Evidence.shared.record(
                "hit-test", window: label,
                ["x": point.x, "y": point.y, "buttonMask": event?.buttonMask.rawValue ?? 0])
        }
        return super.hitTest(point, with: event)
    }

    /// The same chord, declared on the window as well as on its view controller. UIKit gathers
    /// key commands from the responder chain that starts at the window's first responder and
    /// runs exactly one of them, so this is reached only when the chain has no closer handler —
    /// and a chord delivered to a window with no first responder still runs one action.
    override var keyCommands: [UIKeyCommand]? {
        let command = UIKeyCommand(
            input: "b", modifierFlags: .command, action: #selector(windowChord(_:)))
        command.wantsPriorityOverSystemBehavior = true
        return [command]
    }

    @objc private func windowChord(_ sender: Any?) {
        Evidence.shared.record("chord", window: label, ["chord": "cmd+b"])
    }
}

// MARK: - The window's own content

let contentSize = CGSize(width: 520, height: 480)

/// One window's controls and key handling. Both windows are built from this one controller, so
/// a test sends the same operation to either one and compares the two records.
final class FixtureViewController: UIViewController {
    let label: String
    private var controls: [(name: String, view: UIView)] = []
    private var outer: ScrollRegion?

    init(label: String) {
        self.label = label
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { nil }

    override var canBecomeFirstResponder: Bool { true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground

        for (name, title, x, enabled) in [
            ("record", "Record", CGFloat(24), true),
            ("disabled", "Disabled", CGFloat(152), false),
            // Two controls a selector cannot tell apart.
            ("twin-1", "Twin", CGFloat(280), true),
            ("twin-2", "Twin", CGFloat(400), true),
        ] {
            let button = PressButton(type: .system)
            button.frame = CGRect(x: x, y: 20, width: 112, height: 32)
            button.setTitle(title, for: .normal)
            button.label = label
            button.name = name
            button.isEnabled = enabled
            button.accessibilityRespondsToUserInteraction = enabled
            button.accessibilityIdentifier = name
            button.addTarget(button, action: #selector(PressButton.fired(_:)), for: .touchUpInside)
            view.addSubview(button)
            controls.append((name, button))
        }

        let canvas = ClickCanvas(type: .custom)
        canvas.frame = CGRect(x: 24, y: 104, width: 472, height: 120)
        canvas.label = label
        canvas.name = "canvas"
        canvas.backgroundColor = .secondarySystemBackground
        canvas.accessibilityLabel = "Canvas"
        canvas.observeClicks()
        view.addSubview(canvas)
        controls.append(("canvas", canvas))

        let outer = scrollRegion(
            label: label, name: "scroll-outer",
            frame: CGRect(x: 24, y: 236, width: 472, height: 220))
        view.addSubview(outer)
        controls.append(("scroll-outer", outer))
        self.outer = outer

        // The nested region sits inside the outer region's content, so a point inside it
        // addresses both: only the innermost scroll view may move, and the record says which
        // one did. It is placed clear of the outer region's own centre, which is the point
        // that addresses the outer region alone.
        let nested = scrollRegion(
            label: label, name: "scroll-nested",
            frame: CGRect(x: 780, y: 520, width: 160, height: 80))
        outer.addSubview(nested)
        controls.append(("scroll-nested", nested))
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        // Key commands are collected from the responder chain that starts at the window's own
        // first responder, so each window keeps one of its own.
        becomeFirstResponder()
        publishWhenHosted()
    }

    /// What a test addresses in this window, in points from the top left of the UIKit window.
    /// A window number exists only once UIKit has attached the scene to its host window, so the
    /// publication waits for one rather than reporting zero.
    private func publishWhenHosted() {
        guard !Evidence.shared.published(label) else { return }
        guard let window = view.window, hostWindowNumber(of: window) > 0 else {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                self?.publishWhenHosted()
            }
            return
        }
        let visible = window.convert(outer?.bounds ?? .zero, from: outer)
        let published = controls.map { control -> [String: Any] in
            var rect = window.convert(control.view.bounds, from: control.view)
            if control.name == "scroll-nested" { rect = rect.intersection(visible) }
            return [
                "name": control.name,
                "x": rect.minX,
                "y": rect.minY,
                "width": rect.width,
                "height": rect.height,
            ]
        }
        Evidence.shared.publish(window: [
            "label": label,
            "title": window.windowScene?.title ?? "",
            "windowId": hostWindowNumber(of: window),
            "contentWidth": window.bounds.width,
            "contentHeight": window.bounds.height,
            "controls": published,
        ])
    }

    override var keyCommands: [UIKeyCommand]? {
        let command = UIKeyCommand(
            input: "b", modifierFlags: .command, action: #selector(chord(_:)))
        command.wantsPriorityOverSystemBehavior = true
        return [command]
    }

    @objc private func chord(_ sender: Any?) {
        Evidence.shared.record("chord", window: label, ["chord": "cmd+b"])
    }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        record("press-begin", presses)
        super.pressesBegan(presses, with: event)
    }

    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        record("press-end", presses)
        super.pressesEnded(presses, with: event)
    }

    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        record("press-cancelled", presses)
        super.pressesCancelled(presses, with: event)
    }

    private func record(_ kind: String, _ presses: Set<UIPress>) {
        for press in presses {
            var fields: [String: Any] = ["phase": "\(press.phase.rawValue)"]
            if let key = press.key {
                fields["key"] = key.charactersIgnoringModifiers
                fields["modifiers"] = modifierNames(key.modifierFlags)
                fields["keyCode"] = key.keyCode.rawValue
            }
            Evidence.shared.record(kind, window: label, fields)
        }
    }
}

/// A scroll view whose content is larger than it is in both directions, parked in the middle so
/// that every axis and every direction has somewhere to go. Discrete wheel input counts as
/// scrolling here, so a line-unit wheel event moves it as a pixel-unit one does.
func scrollRegion(label: String, name: String, frame: CGRect) -> ScrollRegion {
    let scroll = ScrollRegion(frame: frame)
    scroll.label = label
    scroll.name = name
    scroll.delegate = scroll
    scroll.contentSize = CGSize(width: 1200, height: 1200)
    scroll.showsVerticalScrollIndicator = true
    scroll.showsHorizontalScrollIndicator = true
    scroll.panGestureRecognizer.allowedScrollTypesMask = .all
    scroll.backgroundColor = .tertiarySystemBackground
    scroll.contentOffset = CGPoint(x: 500, y: 500)
    return scroll
}

// MARK: - The scenes

let inspectorActivityType = "me.malstrom.recon.catalystfixture.inspector"

/// The fixture's windows, one per label. Saved scene state is cleared before launch, so each
/// run connects one Main scene and the one Inspector it requests. This guards the edge where a
/// scene still reconnects: a second scene of a label gets no window, so it hosts nothing the
/// helper could mistake for the window the evidence addresses.
@MainActor
enum Scenes {
    static var connected: Set<String> = []
    static var inspectorRequested = false
}

@objc(ReconCatalystSceneDelegate)
final class ReconCatalystSceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private var label: String?

    func scene(
        _ scene: UIScene, willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let windowScene = scene as? UIWindowScene else { return }
        let wanted = connectionOptions.userActivities.contains {
            $0.activityType == inspectorActivityType
        }
        let label = wanted ? "inspector" : "main"
        guard !Scenes.connected.contains(label) else { return }
        Scenes.connected.insert(label)
        self.label = label
        windowScene.title =
            label == "main" ? "Recon Catalyst Fixture — Main" : "Recon Catalyst Fixture — Inspector"
        windowScene.sizeRestrictions?.minimumSize = contentSize
        windowScene.sizeRestrictions?.maximumSize = contentSize

        let window = InstrumentedWindow(windowScene: windowScene)
        window.label = label
        window.frame = CGRect(origin: .zero, size: contentSize)
        window.rootViewController = FixtureViewController(label: label)
        // Visible but not key: UIKit's key window is what Recon's helper changes, so the
        // fixture must not set one itself.
        window.isHidden = false
        self.window = window

        if label == "main" && !Scenes.inspectorRequested {
            Scenes.inspectorRequested = true
            let activity = NSUserActivity(activityType: inspectorActivityType)
            UIApplication.shared.requestSceneSessionActivation(
                nil, userActivity: activity, options: nil, errorHandler: nil)
        }
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        guard let label else { return }
        Evidence.shared.record("active", window: label, ["active": true])
    }

    func sceneWillResignActive(_ scene: UIScene) {
        guard let label else { return }
        Evidence.shared.record("active", window: label, ["active": false])
    }
}

@objc(ReconCatalystAppDelegate)
final class ReconCatalystAppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        let configuration = UISceneConfiguration(
            name: "Default Configuration", sessionRole: connectingSceneSession.role)
        configuration.delegateClass = ReconCatalystSceneDelegate.self
        return configuration
    }
}

/// macOS restores every scene session a previous launch left connected, so the requested
/// inspector accumulates across launches into hundreds of windows. Clearing the saved state
/// before UIKit reads it makes each launch start from one Main scene, which requests one
/// Inspector: exactly the two windows the evidence addresses, with no duplicates to resolve.
func clearSavedSceneState() {
    guard let identifier = Bundle.main.bundleIdentifier else { return }
    let saved = URL(fileURLWithPath: NSHomeDirectory(), isDirectory: true)
        .appendingPathComponent("Library/Saved Application State", isDirectory: true)
    guard
        let entries = try? FileManager.default.contentsOfDirectory(
            at: saved, includingPropertiesForKeys: nil)
    else { return }
    for entry in entries where entry.lastPathComponent.hasPrefix(identifier) {
        try? FileManager.default.removeItem(at: entry)
    }
}

clearSavedSceneState()

UIApplicationMain(
    CommandLine.argc, CommandLine.unsafeArgv, nil,
    NSStringFromClass(ReconCatalystAppDelegate.self))
