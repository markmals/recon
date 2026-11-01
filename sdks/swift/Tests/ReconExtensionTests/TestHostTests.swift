import ReconExtensionRuntime
import ReconExtensionTesting
import Testing

extension ProposalSDK {
    @Suite
    struct TestHost {
        @Test func invocationsRunThroughTheRealRuntime() throws {
            let host = try ReconTestHost(Demo.reconExtension)
            let greeting = try host.invoke("greeting", #"{"name":"Ada"}"#).decode(Greeting.self)
            #expect(greeting == Greeting(message: "Hello, Ada"))
            #expect(
                try host.invoke("greeting", #"{"name":"Ada","shout":true}"#).failure?.code
                    == "INVALID_ARGUMENT")
        }

        @Test func duplicateExtensionIdentifiersAreRejected() {
            #expect(throws: ReconAssemblyError.duplicateExtension("demo")) {
                try ReconTestHost(Demo.reconExtension, Demo.reconExtension)
            }
        }

        @Test func equalActionNamesInDifferentExtensionsAreAddressable() throws {
            let host = try ReconTestHost(Demo.reconExtension, Other.reconExtension)
            #expect(
                try host.invoke("greeting", #"{"name":"Ada"}"#, in: "other").decode(Greeting.self)
                    == Greeting(message: "Hi, Ada"))
            #expect(
                try host.invoke("greeting", #"{"name":"Ada"}"#).decode(Greeting.self)
                    == Greeting(message: "Hello, Ada"))
        }

        /// The runtime does not re-parse results; this check is what keeps a custom codec and
        /// the schema it publishes from drifting apart.
        @Test func aResultThatContradictsItsSchemaFailsTheTest() throws {
            let host = try ReconTestHost(Demo.reconExtension)
            #expect(throws: ReconSchemaMismatch.self) { try host.invoke("misdeclared") }
        }

        @Test func conformingResultsPass() throws {
            let host = try ReconTestHost(Demo.reconExtension)
            #expect(try host.invoke("reference").decode(Reference.self) == Reference(nil))
            #expect(
                try host.invoke("reference", #"{"handle":"000000000000000a"}"#)
                    .decode(Reference.self) == Reference("000000000000000a"))
        }

        @Test func mainActorActionsRunWhileTheTestWaits() throws {
            let host = try ReconTestHost(MainDemo.reconExtension)
            let report = try host.invoke("whereAmI").decode(ThreadReport.self)
            #expect(report.mainThread)
        }

        @Test func anUndeclaredNameIsNotInvocable() throws {
            let host = try ReconTestHost(Demo.reconExtension)
            #expect(try host.invoke("helper").failure?.code == "INVALID_ARGUMENT")
        }
    }
}
