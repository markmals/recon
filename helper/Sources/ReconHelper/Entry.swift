import Darwin

/// The server this process serves from, started once by the loader's bootstrap.
private let helper = ReconHelperServer()

/// The loader's entry point, and the only symbol the injected dylib must export. `argument` names,
/// in decimal, the receive right in this task on which the loader queued the endpoint.
@_cdecl("recon_helper_main")
public func reconHelperMain(_ argument: UnsafePointer<CChar>) -> Int32 {
    // The caller is assembly, where a null pointer is representable and this is the last place
    // that can refuse it.
    guard UInt(bitPattern: UnsafeRawPointer(argument)) != 0,
        let port = mach_port_name_t(String(cString: argument))
    else { return -1 }
    return helper.start(port)
}
