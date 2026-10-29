#import "../ReconCore.h"
#import <bsm/libbsm.h>
#import <mach/mach.h>
#import <sys/socket.h>
#import <sys/stat.h>
#import <sys/un.h>
#import <unistd.h>
#include <assert.h>

extern int recon_helper_main(const char *socket_path);

static NSDictionary *exchange(const char *path, NSDictionary *message) {
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    assert(fd >= 0);
    struct sockaddr_un address = { .sun_family = AF_UNIX };
    strlcpy(address.sun_path, path, sizeof(address.sun_path));
    assert(connect(fd, (struct sockaddr *)&address, sizeof(address)) == 0);
    NSData *payload = [NSJSONSerialization dataWithJSONObject:message options:0 error:NULL];
    uint8_t header[4]; ReconEncodeLength((uint32_t)payload.length, header);
    assert(write(fd, header, sizeof(header)) == sizeof(header));
    assert(write(fd, payload.bytes, payload.length) == payload.length);
    assert(read(fd, header, sizeof(header)) == sizeof(header));
    uint32_t length = ReconDecodeLength(header);
    assert(length > 0 && length <= 8 * 1024 * 1024);
    NSMutableData *data = [NSMutableData dataWithLength:length];
    size_t offset = 0;
    while (offset < length) {
        ssize_t size = read(fd, (char *)data.mutableBytes + offset, length - offset);
        assert(size > 0); offset += size;
    }
    close(fd);
    return [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
}

int main(void) {
    @autoreleasepool {
        char directory[] = "/tmp/recon-helper-tests-XXXXXX";
        assert(mkdtemp(directory));
        assert(chmod(directory, 0700) == 0);
        char path[108];
        assert(snprintf(path, sizeof(path), "%s/s", directory) > 0);
        assert(recon_helper_main(path) == 0);
        audit_token_t token;
        mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
        assert(task_info(mach_task_self(), TASK_AUDIT_TOKEN, (task_info_t)&token, &count) == KERN_SUCCESS);
        NSDictionary *base = @{ @"protocol": @1, @"helperBuild": ReconHelperBuild,
            @"expectedPid": @(getpid()), @"expectedPidversion": @(audit_token_to_pidversion(token)),
            @"expectedUid": @(geteuid()), @"clientPid": @(getpid()),
            @"clientPidversion": @(audit_token_to_pidversion(token)), @"clientUid": @(geteuid()),
            @"profile": @"standard", @"mode": @"existing" };
        NSMutableDictionary *open = [base mutableCopy]; open[@"op"] = @"open";
        NSDictionary *first = exchange(path, open);
        assert([first[@"ok"] boolValue]);
        assert(CFGetTypeID((__bridge CFTypeRef)first[@"ok"]) == CFBooleanGetTypeID());
        assert([first[@"sessionId"] length] == 32);
        assert([first[@"identity"][@"pid"] intValue] == getpid());
        assert([first[@"identity"][@"pidversion"] unsignedIntValue] == audit_token_to_pidversion(token));
        assert([first[@"identity"][@"architecture"] isKindOfClass:NSString.class]);
        assert([first[@"identity"][@"osBuild"] isKindOfClass:NSString.class]);
        assert([first[@"capability"] length] == 64);
        NSDictionary *unauthenticated = exchange(path, open);
        assert([unauthenticated[@"error"][@"code"] isEqual:@"INSPECTION_BUSY"]);
        NSMutableDictionary *reuse = [open mutableCopy];
        reuse[@"sessionId"] = first[@"sessionId"]; reuse[@"capability"] = first[@"capability"];
        NSMutableDictionary *wrongCapability = [reuse mutableCopy];
        wrongCapability[@"capability"] = @"invalid";
        assert([exchange(path, wrongCapability)[@"error"][@"code"] isEqual:@"INSPECTION_BUSY"]);
        NSDictionary *again = exchange(path, reuse);
        assert([again[@"reused"] boolValue]);
        assert(again[@"capability"] == nil);
        assert([again[@"sessionId"] isEqual:first[@"sessionId"]]);
        NSMutableDictionary *invalidRoot = [reuse mutableCopy];
        invalidRoot[@"op"] = @"node"; invalidRoot[@"root"] = @123;
        assert([exchange(path, invalidRoot)[@"error"][@"code"] isEqual:@"INVALID_ARGUMENT"]);
        NSMutableDictionary *invalidWindow = [reuse mutableCopy];
        invalidWindow[@"op"] = @"tree"; invalidWindow[@"windowId"] = @"123";
        assert([exchange(path, invalidWindow)[@"error"][@"code"] isEqual:@"INVALID_ARGUMENT"]);
        NSMutableDictionary *reconcile = [base mutableCopy];
        reconcile[@"op"] = @"reconcile"; reconcile[@"sessionId"] = first[@"sessionId"];
        NSMutableDictionary *wrongBuild = [reconcile mutableCopy];
        wrongBuild[@"helperBuild"] = @"stale-helper-build";
        assert([exchange(path, wrongBuild)[@"error"][@"code"] isEqual:@"INSPECTION_PROTOCOL_MISMATCH"]);
        NSMutableDictionary *missingPeer = [open mutableCopy];
        [missingPeer removeObjectForKey:@"clientPidversion"];
        assert([exchange(path, missingPeer)[@"error"][@"code"] isEqual:@"STALE_SESSION"]);
        assert([exchange(path, reconcile)[@"active"] boolValue]);
        NSMutableDictionary *close = [reconcile mutableCopy];
        close[@"op"] = @"close"; close[@"capability"] = first[@"capability"];
        assert([exchange(path, close)[@"wasActive"] boolValue]);
        assert(![exchange(path, close)[@"wasActive"] boolValue]);
        assert(![exchange(path, reconcile)[@"active"] boolValue]);
        NSMutableDictionary *node = [close mutableCopy]; node[@"op"] = @"node"; node[@"root"] = @"n:invalid";
        assert([exchange(path, node)[@"error"][@"code"] isEqual:@"STALE_SESSION"]);
        NSDictionary *newSession = exchange(path, open);
        assert([newSession[@"ok"] boolValue]);
        assert(![newSession[@"sessionId"] isEqual:first[@"sessionId"]]);
        assert(![newSession[@"capability"] isEqual:first[@"capability"]]);
        unlink(path); rmdir(directory);
    }
    return 0;
}
