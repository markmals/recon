/// Marks a static function of a `@ReconExports` type as an action. Its base name is the export
/// name. The attribute only validates; `@ReconExports` generates the descriptor.
@attached(peer)
public macro ReconAction() = #externalMacro(module: "ReconExtensionMacros", type: "ReconActionMacro")

/// Names an extension and assembles every `@ReconAction` declared in this type's body.
@attached(member, names: arbitrary)
public macro ReconExports(_ identifier: String) =
    #externalMacro(module: "ReconExtensionMacros", type: "ReconExportsMacro")

/// Generates `ReconSchema` for a model whose Codable conformance is synthesized.
@attached(extension, conformances: ReconSchema, names: named(reconSchema))
public macro ReconModel() = #externalMacro(module: "ReconExtensionMacros", type: "ReconModelMacro")
