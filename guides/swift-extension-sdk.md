---
id: guide.swift-extension-sdk
title: Write a Recon Extension in Swift
describes: [proposal.0006]
---

# Write a Recon Extension in Swift

## What it does

Recon's Swift extension SDK lets a first-party extension expose ordinary Swift functions to Recon. You mark a static function with `@ReconAction` inside a namespace marked `@ReconExports("<extension-id>")`. The function's own name becomes the action's name, its parameters arrive decoded from JSON through `Decodable`, and its result leaves through `Encodable`. Macros generate the descriptor, the decoding and encoding adapter, and the published schema; you do not write wire messages, a provider type, or a Rust interface.

Recon's own AppKit inspector is written this way. Its `windows`, `tree`, `node`, `find`, and `fonts` actions run inside the inspected application's helper and answer the existing `recon ui windows --session` and `recon ui view` commands with exactly the payloads those commands already published.

The approach follows [Neon](https://neon-rs.dev/docs/functions/), which exports ordinary Rust functions to JavaScript: explicit exports, ordinary native-language functions, host-compatible data, and generated plumbing. It does not copy JavaScript values, garbage-collected handles, classes, promises, or callbacks, because Recon is a command-line host, not a JavaScript engine.

## Before you start

- Build on an Apple-silicon Mac running macOS 27.
- Use Apple's Swift 6.4 compiler build `swiftlang-6.4.0.34.1`, from Xcode 27.1 or the Command Line Tools. Recon requires that exact build for every build, including Homebrew's. The swift.org toolchain, including the one `swiftly` or `mise` installs, cannot build the helper's arm64e slice. Check the active compiler with `mise run swift:toolchain`; it fails and names the compiler it found when the build differs.
- The first build resolves swift-syntax 604.0.0 for the macros, so it needs network access or an already-resolved checkout.

## How to use it

### Describe your data with Codable models

Use ordinary `Codable` types for inputs and results. Add `@ReconModel` to a model whose Codable conformance Swift synthesizes, and the SDK generates its published schema from the declaration:

- a struct's stored properties, each with an explicit type annotation;
- the struct's own `CodingKeys`, including renamed and omitted members;
- optional properties, which may be absent from the JSON but are never `null`;
- enums with a `String` raw type and no associated values.

`@ReconModel` stops with a compile error where it cannot describe the encoded shape truthfully: a custom `init(from:)` or `encode(to:)` in the type's body, a `let` constant with an initial value (Swift encodes it but never decodes it), `lazy` or property-wrapped properties, generic types, and enums with associated values. For a type with a custom codec, leave out `@ReconModel` and conform to `ReconSchema` yourself, stating the shape the codec actually produces:

```swift
import ReconExtension

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
```

`String`, `Bool`, the fixed-width integer types, `Double`, `Float`, arrays, and dictionaries with `String` keys already have schemas. Integer schemas carry the type's bounds, so a value that would overflow a `UInt32` parameter is rejected before your function runs.

### Export actions from a namespace

Declare an enum or struct marked `@ReconExports` with your extension's identifier, and mark each action inside its body with a bare `@ReconAction`. The namespace is never instantiated; it only groups the actions and names the extension.

```swift
@ReconExports("demo")
enum Demo {
    @ReconAction
    static func greeting(_ name: String, excited: Bool = false) throws -> Greeting {
        guard !name.isEmpty else { throw ReconActionError.invalidArgument("name must not be empty") }
        return Greeting(message: "Hello, \(name)", excited: excited ? true : nil)
    }
}
```

The macro turns each signature into an action contract:

- **Name.** The function's base name is the action name, exactly: `greeting`. Renaming the function renames the action. Names must be lowerCamelCase ASCII letters and digits. The extension identifier must be lowercase ASCII letters, digits, and hyphens, starting with a letter.
- **Input.** The input is one JSON object with a member for each parameter, named by the argument label or, for a parameter without a label, by the parameter's own name. `greeting` takes `{"name": "Ada"}` or `{"name": "Ada", "excited": true}`. A parameter that is optional or has a default value is an optional member: leave it out to get `nil` or the default. Members the signature does not name, missing required members, wrong types, out-of-range integers, and `null` where the schema does not allow it are rejected as `INVALID_ARGUMENT` before your function runs.
- **Result.** Return an `Encodable` value with a schema, or return nothing to answer an empty object.
- **Errors.** Throw `ReconActionError` to report a failure with one of Recon's existing classifications. Any other thrown error is reported as `INSPECTION_FAILED`; it need not be `Codable`.

| You throw | Recon reports |
| --- | --- |
| `ReconActionError.invalidArgument(message)` | `INVALID_ARGUMENT` |
| `ReconActionError.staleReference(message)` | `STALE_NODE` |
| `ReconActionError.unavailable(message)` | `INSPECTION_UNAVAILABLE` |
| `ReconActionError.cancelled`, or what `context.checkCancellation()` throws | `INSPECTION_TIMEOUT` |
| any other error | `INSPECTION_FAILED` |

A result that cannot be encoded, such as a `Double.nan`, fails as `INSPECTION_FAILED` at the encoding stage. Recon never runs the action a second time to retry.

### Use the context for deadlines and cancellation

Add a parameter of type `ReconContext` when an action loops or waits. It is supplied by Recon, never by the caller, and it is not part of the input object. It can appear anywhere in the parameter list, at most once.

```swift
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
```

Its input is `{"upTo": 3}`: the argument label names the member.

`checkCancellation()` throws once the invocation's deadline has passed or its session has been closed. Cancellation is cooperative: Recon cannot interrupt a getter that is already running, and it never rolls work back. When an action has already started and its deadline passes, Recon reports `INSPECTION_TIMEOUT` with `targetExecutionMayContinue: true`. An action still waiting to start when its session closes or its deadline passes never starts.

### Choose isolation

An action is `nonisolated` unless it or its namespace is `@MainActor`. Mark the namespace `@MainActor` when its actions read live AppKit state; Recon then runs each action on the target application's main thread, and it commits an action as started there only while the action's session is still open. Mark an individual action `nonisolated` to opt it out of a `@MainActor` namespace. Actions may be `async`. Other global actors are rejected at compile time.

### Read native objects only through native-support accessors

An Objective-C getter can raise an exception, and an Objective-C exception must never unwind through Swift code. Read target objects through the allowlisted accessors in `ReconNativeSupport`, which call the getter and catch any exception in Objective-C before control returns to your Swift code:

```swift
import ReconNativeSupport

func title(of window: AnyObject) -> String? {
    switch ReconNative.string(.title, of: window) {
    case .value(let title): title
    case .unavailable, .failed: nil
    }
}
```

`.unavailable` means the object does not have the value; `.failed` means reading it failed, including by raising an exception. Keep the distinction in your result models, as Recon's AppKit observations do. Accessors are an allowlist: there is no accessor that takes a selector, key path, or expression, and a caller can never supply one. Adding an accessor family is a change to the SDK's native support, not something an extension does for itself. `ReconNativeSupport` is for actions that run inside an inspected application; the rest of the SDK does not import AppKit.

### Test with Swift Testing

Test an extension with ordinary Swift tools. `ReconExtensionTesting` provides `ReconTestHost`, which assembles your extension the way the helper does and runs each invocation through the real runtime: input validation, isolation, error classification, and encoding.

The runtime validates inputs against their schema, but it does not re-check results. Every result `ReconTestHost` returns has been checked against the action's published schema, and a result that does not match throws `ReconSchemaMismatch` and fails the test. This is how a `CodingKeys` or custom codec that disagrees with its schema is caught, so give each action tests that exercise the shapes its results can take.

```swift
import ReconExtensionTesting
import Testing

@testable import Demo

@Test func greetsByName() throws {
    let host = try ReconTestHost(Demo.reconExtension)
    let greeting = try host.invoke("greeting", #"{"name":"Ada","excited":true}"#).decode(Greeting.self)
    #expect(greeting == Greeting(message: "Hello, Ada", excited: true))
}

@Test func rejectsAnEmptyName() throws {
    let host = try ReconTestHost(Demo.reconExtension)
    #expect(try host.invoke("greeting", #"{"name":""}"#).failure?.code == "INVALID_ARGUMENT")
}
```

Call `invoke` from a test that is not `@MainActor`: it waits for the action, and a `@MainActor` action needs the main thread to be free. `ReconTestHost` rejects two extensions with the same identifier, exactly as the helper's assembly does; equal action names in different extensions are fine.

### Assemble the extension into the helper

Recon ships extensions inside its target helper; there is no plugin directory and no runtime installation. Add your extension's package as a dependency of the helper package in `helper/Package.swift`, and add its descriptor to the explicit list in `helper/Sources/ReconHelper/Assembly.swift`:

```swift
let assembledExtensions = [AppKitInspector.reconExtension, Demo.reconExtension]
```

Then build the universal helper and its build identity:

```sh
mise run helper:build
```

The build identity covers your sources, the resolved package versions, the compiler, linker, and SDK, and the build configuration, but not the checkout path or the time. A helper only talks to the `recon` built with the same identity, and an application that already has an older helper loaded must be restarted before it can use the new one.

### Expose an action as a command

Exporting an action does not create a command. Recon's command registry decides which actions the CLI reaches, with what arguments, help, schemas, effects, and errors. The existing `recon ui windows --session` and `recon ui view tree`, `node`, `find`, and `fonts` commands map to the AppKit actions `windows`, `tree`, `node`, `find`, and `fonts`. A new public command for another action needs its own design.

## Worked example

A package with an extension target `Demo` that depends on the SDK's `ReconExtension` product, and a test target that depends on `Demo` and `ReconExtensionTesting`:

```swift
import ReconExtension

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
```

Invoking `greeting` with `{"name":"Ada"}` answers `{"message":"Hello, Ada"}`: `excited` is `nil`, so it is absent rather than `null`. The published input schema is `{"type":"object","properties":{"name":{"type":"string"},"excited":{"type":"boolean"}},"required":["name"],"additionalProperties":false}`. Invoking it with `{"name":"Ada","shout":true}` fails with `INVALID_ARGUMENT` before the function runs, because `shout` is not a parameter.

## Limits and edge cases

- An action must be a `static` function in the body of its `@ReconExports` type. A free function, an instance method, or an action declared in an `extension` of the namespace is a compile error rather than an action that is silently not exported.
- Two annotated functions with the same base name in one extension are a compile error, even when their parameters differ. Variadic, `inout`, and generic actions are not supported.
- A type without `Codable` conformance or a schema fails to compile with the type named, for example `static method 'of' requires that 'Date' conform to 'ReconSchema'`.
- Members are never `null` unless a schema says so. An optional property is absent when `nil`; elements of an array of optionals, such as `[String?]`, can be `null`.
- The order of members in an encoded object is not significant and is not stable; compare decoded values, not JSON text.
- Encoding and decoding, including your custom codec code, run with the action's own isolation, deadline, and exception rules. `Codable` does not make an Objective-C getter safe to call.
- Extensions are trusted code compiled into the helper. Using the SDK grants no access beyond what the helper's session already allows, and the action set is fixed when the helper is built: a client cannot name an action that was not exported, a selector, or code to run.
