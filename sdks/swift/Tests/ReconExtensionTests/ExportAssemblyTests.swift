import ReconExtensionRuntime
import Testing

extension ProposalSDK {
    @Suite
    struct ExportAssembly {
        @Test func bareAnnotationsExportInferredNamesInDeclarationOrder() {
            #expect(Demo.reconExtension.identifier == "demo")
            #expect(
                Demo.reconExtension.actions.map(\.name) == [
                    "greeting", "measure", "survey", "reference", "fail", "unencodable",
                    "misdeclared", "pause", "nothing", "bulk",
                ])
        }

        @Test func equalLocalNamesInDistinctExtensionsAreDistinctActions() {
            #expect(
                json(invoke("demo", "greeting", #"{"name":"Mark"}"#))
                    == #"{"message":"Hello, Mark"}"#)
            #expect(
                json(invoke("other", "greeting", #"{"name":"Mark"}"#)) == #"{"message":"Hi, Mark"}"#)
        }

        @Test func duplicateExtensionIdentifiersAreRejectedAtAssembly() {
            #expect(throws: ReconAssemblyError.duplicateExtension("demo")) {
                try ReconRuntime(assembling: [Demo.reconExtension, Demo.reconExtension])
            }
        }

        @Test func undeclaredAndUnannotatedNamesAreNotInvocable() {
            #expect(failure(invoke("demo", "helper", "{}"))?.code == "INVALID_ARGUMENT")
            #expect(failure(invoke("demo", "helper", "{}"))?.stage == "operation")
            #expect(failure(invoke("missing", "greeting", "{}"))?.stage == "operation")
        }
    }
}
