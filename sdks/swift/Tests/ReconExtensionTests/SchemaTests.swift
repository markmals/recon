import ReconExtension
import Testing

extension ProposalSDK {
    @Suite
    struct GeneratedSchemas {
        func schema(_ extensionIdentifier: String, _ name: String) -> (input: String, output: String)
        {
            let action = runtime.action(extensionIdentifier, name)!
            return (action.input.jsonSchema, action.output.jsonSchema)
        }

        @Test func synthesizedModelsFollowCodingKeysAndOptionality() {
            #expect(
                schema("demo", "measure").output
                    == #"{"type":"object","properties":{"id":{"type":"string"},"ratio":{"type":"number"},"count":{"type":"integer","minimum":-9223372036854775808,"maximum":9223372036854775807},"windowId":{"type":"integer","minimum":0,"maximum":4294967295},"note":{"type":"string"}},"required":["id","ratio","count","windowId"],"additionalProperties":false}"#
            )
        }

        @Test func parametersBecomeAStrictInputObject() {
            #expect(
                schema("demo", "survey").input
                    == #"{"type":"object","properties":{"points":{"type":"array","items":{"type":"array","items":{"type":"number"}}},"limit":{"type":"integer","minimum":-9223372036854775808,"maximum":9223372036854775807}},"required":["points"],"additionalProperties":false}"#
            )
        }

        @Test func customCodecsUseTheirStatedSchemaAndEnumsTheirRawValues() {
            #expect(
                schema("demo", "survey").output
                    == #"{"type":"object","properties":{"points":{"type":"array","items":{"type":"array","items":{"type":"number"}}},"completeness":{"enum":["complete","truncated-by-limit"]},"tags":{"type":"object","additionalProperties":{"type":"integer","minimum":-9223372036854775808,"maximum":9223372036854775807}},"maybe":{"type":"array","items":{"anyOf":[{"type":"string"},{"type":"null"}]}}},"required":["points","completeness","tags","maybe"],"additionalProperties":false}"#
            )
        }

        /// A value with alternative shapes publishes them; it does not publish the one its last
        /// encoding happened to take.
        @Test func unionsPublishEveryAlternative() {
            #expect(
                schema("demo", "reference").output
                    == #"{"anyOf":[{"type":"string"},{"type":"object","properties":{"status":{"enum":["absent"]}},"required":["status"],"additionalProperties":false}]}"#
            )
        }

        @Test func anOptionalWholePayloadIsNullable() {
            #expect(ReconSchemaNode.of(String?.self).jsonSchema
                == #"{"anyOf":[{"type":"string"},{"type":"null"}]}"#)
        }
    }
}
