import ReconExtension
import ReconNativeSupport

/// The breadth-first walk every traversing read shares.
///
/// It visits each object once, reports a repeat as a reference to the row that described it,
/// and records every bound it hits in the read's coverage.
extension Work {
    func traverse(_ scope: Scope) throws -> Traversal {
        try checkRootKind(scope.root)
        var traversal = Traversal()
        try seedRoots(scope, into: &traversal)
        try walk(&traversal)
        if traversal.index < traversal.queue.count { note(truncated: "unsearched-scope") }
        if expired {
            throw ReconActionError.cancelled
        }
        return traversal
    }

    /// A traversal's state: what is still to visit, what has been reported, and what repeated.
    struct Traversal {
        var queue: [(object: AnyObject, depth: Int)] = []
        var index = 0
        var records: [NodeRecord] = []
        var windowRoots: [NodeRecord] = []
        var fonts = FontIndex()
        var seen: Set<ObjectIdentifier> = []
    }

    private func checkRootKind(_ root: AnyObject?) throws {
        guard let root else { return }
        let kind = ReconNative.kind(of: root)
        let compatible =
            request.traversesLayers
            ? [.window, .view, .layer].contains(kind) : [.window, .view].contains(kind)
        guard compatible else {
            throw ReconActionError.invalidArgument(
                "Root kind is incompatible with traversal", stage: "root")
        }
    }

    private func seedRoots(_ scope: Scope, into traversal: inout Traversal) throws {
        if let selected = scope.root ?? scope.window {
            if let window = scope.window
                ?? (ReconNative.kind(of: selected) == .window ? selected : nil)
            {
                traversal.windowRoots.append(try record(window))
            }
            if let first = try traversalRoot(selected) {
                traversal.queue.append((first, 0))
            } else {
                note(narrowed: "selected-scope:absent-content-or-layer")
            }
            return
        }
        for candidate in scope.windows {
            if !consume() { break }
            traversal.windowRoots.append(try record(candidate))
            if let content = try related(.windowContentView, of: candidate) {
                traversal.queue.append((content, 0))
            }
            if request.operation != .find, traversal.windowRoots.count >= limit {
                if traversal.windowRoots.count < scope.windows.count {
                    note(truncated: "windows:limit")
                }
                break
            }
        }
    }

    private func walk(_ traversal: inout Traversal) throws {
        while traversal.index < traversal.queue.count, !expired {
            let entry = traversal.queue[traversal.index]
            traversal.index += 1
            if !consume() { break }
            if traversal.seen.contains(ObjectIdentifier(entry.object)) {
                if try !reportRepeat(entry, into: &traversal) { break }
                continue
            }
            traversal.seen.insert(ObjectIdentifier(entry.object))
            try report(entry, into: &traversal)
            if emitted >= limit {
                if traversal.index < traversal.queue.count || entry.depth < depth {
                    note(truncated: "returned:limit")
                }
                break
            }
            try enqueueChildren(of: entry, into: &traversal)
        }
    }

    /// Reports a repeat, and answers whether the walk may continue.
    private func reportRepeat(
        _ entry: (object: AnyObject, depth: Int), into traversal: inout Traversal
    ) throws -> Bool {
        if request.operation == .fonts { return true }
        if request.operation == .find, try !request.matches(entry.object) { return true }
        traversal.records.append(try repeatedRecord(entry.object, depth: entry.depth))
        emitted += 1
        guard emitted >= limit else { return true }
        if traversal.index < traversal.queue.count { note(truncated: "returned:limit") }
        return false
    }

    private func report(
        _ entry: (object: AnyObject, depth: Int), into traversal: inout Traversal
    ) throws {
        var include = true
        if request.operation == .find { include = try request.matches(entry.object) }
        if include, request.operation != .fonts {
            traversal.records.append(try record(entry.object, depth: entry.depth))
            emitted += 1
        }
        if request.operation == .fonts {
            try traversal.fonts.add(try font(entry.object), of: entry.object, in: self)
        }
    }

    private func enqueueChildren(
        of entry: (object: AnyObject, depth: Int), into traversal: inout Traversal
    ) throws {
        let children = try children(of: entry.object)
        guard entry.depth < depth else {
            if !children.isEmpty { note(narrowed: "depth") }
            return
        }
        for child in children {
            // A tree read stops queueing once the queue alone would exceed what it may return,
            // because every queued node would otherwise be visited for nothing.
            if request.operation == .tree,
                traversal.queue.count - traversal.index >= limit - emitted
            {
                note(truncated: "returned:limit")
                note(truncated: "unsearched-scope")
                break
            }
            if !consume() { break }
            traversal.queue.append((child, entry.depth + 1))
        }
    }
}

/// Fonts grouped by the descriptor they share, in the order the walk first met them.
struct FontIndex {
    private struct Key: Hashable {
        let postScriptName: String?
        let pointSize: Double?
        let symbolicTraits: Int?
        let weight: Double?
    }

    private var order: [Key] = []
    private var groups: [Key: FontGroup] = [:]

    var all: [FontGroup] { order.compactMap { groups[$0] } }

    mutating func add(_ facet: FontFacet, of object: AnyObject, in work: Work) throws {
        for use in Self.uses(in: facet) {
            let key = Key(
                postScriptName: use.descriptor.postScriptName,
                pointSize: use.descriptor.pointSize?.number,
                symbolicTraits: use.descriptor.symbolicTraits,
                weight: use.descriptor.weight?.number)
            let source = FontUse(
                handle: work.reference(object) ?? "",
                kind: ReconNative.kind(of: object).publishedName ?? "", source: use.source,
                range: use.range)
            if groups[key] == nil {
                order.append(key)
                groups[key] = FontGroup(descriptor: use.descriptor, sources: [source])
            } else {
                groups[key] = FontGroup(
                    descriptor: groups[key]!.descriptor, sources: groups[key]!.sources + [source])
            }
            work.emitted += 1
            if work.emitted >= work.limit { break }
        }
    }

    private static func uses(in facet: FontFacet) -> [(descriptor: FontDescriptor, source: String,
        range: TextRange?)]
    {
        var uses: [(FontDescriptor, String, TextRange?)] = []
        if let carrier = facet.carrier, carrier.status == .value, let source = facet.source {
            uses.append((carrier, source, nil))
        }
        for run in facet.runs ?? [] { uses.append((run.font, "attributed-run", run.range)) }
        return uses
    }
}
