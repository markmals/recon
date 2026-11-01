// Every declaration here is copied from `guides/swift-extension-sdk.md`. The guide is the
// contract: if an example stops compiling or stops behaving as the guide prints, one of the two
// is wrong, and `GuideBehaviourTests` says which.
import ReconExtension
import ReconNativeSupport

struct Point: Codable, ReconSchema {
    let x: Double
    let y: Double

    init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }

    // Encodes as [x, y] rather than {"x": …, "y": …}.
    init(from decoder: any Decoder) throws {
        var values = try decoder.unkeyedContainer()
        x = try values.decode(Double.self)
        y = try values.decode(Double.self)
    }

    func encode(to encoder: any Encoder) throws {
        var values = encoder.unkeyedContainer()
        try values.encode(x)
        try values.encode(y)
    }

    static var reconSchema: ReconSchemaNode { .array(.number) }
}

@ReconModel
struct Greeting: Codable, Equatable {
    let message: String
    var excited: Bool?
}

@ReconExports("demo")
enum Demo {
    @ReconAction
    static func greeting(_ name: String, excited: Bool = false) throws -> Greeting {
        guard !name.isEmpty else { throw ReconActionError.invalidArgument("name must not be empty") }
        return Greeting(message: "Hello, \(name)", excited: excited ? true : nil)
    }
}

@ReconExports("counter")
enum Counter {
    @ReconAction
    static func count(upTo limit: Int, context: ReconContext) throws -> Int {
        var total = 0
        for _ in 0..<limit {
            try context.checkCancellation()
            total += 1
        }
        return total
    }
}

func title(of window: AnyObject) -> String? {
    switch ReconNative.string(.title, of: window) {
    case .value(let title): title
    case .unavailable, .failed: nil
    }
}
