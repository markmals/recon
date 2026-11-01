import Foundation
import ReconNativeSupport
import Testing

@Suite(.serialized)
struct NativeHandles {
    @Test func aHandleIsSixteenLowercaseHexDigitsAndIsReusedForOneObject() {
        let registry = ReconHandleRegistry()
        let object = Subject()
        let handle = registry.handle(for: object)
        #expect(handle?.count == 16)
        #expect(ReconHandleRegistry.isWellFormed(handle ?? ""))
        #expect(!ReconHandleRegistry.isWellFormed("ABCDEF1234567890"))
        #expect(!ReconHandleRegistry.isWellFormed("abcdef12345678900"))
        #expect(!ReconHandleRegistry.isWellFormed("n:invalid"))
        #expect(registry.handle(for: object) == handle)
        #expect(registry.object(for: handle ?? "") === object)
    }

    @Test func aHandleGoesStaleWithItsObjectAndIsNeverMintedAgain() {
        let registry = ReconHandleRegistry()
        var first: String?
        autoreleasepool {
            let object = Subject()
            first = registry.handle(for: object)
        }
        #expect(registry.object(for: first ?? "") == nil)
        let replacement = Subject()
        let second = registry.handle(for: replacement)
        #expect(second != first)
        #expect(registry.object(for: first ?? "") == nil)
    }

    @Test func revokingASessionRevokesItsHandles() {
        let registry = ReconHandleRegistry()
        let object = Subject()
        let handle = registry.handle(for: object)
        registry.revoke()
        #expect(registry.object(for: handle ?? "") == nil)
        #expect(registry.handle(for: object) == nil)
    }

    @Test func anotherSessionNeverReissuesAHandle() {
        let object = Subject()
        let first = ReconHandleRegistry().handle(for: object)
        let second = ReconHandleRegistry().handle(for: object)
        #expect(first != second)
    }
}

@Suite(.serialized)
struct NativeSessions {
    @Test func openingAndClosingASessionOwnsItsHandles() throws {
        ReconNativeSession.open(identifier: "one", helperBuild: "build", protocolVersion: 1)
        let session = try #require(ReconNativeSession.current)
        #expect(session.identifier == "one")
        #expect(session.helperBuild == "build")
        #expect(session.protocolVersion == 1)
        let handle = try #require(session.handles.handle(for: Subject()))

        ReconNativeSession.close(identifier: "other")
        #expect(ReconNativeSession.current != nil)

        ReconNativeSession.close(identifier: "one")
        #expect(ReconNativeSession.current == nil)
        #expect(session.handles.object(for: handle) == nil)
    }

    @Test func reopeningTheSameSessionKeepsItsHandles() throws {
        ReconNativeSession.open(identifier: "two", helperBuild: "build", protocolVersion: 1)
        let session = try #require(ReconNativeSession.current)
        let subject = Subject()
        let handle = try #require(session.handles.handle(for: subject))
        ReconNativeSession.open(identifier: "two", helperBuild: "build", protocolVersion: 1)
        #expect(ReconNativeSession.current?.handles.object(for: handle) === subject)
        ReconNativeSession.close(identifier: "two")
    }
}

@Suite
struct NativeReads {
    @Test func aRaisingGetterIsCaughtBeforeSwiftSeesIt() {
        #expect(ReconNative.string(.toolTip, of: Raising()) == .failed)
        #expect(ReconNative.string(.toolTip, of: Subject()) == .unavailable)
        #expect(ReconNative.string(.toolTip, of: Tipped()) == .value("tip"))
    }

    @Test func theProcessIdentityDescribesThisProcess() {
        let identity = ReconProcessIdentity.current
        #expect(identity.pid == Int(ProcessInfo.processInfo.processIdentifier))
        #expect(["arm64", "arm64e", "unknown"].contains(identity.architecture))
        #expect(!identity.osBuild.isEmpty)
    }
}

private final class Subject: NSObject {}

private final class Tipped: NSObject {
    @objc var toolTip: String { "tip" }
}

private final class Raising: NSObject {
    @objc var toolTip: String {
        NSException(name: .genericException, reason: "unreadable", userInfo: nil).raise()
        return ""
    }
}
