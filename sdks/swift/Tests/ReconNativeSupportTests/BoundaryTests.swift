import Foundation
import ObjectiveC
import Testing

@testable import ReconNativeSupport

/// What an inspected application can answer, and what Recon does with it.
///
/// Every value a target hands back is still the target's: interpreting one must not message it
/// outside the boundary, and nothing it reports may make Recon trap.
@Suite(.serialized)
struct Boundary {
    @Test func aStringThatCannotBeReadIsAFailedRead() {
        #expect(ReconNative.string(.toolTip, of: Carrier(RaisingString())) == .failed)
    }

    /// Deciding that a value is a string asks the runtime, not the object, so an object that
    /// raises when asked what it is still answers its value.
    @Test func aStringThatRefusesToSayWhatItIsIsStillRead() {
        #expect(ReconNative.string(.toolTip, of: Carrier(SecretiveString())) == .value("secret"))
    }

    @Test func aValueThatIsNotAStringIsAFailedRead() {
        #expect(ReconNative.string(.toolTip, of: Carrier(NSObject())) == .failed)
    }

    /// The copy itself is pinned by the raising-string tests: a target string read outside the
    /// boundary would raise there instead of failing the read.
    @Test func anOrdinaryStringIsReadAsItsValue() throws {
        let read = ReconNative.string(.toolTip, of: Carrier("borrowed" as NSString))
        #expect(read == .value("borrowed"))
    }

    /// A collection that reports more elements than any real one holds is refused rather than
    /// reserved for: the target decides the number, so it cannot decide an allocation.
    @Test func aCollectionThatReportsTooManyElementsIsRefused() {
        #expect(failed(Message.elements(of: lyingCollection)))
        #expect(failed(Message.elements(of: ["one", "two"] as NSArray, limit: 1)))
    }

    @Test func aCollectionIsCopiedBeforeItIsRead() throws {
        let read = Message.elements(of: ["one", "two"] as NSArray)
        guard case .value(let elements) = read else {
            Issue.record("expected the elements")
            return
        }
        #expect(elements.count == 2)
    }

    @Test func somethingThatIsNotACollectionIsRefused() {
        #expect(failed(Message.elements(of: NSObject())))
    }

    private func failed<Value>(_ read: ReconRead<Value>) -> Bool {
        if case .failed = read { return true }
        return false
    }
}

/// An object whose one readable property answers whatever the test gives it.
private final class Carrier: NSObject {
    private let answer: AnyObject

    init(_ answer: AnyObject) { self.answer = answer }

    @objc var toolTip: AnyObject { answer }
}

/// A string whose own characters cannot be read.
private final class RaisingString: NSString {
    override var length: Int {
        NSException(name: .genericException, reason: "unreadable", userInfo: nil).raise()
        return 0
    }

    override func character(at index: Int) -> unichar { 0 }
}

/// A string that raises when asked what class it is, but can be read.
private final class SecretiveString: NSString {
    private let value = "secret" as NSString

    override var length: Int { value.length }

    override func character(at index: Int) -> unichar { value.character(at: index) }

    override func isKind(of aClass: AnyClass) -> Bool {
        NSException(name: .genericException, reason: "unanswerable", userInfo: nil).raise()
        return false
    }
}

/// A collection that reports a count no process could hold. It is built at run time because
/// Swift cannot subclass `NSArray`'s class cluster, and the point is an object that answers
/// `-count` with a lie.
nonisolated(unsafe) private let lyingCollection: AnyObject = {
    let created: AnyClass = objc_allocateClassPair(NSArray.self, "ReconLyingCollection", 0)!
    let count: @convention(c) (AnyObject, Selector) -> Int = { _, _ in Int.max }
    class_addMethod(
        created, NSSelectorFromString("count"), unsafeBitCast(count, to: IMP.self), "q@:")
    let element: @convention(c) (AnyObject, Selector, Int) -> AnyObject = { _, _, _ in NSObject() }
    class_addMethod(
        created, NSSelectorFromString("objectAtIndex:"), unsafeBitCast(element, to: IMP.self),
        "@@:q")
    objc_registerClassPair(created)
    return class_createInstance(created, 0) as AnyObject
}()
