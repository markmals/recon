#import "ReconCore.h"
#import <Security/Security.h>

#ifndef RECON_HELPER_BUILD
#error "RECON_HELPER_BUILD must be the content hash embedded by build-helper.ts"
#endif
NSString *const ReconHelperBuild = @RECON_HELPER_BUILD;
NSUInteger const ReconProtocolVersion = 1;

uint32_t ReconDecodeLength(const uint8_t b[4]) {
    return ((uint32_t)b[0] << 24) | ((uint32_t)b[1] << 16) | ((uint32_t)b[2] << 8) | b[3];
}
void ReconEncodeLength(uint32_t length, uint8_t b[4]) {
    b[0] = length >> 24; b[1] = length >> 16; b[2] = length >> 8; b[3] = length;
}
BOOL ReconValidFrameLength(uint32_t length) { return length > 0 && length <= 65536; }

NSDictionary *ReconError(NSString *code, NSString *stage, NSString *message) {
    return @{ @"ok": (NSNumber *)kCFBooleanFalse, @"error": @{ @"code": code, @"stage": stage, @"message": message } };
}
NSString *ReconToken(void) {
    uint8_t bytes[32];
    if (SecRandomCopyBytes(kSecRandomDefault, sizeof(bytes), bytes) != errSecSuccess) return nil;
    static const char hex[] = "0123456789abcdef";
    char text[65];
    for (NSUInteger i = 0; i < sizeof(bytes); i++) {
        text[i * 2] = hex[bytes[i] >> 4]; text[i * 2 + 1] = hex[bytes[i] & 15];
    }
    text[64] = 0;
    return [NSString stringWithUTF8String:text];
}
