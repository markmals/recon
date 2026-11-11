import ReconExtension
import ReconNativeSupport

/// Recon's Catalyst interactor: the one closed interaction operation `recon ui key` reaches on a
/// Mac Catalyst target, run inside that application on its own main thread.
///
/// It makes the selected window key inside the target and reports what changed. There is no read
/// here: `ui view` stays read-only, and native UIKit inspection is a separate concern this
/// extension deliberately does not begin.
@ReconExports("catalyst")
@MainActor
public enum CatalystInteractor {
    /// Makes the `UIWindow` hosted in the selected WindowServer window the target's key window.
    ///
    /// The session is the access relationship this change is made inside, so an invocation
    /// without one is refused rather than performed for nobody: a Catalyst chord requires a
    /// session precisely because it runs the target's own code.
    @ReconAction
    static func makeWindowKey(windowId: Int, context: ReconContext) throws -> KeyWindowChange {
        guard let session = ReconNativeSession.current else {
            throw ReconActionError.unavailable("No native session is open", stage: "interaction")
        }
        return try MakeWindowKey(uiKit: TargetUIKit(), session: session)
            .perform(windowId: windowId, isCancelled: { context.isCancelled })
    }
}
