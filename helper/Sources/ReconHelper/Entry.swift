import Darwin

/// The server this process serves from, started once by the loader's bootstrap.
private let helper = ReconHelperServer()

/// The loader's entry point, and the only symbol the injected dylib must export.
@_cdecl("recon_helper_main")
public func reconHelperMain(_ path: UnsafePointer<CChar>) -> Int32 {
    // The caller is assembly, where a null pointer is representable and this is the last place
    // that can refuse it.
    guard UInt(bitPattern: UnsafeRawPointer(path)) != 0 else { return -1 }
    return helper.start(String(cString: path))
}
