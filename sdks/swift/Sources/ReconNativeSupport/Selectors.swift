import AppKit
import Foundation

/// The allowlist: every selector Recon may send, and the class it may be sent to.
///
/// AppKit is imported here for its class objects and its attribute keys, which are constants
/// rather than anything read from the inspected application. Every read of a live object still
/// goes through `Message`.
enum Fields {
    static let count = NSSelectorFromString("count")
    static let objectAtIndex = NSSelectorFromString("objectAtIndex:")
    static let objectForKey = NSSelectorFromString("objectForKey:")
    static let doubleValue = NSSelectorFromString("doubleValue")
    static let integerValue = NSSelectorFromString("integerValue")
    static let localizedName = NSSelectorFromString("localizedName")
    static let name = NSSelectorFromString("name")

    static let sharedApplication = NSSelectorFromString("sharedApplication")
    static let applicationWindows = NSSelectorFromString("windows")
    static let windowWithWindowNumber = NSSelectorFromString("windowWithWindowNumber:")
    static let screens = NSSelectorFromString("screens")
    static let frame = NSSelectorFromString("frame")
    static let deviceDescription = NSSelectorFromString("deviceDescription")
    static let convertRectToView = NSSelectorFromString("convertRect:toView:")
    static let convertRectToScreen = NSSelectorFromString("convertRectToScreen:")
    static let bounds = NSSelectorFromString("bounds")
    static let window = NSSelectorFromString("window")

    static let mainBundle = NSSelectorFromString("mainBundle")
    static let bundlePath = NSSelectorFromString("bundlePath")
    static let bundleIdentifier = NSSelectorFromString("bundleIdentifier")
    static let executablePath = NSSelectorFromString("executablePath")

    static let colorSpace = NSSelectorFromString("colorSpace")
    static let catalogNameComponent = NSSelectorFromString("catalogNameComponent")
    static let colorNameComponent = NSSelectorFromString("colorNameComponent")
    static let deviceRGBColorSpace = NSSelectorFromString("deviceRGBColorSpace")
    static let colorUsingColorSpace = NSSelectorFromString("colorUsingColorSpace:")
    static let colorWithCGColor = NSSelectorFromString("colorWithCGColor:")
    static let redComponent = NSSelectorFromString("redComponent")
    static let greenComponent = NSSelectorFromString("greenComponent")
    static let blueComponent = NSSelectorFromString("blueComponent")
    static let alphaComponent = NSSelectorFromString("alphaComponent")

    static let font = NSSelectorFromString("font")
    static let familyName = NSSelectorFromString("familyName")
    static let fontName = NSSelectorFromString("fontName")
    static let pointSize = NSSelectorFromString("pointSize")
    static let fontDescriptor = NSSelectorFromString("fontDescriptor")
    static let symbolicTraits = NSSelectorFromString("symbolicTraits")
    static let postscriptName = NSSelectorFromString("postscriptName")
    static let attributeAtIndex = NSSelectorFromString(
        "attribute:atIndex:longestEffectiveRange:inRange:")

    static let firstItem = NSSelectorFromString("firstItem")
    static let secondItem = NSSelectorFromString("secondItem")
    static let firstAttribute = NSSelectorFromString("firstAttribute")
    static let secondAttribute = NSSelectorFromString("secondAttribute")
    static let relation = NSSelectorFromString("relation")
    static let multiplier = NSSelectorFromString("multiplier")
    static let constant = NSSelectorFromString("constant")
    static let priority = NSSelectorFromString("priority")
    static let isActive = NSSelectorFromString("isActive")
    static let identifier = NSSelectorFromString("identifier")

    static let transform = NSSelectorFromString("transform")
}

/// Where one read gets its value: the selector to send, and the class that must own the object
/// for the read to apply at all. An object of another class has no such value, which is not the
/// same as a getter that answered nothing.
struct FieldBinding {
    let selector: Selector
    let owner: ReconNativeClass?

    init(_ selector: Selector, _ owner: ReconNativeClass? = nil) {
        self.selector = selector
        self.owner = owner
    }
}
