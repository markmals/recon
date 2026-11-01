import CoreGraphics
import Foundation
import QuartzCore
import ReconNativeAccessors

/// One message to one object, through the Objective-C boundary that catches what it raises.
///
/// Nothing outside this file sends a message to an inspected application's object, and nothing
/// here decides which messages may be sent: `Fields` names the selectors, and the public
/// `ReconNative` surface names the reads.
enum Message {
    static func object(_ selector: Selector, to target: AnyObject) -> ReconRead<AnyObject> {
        var result: AnyObject?
        return read(ReconSendObject(target, selector, &result)) { result }
    }

    static func object(_ selector: Selector, to target: AnyObject, with argument: AnyObject?)
        -> ReconRead<AnyObject>
    {
        var result: AnyObject?
        return read(ReconSendObjectTo(target, selector, argument, &result)) { result }
    }

    static func object(_ selector: Selector, to target: AnyObject, at index: Int)
        -> ReconRead<AnyObject>
    {
        var result: AnyObject?
        return read(ReconSendObjectAt(target, selector, index, &result)) { result }
    }

    /// A Core Foundation result, which Core Animation's colours are.
    static func reference(_ selector: Selector, to target: AnyObject)
        -> ReconRead<UnsafeRawPointer>
    {
        var result: UnsafeRawPointer?
        return read(ReconSendReference(target, selector, &result)) { result }
    }

    static func flag(_ selector: Selector, to target: AnyObject) -> ReconRead<Bool> {
        var result = ObjCBool(false)
        return read(ReconSendBool(target, selector, &result)) { result.boolValue }
    }

    /// Whether the object implements a getter at all, which it may answer for itself.
    static func responds(to question: Selector, _ target: AnyObject) -> Bool {
        var result = ObjCBool(false)
        let status = ReconSendResponds(
            target, NSSelectorFromString("respondsToSelector:"), question, &result)
        return status == .value && result.boolValue
    }

    static func integer(_ selector: Selector, to target: AnyObject) -> ReconRead<Int> {
        var result = 0
        return read(ReconSendInteger(target, selector, &result)) { result }
    }

    static func double(_ selector: Selector, to target: AnyObject) -> ReconRead<Double> {
        var result = 0.0
        return read(ReconSendDouble(target, selector, &result)) { result }
    }

    /// A getter that answers a single-precision value returns it in a different register than a
    /// double, so the two cannot share one trampoline.
    static func float(_ selector: Selector, to target: AnyObject) -> ReconRead<Double> {
        var result: Float = 0
        return read(ReconSendFloat(target, selector, &result)) { Double(result) }
    }

    static func float(_ selector: Selector, to target: AnyObject, for argument: Int)
        -> ReconRead<Double>
    {
        var result: Float = 0
        return read(ReconSendFloatFor(target, selector, argument, &result)) { Double(result) }
    }

    static func rect(_ selector: Selector, to target: AnyObject) -> ReconRead<CGRect> {
        var result = CGRect.zero
        return read(ReconSendRect(target, selector, &result)) { result }
    }

    static func rect(_ selector: Selector, to target: AnyObject, of rect: CGRect)
        -> ReconRead<CGRect>
    {
        var result = CGRect.zero
        return read(ReconSendRectOf(target, selector, rect, &result)) { result }
    }

    static func rect(
        _ selector: Selector, to target: AnyObject, of rect: CGRect, in view: AnyObject?
    ) -> ReconRead<CGRect> {
        var result = CGRect.zero
        return read(ReconSendRectOfIn(target, selector, rect, view, &result)) { result }
    }

    static func size(_ selector: Selector, to target: AnyObject) -> ReconRead<CGSize> {
        var result = CGSize.zero
        return read(ReconSendSize(target, selector, &result)) { result }
    }

    static func transform(_ selector: Selector, to target: AnyObject) -> ReconRead<CATransform3D> {
        var result = CATransform3D()
        return read(ReconSendTransform(target, selector, &result)) { result }
    }

    /// The longest run of one attribute from an index, which answers both a value and its range.
    static func attribute(
        _ selector: Selector, to target: AnyObject, named name: AnyObject, at index: Int,
        within limit: NSRange
    ) -> ReconRead<(value: AnyObject?, range: NSRange)> {
        var result: AnyObject?
        var effective = NSRange()
        let status = ReconSendAttribute(
            target, selector, name, UInt(index), limit, &effective, &result)
        switch status {
        case .value: return .value((result, effective))
        case .unavailable: return .unavailable
        default: return .failed
        }
    }

    /// The longest string one read publishes. Every observation bounds what it reports far
    /// below this; the bound here is what stops a target that claims a length no process could
    /// hold from deciding an allocation.
    static let maximumStringScalars = 1 << 20
    /// The most elements a collection may report. No collection a read reaches — an
    /// application's windows, a view's subviews or constraints, a layer's sublayers — comes
    /// near it, and the read's own visit budget stops at 25,000 long before.
    static let maximumElements = 1 << 20

    /// A string the object answered, verified and copied inside the boundary. A value that is
    /// not a string is a failed read, not an absent one: the object answered something Recon
    /// cannot publish.
    static func text(_ selector: Selector, to target: AnyObject) -> ReconRead<String> {
        var result: NSString?
        return read(ReconSendString(target, selector, UInt(maximumStringScalars), &result)) {
            result as String?
        }
    }

    /// The elements of a collection the object answered, snapshotted inside the boundary so
    /// that reading them messages the copy rather than the target.
    static func elements(of collection: AnyObject, limit: Int = maximumElements)
        -> ReconRead<[AnyObject]>
    {
        var snapshot: NSArray?
        guard ReconCopyElements(collection, UInt(limit), &snapshot) == .value,
            let snapshot
        else { return .failed }
        var elements: [AnyObject] = []
        elements.reserveCapacity(snapshot.count)
        for index in 0..<snapshot.count {
            elements.append(snapshot.object(at: index) as AnyObject)
        }
        return .value(elements)
    }

    private static func read<Value>(_ status: ReconSendStatus, _ value: () -> Value?)
        -> ReconRead<Value>
    {
        switch status {
        case .value: value().map { ReconRead.value($0) } ?? .unavailable
        case .unavailable: .unavailable
        default: .failed
        }
    }
}
