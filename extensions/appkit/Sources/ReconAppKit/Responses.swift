import ReconExtension

/// The application's windows, each with the WindowServer identifier it could be matched to.
@ReconModel
struct WindowsResponse: Codable, Equatable {
    let windows: [NodeRecord]
    let observation: Observation
    let coverage: Coverage
}

/// One named object.
@ReconModel
struct NodeResponse: Codable, Equatable {
    let node: NodeRecord
    let observation: Observation
    let coverage: Coverage
}

/// A traversal: the nodes it reached, and the windows those nodes hang from.
@ReconModel
struct NodesResponse: Codable, Equatable {
    let nodes: [NodeRecord]
    let windowRoots: [NodeRecord]
    let observation: Observation
    let coverage: Coverage
}

/// One font, and every place in the traversal that uses it.
@ReconModel
struct FontUse: Codable, Equatable {
    let handle: String
    let kind: String
    let source: String
    let range: TextRange?
}

@ReconModel
struct FontGroup: Codable, Equatable {
    let descriptor: FontDescriptor
    let sources: [FontUse]
}

@ReconModel
struct FontsResponse: Codable, Equatable {
    let fonts: [FontGroup]
    let windowRoots: [NodeRecord]
    let observation: Observation
    let coverage: Coverage
}
