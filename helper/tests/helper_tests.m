#import "../Sources/ReconHelperServer/ReconCore.h"
#import <Foundation/Foundation.h>
#include <assert.h>

static void testFrames(void) {
    uint8_t b[4];
    for (uint32_t size = 0; size <= 65538; size++) {
        ReconEncodeLength(size, b);
        assert(ReconDecodeLength(b) == size);
        assert(ReconValidFrameLength(size) == (size > 0 && size <= 65536));
    }
}

static void testTokens(void) {
    NSString *token = ReconToken();
    assert(token.length == 64);
    NSCharacterSet *hex = [NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdef"];
    assert([token rangeOfCharacterFromSet:hex.invertedSet].location == NSNotFound);
    assert(![token isEqualToString:ReconToken()]);
}

static void testErrors(void) {
    NSDictionary *error = ReconError(@"STALE_SESSION", @"read", @"Session authentication failed");
    assert(CFGetTypeID((__bridge CFTypeRef)error[@"ok"]) == CFBooleanGetTypeID());
    assert(![error[@"ok"] boolValue]);
    assert([error[@"error"][@"code"] isEqualToString:@"STALE_SESSION"]);
    assert([error[@"error"][@"stage"] isEqualToString:@"read"]);
    assert([NSJSONSerialization isValidJSONObject:error]);
}

int main(void) {
    @autoreleasepool { testFrames(); testTokens(); testErrors(); }
    return 0;
}
