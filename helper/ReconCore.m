#import "ReconCore.h"
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <Security/Security.h>
#import <objc/runtime.h>
#import <math.h>
#import <mach/mach.h>
#import <sys/sysctl.h>
#import <stdatomic.h>

#ifndef RECON_HELPER_BUILD
#error "RECON_HELPER_BUILD must be the content hash embedded by build-helper.ts"
#endif
NSString *const ReconHelperBuild = @RECON_HELPER_BUILD;
NSUInteger const ReconProtocolVersion = 1;
static atomic_uint_fast64_t nextHandle = ATOMIC_VAR_INIT(0);
static atomic_bool handlesExhausted = ATOMIC_VAR_INIT(false);

@interface ReconEntry : NSObject
@property (weak) id object;
@end
@implementation ReconEntry @end

@implementation ReconRegistry {
    NSMutableDictionary<NSString *, ReconEntry *> *_entries;
    NSMapTable<id, NSString *> *_reverse;
    BOOL _revoked;
}
- (instancetype)init {
    if ((self = [super init])) {
        _entries = [NSMutableDictionary new];
        _reverse = [NSMapTable weakToStrongObjectsMapTable];
    }
    return self;
}
- (NSString *)handleForObject:(id)object {
    if (_revoked || !object || atomic_load(&handlesExhausted)) return nil;
    NSString *existing = [_reverse objectForKey:object];
    if (existing && [self objectForHandle:existing] == object) return existing;
    if (_entries.count >= 100000) {
        @autoreleasepool {
            for (NSString *key in [_entries allKeys]) {
                if (!_entries[key].object) [_entries removeObjectForKey:key];
            }
        }
        if (_entries.count >= 100000) return nil;
    }
    uint64_t number = atomic_fetch_add(&nextHandle, 1) + 1;
    if (!number) { atomic_store(&handlesExhausted, true); return nil; }
    NSString *handle = [NSString stringWithFormat:@"%016llx", (unsigned long long)number];
    ReconEntry *entry = [ReconEntry new];
    entry.object = object;
    _entries[handle] = entry;
    [_reverse setObject:handle forKey:object];
    return handle;
}
- (id)objectForHandle:(NSString *)handle {
    if (_revoked || ![handle isKindOfClass:NSString.class]) return nil;
    return _entries[handle].object;
}
- (void)revoke {
    _revoked = YES;
    [_entries removeAllObjects];
    [_reverse removeAllObjects];
}
@end

static BOOL integerInRange(id value, NSInteger minimum, NSInteger maximum) {
    if (![value isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)value) == CFBooleanGetTypeID()) return NO;
    double number = [value doubleValue];
    return isfinite(number) && number == floor(number) && number >= minimum && number <= maximum;
}

BOOL ReconValidBounds(NSDictionary *request) {
    id depth = request[@"depth"], limit = request[@"limit"], fields = request[@"fields"];
    if (depth && !integerInRange(depth, 0, 64)) return NO;
    if (limit && !integerInRange(limit, 1, 25000)) return NO;
    if (fields) {
        if (![fields isKindOfClass:NSArray.class] || [fields count] == 0) return NO;
        NSSet *allow = [NSSet setWithArray:@[ @"basic", @"appearance", @"font", @"layout", @"layer", @"cell" ]];
        NSMutableSet *seen = [NSMutableSet new];
        for (id field in fields) {
            if (![field isKindOfClass:NSString.class] || ![allow containsObject:field] || [seen containsObject:field]) return NO;
            [seen addObject:field];
        }
    }
    return YES;
}

uint32_t ReconDecodeLength(const uint8_t b[4]) {
    return ((uint32_t)b[0] << 24) | ((uint32_t)b[1] << 16) | ((uint32_t)b[2] << 8) | b[3];
}
void ReconEncodeLength(uint32_t length, uint8_t b[4]) {
    b[0] = length >> 24; b[1] = length >> 16; b[2] = length >> 8; b[3] = length;
}
BOOL ReconValidFrameLength(uint32_t length) { return length > 0 && length <= 65536; }

NSString *ReconJSONString(NSString *value, NSUInteger maxScalars) {
    if (![value isKindOfClass:NSString.class]) return @"";
    NSUInteger i = 0, scalars = 0;
    while (i < value.length && scalars < maxScalars) {
        unichar c = [value characterAtIndex:i++];
        if (CFStringIsSurrogateHighCharacter(c) && i < value.length &&
            CFStringIsSurrogateLowCharacter([value characterAtIndex:i])) i++;
        scalars++;
    }
    return i == value.length ? value : [value substringToIndex:i];
}
NSDictionary *ReconError(NSString *code, NSString *stage, NSString *message) {
    return @{ @"ok": @NO, @"error": @{ @"code": code, @"stage": stage, @"message": message } };
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
NSString *ReconKind(id object) {
    if ([object isKindOfClass:NSWindow.class]) return @"window";
    if ([object isKindOfClass:NSView.class]) return @"view";
    if ([object isKindOfClass:CALayer.class]) return @"layer";
    if ([object isKindOfClass:NSCell.class]) return @"cell";
    if ([object isKindOfClass:NSLayoutGuide.class]) return @"layoutGuide";
    return nil;
}

BOOL ReconValidHandle(id handle) {
    if (![handle isKindOfClass:NSString.class] || [handle length] != 16) return NO;
    NSCharacterSet *hex = [NSCharacterSet characterSetWithCharactersInString:@"0123456789abcdef"];
    return [handle rangeOfCharacterFromSet:hex.invertedSet].location == NSNotFound;
}

NSDictionary *ReconProcessIdentity(void) {
    NSBundle *bundle = NSBundle.mainBundle;
    NSString *path = bundle.bundlePath;
    NSString *bundlePath = [path.pathExtension isEqualToString:@"app"] ? path : nil;
    audit_token_t token;
    mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
    uint32_t pidversion = task_info(mach_task_self(), TASK_AUDIT_TOKEN, (task_info_t)&token, &count) == KERN_SUCCESS ? token.val[7] : 0;
    char build[128] = {0};
    size_t size = sizeof(build);
    if (sysctlbyname("kern.osversion", build, &size, NULL, 0) != 0) build[0] = 0;
#if defined(__arm64e__)
    NSString *architecture = @"arm64e";
#else
    NSString *architecture = @"arm64";
#endif
    return @{ @"pid": @(getpid()), @"pidversion": @(pidversion), @"uid": @(geteuid()),
        @"bundleId": bundle.bundleIdentifier ?: NSNull.null,
        @"bundlePath": bundlePath ?: NSNull.null,
        @"executablePath": bundle.executablePath ?: NSNull.null,
        @"architecture": architecture, @"osBuild": @(build),
        @"osVersion": NSProcessInfo.processInfo.operatingSystemVersionString };
}
