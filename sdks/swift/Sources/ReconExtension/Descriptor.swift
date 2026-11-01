import Foundation

/// How the runtime must schedule an action so its Swift isolation is preserved.
public enum ReconInvocation: Sendable {
    case nonisolated(@Sendable (Data, ReconContext) throws -> Data)
    case nonisolatedAsync(@Sendable (Data, ReconContext) async throws -> Data)
    case mainActor(@MainActor @Sendable (Data, ReconContext) throws -> Data)
    case mainActorAsync(@MainActor @Sendable (Data, ReconContext) async throws -> Data)
}

/// One exported action: its name, its published payload schemas, and its adapter.
public struct ReconActionDescriptor: Sendable {
    public let name: String
    public let input: ReconSchemaNode
    public let output: ReconSchemaNode
    public let invocation: ReconInvocation

    public init(
        name: String, input: ReconSchemaNode, output: ReconSchemaNode, invocation: ReconInvocation
    ) {
        self.name = name
        self.input = input
        self.output = output
        self.invocation = invocation
    }
}

/// One extension's identifier and the actions `@ReconExports` collected from its body.
public struct ReconExtensionDescriptor: Sendable {
    public let identifier: String
    public let actions: [ReconActionDescriptor]

    public init(identifier: String, actions: [ReconActionDescriptor]) {
        self.identifier = identifier
        self.actions = actions
    }
}
