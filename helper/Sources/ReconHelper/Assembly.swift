import ReconAppKit
import ReconExtension

/// Every extension this helper ships, named explicitly. There is no plugin directory and no
/// runtime installation: the action set is fixed when the helper is built.
let assembledExtensions: [ReconExtensionDescriptor] = [AppKitInspector.reconExtension]
