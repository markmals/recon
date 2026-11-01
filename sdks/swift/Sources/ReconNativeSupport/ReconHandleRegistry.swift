import Foundation

/// Opaque handles for live target objects, scoped to one access session.
///
/// A handle names an object weakly: the registry never keeps an object alive, and a handle whose
/// object has gone answers nothing, which an action reports as a stale reference. Numbers come
/// from one process-wide counter, so a handle minted in one session is never minted again in
/// another, and closing a session revokes every handle it issued.
///
/// The lock is held for every table access because a session is revoked from the helper's server
/// thread while the main thread may be resolving handles for an in-flight read.
public final class ReconHandleRegistry: @unchecked Sendable {
    /// Beyond this many live entries a traversal is reporting more of the target than any
    /// observation consumer can use, and the session has leaked handles rather than reused them.
    public static let capacity = 100_000

    private static let counter = Counter()

    private let lock = NSLock()
    private var entries: [String: WeakObject] = [:]
    private var handles: [ObjectIdentifier: String] = [:]
    private var revoked = false

    public init() {}

    /// Whether a handle is spelled the way this registry mints them.
    public static func isWellFormed(_ handle: String) -> Bool {
        handle.count == 16 && handle.allSatisfy { $0.isHexDigit && !$0.isUppercase }
    }

    /// The handle for an object, minting one on first sight. Nil once the session is revoked or
    /// the registry is full.
    public func handle(for object: AnyObject) -> String? {
        lock.lock()
        defer { lock.unlock() }
        guard !revoked else { return nil }
        let identity = ObjectIdentifier(object)
        if let existing = handles[identity], entries[existing]?.object === object { return existing }
        guard makeRoom() else { return nil }
        let handle = Self.counter.next()
        entries[handle] = WeakObject(object)
        handles[identity] = handle
        return handle
    }

    public func object(for handle: String) -> AnyObject? {
        lock.lock()
        defer { lock.unlock() }
        return revoked ? nil : entries[handle]?.object
    }

    /// Closing a session revokes its handles. Objects an in-flight read still holds stay alive;
    /// only the naming of them ends.
    public func revoke() {
        lock.lock()
        defer { lock.unlock() }
        revoked = true
        entries.removeAll()
        handles.removeAll()
    }

    /// Sweeps entries whose objects have gone before answering that the registry is full.
    private func makeRoom() -> Bool {
        guard entries.count >= Self.capacity else { return true }
        for (handle, entry) in entries where entry.object == nil {
            entries.removeValue(forKey: handle)
        }
        handles = handles.filter { entries[$0.value] != nil }
        return entries.count < Self.capacity
    }
}

private final class WeakObject {
    weak var object: AnyObject?
    init(_ object: AnyObject) { self.object = object }
}

private final class Counter: @unchecked Sendable {
    private let lock = NSLock()
    private var issued: UInt64 = 0

    func next() -> String {
        lock.lock()
        defer { lock.unlock() }
        issued += 1
        return String(format: "%016llx", issued)
    }
}
