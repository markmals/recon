import CoreGraphics
import ReconExtension
import ReconNativeSupport

/// One rectangle in one named coordinate space. A rectangle with a non-finite edge is not a
/// rectangle at an unknown position: it is the absence of one.
@ReconModel
struct Rect: Codable, Equatable {
    let status: StatusName
    let space: String?
    let units: String?
    let orientation: String?
    let x: Double?
    let y: Double?
    let width: Double?
    let height: Double?

    init(_ rect: CGRect, space: String, orientation: String?) {
        let finite =
            rect.origin.x.isFinite && rect.origin.y.isFinite && rect.size.width.isFinite
            && rect.size.height.isFinite
        status = finite ? .value : .nonFinite
        self.space = finite ? space : nil
        units = finite ? "logical-points" : nil
        self.orientation = finite ? (orientation ?? "unknown") : nil
        x = finite ? rect.origin.x : nil
        y = finite ? rect.origin.y : nil
        width = finite ? rect.size.width : nil
        height = finite ? rect.size.height : nil
    }

    private init(_ status: StatusName) {
        self.status = status
        space = nil
        units = nil
        orientation = nil
        x = nil
        y = nil
        width = nil
        height = nil
    }

    /// A screen height Recon could not read is the absence of the transform, not a rectangle at
    /// an unknown position.
    static let unavailable = Rect(.unavailable)

    static func screenTopLeft(_ rect: CGRect, primaryScreenHeight: Double) -> Rect {
        guard primaryScreenHeight.isFinite else { return .unavailable }
        return Rect(
            ReconNative.screenTopLeft(rect, primaryScreenHeight: primaryScreenHeight),
            space: "global-screen", orientation: "top-left")
    }
}

/// The transformation a top-left rectangle was produced with. `scale` is the display's backing
/// scale as metadata; it is reported, never applied.
@ReconModel
struct ScreenTransform: Codable, Equatable {
    let status: StatusName
    let method: String?
    let orientation: String?
    let primaryScreenHeight: Double?
    let displayId: Int?
    let scale: Double?

    init(displayId: Int?, scale: Double, primaryScreenHeight: Double) {
        guard primaryScreenHeight.isFinite else {
            status = .unavailable
            method = nil
            orientation = nil
            self.primaryScreenHeight = nil
            self.displayId = nil
            self.scale = nil
            return
        }
        status = .value
        method = "primary-screen-height-flip"
        orientation = "top-left"
        self.primaryScreenHeight = primaryScreenHeight
        self.displayId = displayId
        self.scale = scale.isFinite ? scale : nil
    }
}

/// A rectangle's position in the spaces a consumer correlates against: its window, the global
/// screen, and accessibility's top-left origin.
@ReconModel
struct NormalizedFrame: Codable, Equatable {
    let status: StatusName?
    let window: Rect?
    let screen: Rect?
    let screenTopLeft: Rect?
    let backingScaleFactor: NumberValue?
    let displayId: DisplayIdentifier?
    let transform: ScreenTransform?

    /// A view in no window has no normalization to report.
    static let absent = NormalizedFrame(
        status: .absent, window: nil, screen: nil, screenTopLeft: nil, backingScaleFactor: nil,
        displayId: nil, transform: nil)

    init(
        status: StatusName? = nil, window: Rect? = nil, screen: Rect? = nil, screenTopLeft: Rect?,
        backingScaleFactor: NumberValue?, displayId: DisplayIdentifier?,
        transform: ScreenTransform?
    ) {
        self.status = status
        self.window = window
        self.screen = screen
        self.screenTopLeft = screenTopLeft
        self.backingScaleFactor = backingScaleFactor
        self.displayId = displayId
        self.transform = transform
    }
}
