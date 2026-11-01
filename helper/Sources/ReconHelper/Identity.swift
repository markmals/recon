import Darwin
import Foundation
import MachO

/// What this helper speaks, and what it was built from.
///
/// The build identity is the content hash of every input the build consumed, linked into the
/// image as `__TEXT,__recon_build`. A request that names a different build is refused rather
/// than answered from the wrong code, and an image built without the section names no build at
/// all: the protocol gate then refuses every request instead of inventing an identity.
enum ReconHelperIdentity {
    static let protocolVersion = 1
    static let build = linkedBuild()
}

private func linkedBuild() -> String {
    let anchor: @convention(c) () -> Void = imageAnchor
    var image = Dl_info()
    guard dladdr(unsafeBitCast(anchor, to: UnsafeRawPointer.self), &image) != 0,
        let header = image.dli_fbase?.assumingMemoryBound(to: mach_header_64.self)
    else { return "" }
    var size = UInt(0)
    guard let section = getsectiondata(header, "__TEXT", "__recon_build", &size) else { return "" }
    let text = String(
        decoding: UnsafeRawBufferPointer(start: section, count: Int(size)), as: UTF8.self)
    return text.trimmingCharacters(in: .whitespacesAndNewlines)
}

/// `dladdr` answers for the image a symbol lives in, which is how the helper finds its own.
private func imageAnchor() {}
