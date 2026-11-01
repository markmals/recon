import Security

/// 32 bytes of secure randomness in hexadecimal, or nil when the system has none to give.
func reconToken() -> String? {
    var bytes = [UInt8](repeating: 0, count: 32)
    guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
        return nil
    }
    let digits = Array("0123456789abcdef".utf8)
    var text = [UInt8]()
    text.reserveCapacity(bytes.count * 2)
    for byte in bytes {
        text.append(digits[Int(byte >> 4)])
        text.append(digits[Int(byte & 15)])
    }
    return String(decoding: text, as: UTF8.self)
}
