/// What one allowlisted read of a live target object answered.
///
/// `unavailable` means the object does not have the value; `failed` means reading it failed,
/// including by raising an Objective-C exception that native support caught.
///
/// A read can answer a live framework object, which is neither `Sendable` nor serializable; an
/// action turns it into its own data before it leaves the target's main thread.
public enum ReconRead<Value> {
    case value(Value)
    case unavailable
    case failed

    public var value: Value? {
        guard case .value(let value) = self else { return nil }
        return value
    }

    public func map<Mapped>(_ transform: (Value) -> Mapped) -> ReconRead<Mapped> {
        switch self {
        case .value(let value): .value(transform(value))
        case .unavailable: .unavailable
        case .failed: .failed
        }
    }
}

extension ReconRead: Equatable where Value: Equatable {}
