import AppKit

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

        // The window ordered front last stays the application's main window, and observers
        // expect that to be the main window rather than the inspector. The fixture never
        // activates itself, so `open -g` leaves the frontmost application untouched.
        for window in [makeInspectorWindow(), makeMainWindow()] {
            openWindows.append(window)
            window.orderFront(nil)
        }
    }
}

func makeMainWindow() -> NSWindow {
    let window = makeWindow(
        title: "Recon Fixture — Main",
        frame: NSRect(x: 160, y: 240, width: 480, height: 320)
    )

    let record = NSButton(title: "Record", target: nil, action: nil)
    record.frame = NSRect(x: 24, y: 232, width: 120, height: 32)

    let value = NSTextField(string: "fixture value")
    value.frame = NSRect(x: 24, y: 176, width: 260, height: 24)

    let enabled = NSButton(checkboxWithTitle: "Enabled", target: nil, action: nil)
    enabled.frame = NSRect(x: 24, y: 128, width: 160, height: 24)

    for control in [record, value, enabled] as [NSView] {
        window.contentView?.addSubview(control)
    }
    return window
}

func makeInspectorWindow() -> NSWindow {
    makeWindow(
        title: "Recon Fixture — Inspector",
        frame: NSRect(x: 700, y: 300, width: 320, height: 220)
    )
}

func makeWindow(title: String, frame: NSRect) -> NSWindow {
    let window = NSWindow(
        contentRect: frame,
        styleMask: [.titled, .closable, .miniaturizable, .resizable],
        backing: .buffered,
        defer: false
    )
    window.title = title
    window.isReleasedWhenClosed = false
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
