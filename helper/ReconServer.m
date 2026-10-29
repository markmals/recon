#import "ReconInspect.h"
#import <AppKit/AppKit.h>
#import <bsm/libbsm.h>
#import <mach/mach.h>
#import <mach/mach_time.h>
#import <pthread.h>
#import <stdatomic.h>
#import <sys/socket.h>
#import <sys/stat.h>
#import <sys/un.h>
#import <unistd.h>
#import <math.h>

static NSLock *stateLock;
static NSString *activeSession, *activeCapability, *activeProfile, *establishedMode, *lastClosed;
static ReconRegistry *activeRegistry;
static atomic_bool reading = ATOMIC_VAR_INIT(false);
static uint32_t ownPidversion;
static atomic_bool serverStarted = ATOMIC_VAR_INIT(false);
static uint64_t nowNs(void) {
    static mach_timebase_info_data_t scale;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ mach_timebase_info(&scale); });
    return mach_continuous_time() * scale.numer / scale.denom;
}
static BOOL transfer(int fd, uint8_t *bytes, size_t count, BOOL sending) {
    while (count) {
        ssize_t n = sending ? send(fd, bytes, count, 0) : recv(fd, bytes, count, 0);
        if (n <= 0) { if (n < 0 && errno == EINTR) continue; return NO; }
        bytes += n; count -= (size_t)n;
    }
    return YES;
}
static void sendResponse(int fd, NSDictionary *response) {
    NSData *json = [NSJSONSerialization dataWithJSONObject:response options:0 error:NULL];
    if (!json || json.length > 8 * 1024 * 1024) {
        json = [NSJSONSerialization dataWithJSONObject:ReconError(@"INSPECTION_UNAVAILABLE", @"response-size", @"Native response exceeds 8 MiB") options:0 error:NULL];
    }
    uint8_t length[4]; ReconEncodeLength((uint32_t)json.length, length);
    if (!transfer(fd, length, 4, YES)) return;
    transfer(fd, (uint8_t *)json.bytes, json.length, YES);
}
static NSDictionary *validateIdentity(NSDictionary *request) {
    NSNumber *pid = request[@"expectedPid"], *version = request[@"expectedPidversion"], *uid = request[@"expectedUid"];
    if (![pid isKindOfClass:NSNumber.class] || ![version isKindOfClass:NSNumber.class] || ![uid isKindOfClass:NSNumber.class])
        return ReconError(@"INVALID_ARGUMENT", @"identity", @"Expected process identity is required");
    if (pid.intValue != getpid() || version.unsignedIntValue != ownPidversion || uid.unsignedIntValue != geteuid())
        return ReconError(@"STALE_SESSION", @"identity", @"Process incarnation or owner changed");
    return nil;
}
static NSDictionary *handleLifecycle(NSDictionary *request) {
    NSString *op = request[@"op"], *sessionId = request[@"sessionId"];
    [stateLock lock];
    NSDictionary *response = nil;
    if ([op isEqual:@"reconcile"]) {
        if (![sessionId isKindOfClass:NSString.class] || !sessionId.length)
            response = ReconError(@"INVALID_ARGUMENT", @"reconcile", @"Session identity is required");
        else response = @{ @"ok": (NSNumber *)kCFBooleanTrue, @"active": @([activeSession isEqual:sessionId]),
            @"outstandingRead": @(atomic_load(&reading)), @"sessionId": [activeSession isEqual:sessionId] ? activeSession : NSNull.null,
            @"incarnation": @{ @"pid": @(getpid()), @"pidversion": @(ownPidversion) } };
    } else if ([op isEqual:@"close"]) {
        if (![sessionId isKindOfClass:NSString.class] || !sessionId.length)
            response = ReconError(@"INVALID_ARGUMENT", @"close", @"Session identity is required");
        else if ([activeSession isEqual:sessionId]) {
            if (![activeCapability isEqual:request[@"capability"]])
                response = ReconError(@"STALE_SESSION", @"close", @"Session authentication failed");
            else {
                lastClosed = activeSession;
                activeSession = nil; activeCapability = nil; activeProfile = nil; establishedMode = nil;
                activeRegistry = nil;
                response = @{ @"ok": (NSNumber *)kCFBooleanTrue, @"revoked": (NSNumber *)kCFBooleanTrue, @"wasActive": (NSNumber *)kCFBooleanTrue,
                    @"helperResident": (NSNumber *)kCFBooleanTrue, @"bootstrapActive": (NSNumber *)kCFBooleanTrue };
            }
        } else if ([lastClosed isEqual:sessionId])
            response = @{ @"ok": (NSNumber *)kCFBooleanTrue, @"revoked": (NSNumber *)kCFBooleanTrue, @"wasActive": (NSNumber *)kCFBooleanFalse,
                @"helperResident": (NSNumber *)kCFBooleanTrue, @"bootstrapActive": (NSNumber *)kCFBooleanTrue };
        else response = ReconError(@"STALE_SESSION", @"close", @"Session is not known to this process");
    } else if ([op isEqual:@"open"]) {
        response = validateIdentity(request);
        NSString *profile = request[@"profile"], *mode = request[@"mode"];
        if (!response && (![profile isKindOfClass:NSString.class] || ![@[ @"standard", @"research" ] containsObject:profile] ||
                          ![mode isKindOfClass:NSString.class] || ![@[ @"existing", @"launch", @"attach" ] containsObject:mode]))
            response = ReconError(@"INVALID_ARGUMENT", @"open", @"Requested profile and mode are required");
        if (!response && activeSession && (![activeProfile isEqual:profile] || [mode isEqual:@"launch"]))
            response = ReconError(@"INSPECTION_BUSY", @"open", @"An incompatible inspection session is active");
        if (!response && activeSession &&
            (![activeSession isEqual:request[@"sessionId"]] || ![activeCapability isEqual:request[@"capability"]]))
            response = ReconError(@"INSPECTION_BUSY", @"open", @"An inspection session is already active");
        if (!response && !activeSession && (request[@"sessionId"] || request[@"capability"]))
            response = ReconError(@"STALE_SESSION", @"open", @"Session authentication failed");
        if (!response) {
            BOOL reused = activeSession != nil;
            if (!reused) {
                activeSession = [ReconToken() substringToIndex:32]; activeCapability = ReconToken();
                if (!activeSession || !activeCapability) response = ReconError(@"INSPECTION_FAILED", @"random", @"Secure randomness is unavailable");
                else { activeRegistry = [ReconRegistry new]; activeProfile = profile; establishedMode = mode; }
            }
            if (!response) {
                NSMutableDictionary *opened = [@{ @"ok": (NSNumber *)kCFBooleanTrue, @"sessionId": activeSession, @"reused": @(reused),
                @"pid": @(getpid()), @"pidversion": @(ownPidversion), @"uid": @(geteuid()),
                @"incarnation": @{ @"pid": @(getpid()), @"pidversion": @(ownPidversion) },
                @"identity": ReconProcessIdentity(),
                @"helperBuild": ReconHelperBuild, @"protocol": @(ReconProtocolVersion),
                @"profile": activeProfile, @"mode": mode, @"requestedMode": mode,
                @"requestedProfile": profile, @"establishedMode": establishedMode,
                @"capabilities": @[ @"windows", @"tree", @"node", @"find", @"fonts" ],
                @"effects": @[ @"helper-bootstrap-resident" ],
                @"helperResident": (NSNumber *)kCFBooleanTrue, @"bootstrapActive": (NSNumber *)kCFBooleanTrue } mutableCopy];
                if (!reused) opened[@"capability"] = activeCapability;
                response = opened;
            }
        }
    }
    [stateLock unlock];
    return response;
}
static NSDictionary *handleRead(NSDictionary *request, uint64_t requestStart) {
    [stateLock lock];
    BOOL authenticated = activeSession && [activeSession isEqual:request[@"sessionId"]] &&
        [activeCapability isEqual:request[@"capability"]];
    ReconRegistry *registry = authenticated ? activeRegistry : nil;
    NSString *session = authenticated ? activeSession : nil;
    [stateLock unlock];
    if (!authenticated) return ReconError(@"STALE_SESSION", @"read", @"Session authentication failed");
    if (!ReconValidBounds(request)) return ReconError(@"INVALID_ARGUMENT", @"bounds", @"Invalid depth, limit, or fields");
    NSString *op = request[@"op"], *kind = request[@"kind"];
    if (kind && (![kind isKindOfClass:NSString.class] || ![@[ @"view", @"layer" ] containsObject:kind]))
        return ReconError(@"INVALID_ARGUMENT", @"kind", @"Traversal kind must be view or layer");
    if (([op isEqual:@"find"] || [op isEqual:@"tree"] || [op isEqual:@"fonts"]) &&
        (request[@"class"] && ![request[@"class"] isKindOfClass:NSString.class] ||
         request[@"identifier"] && ![request[@"identifier"] isKindOfClass:NSString.class]))
        return ReconError(@"INVALID_ARGUMENT", @"filters", @"Exact filters must be strings");
    if (request[@"root"] && !ReconValidHandle(request[@"root"]))
        return ReconError(@"INVALID_ARGUMENT", @"root", @"Malformed native handle");
    if ([op isEqual:@"node"] && !request[@"root"])
        return ReconError(@"INVALID_ARGUMENT", @"root", @"Native handle is required");
    id windowId = request[@"windowId"];
    if (windowId) {
        if (![windowId isKindOfClass:NSNumber.class] || CFGetTypeID((__bridge CFTypeRef)windowId) == CFBooleanGetTypeID() ||
            !isfinite([windowId doubleValue]) || [windowId doubleValue] != floor([windowId doubleValue]) ||
            [windowId doubleValue] < 1 || [windowId doubleValue] > UINT32_MAX)
            return ReconError(@"INVALID_ARGUMENT", @"window", @"WindowServer ID must be a positive 32-bit integer");
    }
    if ([op isEqual:@"find"] && !request[@"class"] && !request[@"identifier"])
        return ReconError(@"INVALID_ARGUMENT", @"filters", @"Find requires an exact class or identifier");
    if (nowNs() >= requestStart + 2000000000ULL)
        return ReconError(@"INSPECTION_TIMEOUT", @"transport", @"Native request exceeded its deadline before dispatch");
    bool expected = false;
    if (!atomic_compare_exchange_strong(&reading, &expected, true))
        return ReconError(@"INSPECTION_BUSY", @"target-execution", @"A prior native read has not finished");
    ReconWork *work = [ReconWork new]; work.registry = registry; work.request = request;
    work.sessionId = session; work.depth = request[@"depth"] ? [request[@"depth"] unsignedIntegerValue] : 3;
    work.limit = request[@"limit"] ? [request[@"limit"] unsignedIntegerValue] : 200;
    work.deadline = requestStart + 2000000000ULL;
    work.startedAt = NSDate.date.timeIntervalSince1970;
    dispatch_semaphore_t done = dispatch_semaphore_create(0);
    __block NSDictionary *answer = nil;
    __block atomic_bool started = ATOMIC_VAR_INIT(false);
    dispatch_async(dispatch_get_main_queue(), ^{
        @autoreleasepool {
            if (nowNs() < work.deadline) {
                atomic_store(&started, true);
                @try { answer = ReconInspect(work); }
                @catch (NSException *exception) {
                    answer = ReconError(@"INSPECTION_FAILED", @"target-execution", @"Native getter raised an exception");
                }
            }
            atomic_store(&reading, false);
            dispatch_semaphore_signal(done);
        }
    });
    uint64_t remaining = nowNs() < work.deadline ? work.deadline - nowNs() : 0;
    if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, (int64_t)remaining)))
        return @{ @"ok": (NSNumber *)kCFBooleanFalse, @"error": @{ @"code": @"INSPECTION_TIMEOUT",
            @"message": @"Native read exceeded its deadline", @"stage": atomic_load(&started) ? @"target-execution" : @"main-thread-queue",
            @"targetExecutionMayContinue": @(atomic_load(&started)) } };
    [stateLock lock]; BOOL current = [activeSession isEqual:session] && [activeCapability isEqual:request[@"capability"]]; [stateLock unlock];
    return current ? (answer ?: ReconError(@"INSPECTION_TIMEOUT", @"main-thread-queue", @"Queued native read expired")) :
        ReconError(@"STALE_SESSION", @"read", @"Session closed during native read");
}
static NSDictionary *dispatchRequest(NSDictionary *request, uint64_t requestStart) {
    NSString *op = request[@"op"];
    if (![op isKindOfClass:NSString.class]) return ReconError(@"INVALID_ARGUMENT", @"request", @"Operation is required");
    BOOL recovery = [op isEqual:@"reconcile"] || [op isEqual:@"close"];
    if (![request[@"protocol"] isKindOfClass:NSNumber.class] || [request[@"protocol"] integerValue] != ReconProtocolVersion ||
        ![request[@"helperBuild"] isKindOfClass:NSString.class] || ![request[@"helperBuild"] isEqual:ReconHelperBuild])
        return ReconError(@"INSPECTION_PROTOCOL_MISMATCH", @"protocol", @"Helper build or wire protocol differs");
    if ([op isEqual:@"open"] || recovery) return handleLifecycle(request);
    if ([@[ @"windows", @"tree", @"node", @"find", @"fonts" ] containsObject:op]) return handleRead(request, requestStart);
    return ReconError(@"INVALID_ARGUMENT", @"operation", @"Unsupported closed operation");
}
static void *serveClient(void *context) {
    int fd = (int)(intptr_t)context;
    @autoreleasepool {
        NSMutableData *data = nil;
        NSDictionary *request = nil;
        NSNumber *clientPid = nil, *clientVersion = nil, *clientUid = nil;
        audit_token_t token;
        socklen_t length = sizeof(token);
        if (getsockopt(fd, SOL_LOCAL, LOCAL_PEERTOKEN, &token, &length) != 0 || length != sizeof(token) ||
            audit_token_to_euid(token) != geteuid()) goto finish;
        struct timeval timeout = { .tv_sec = 10 };
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout));
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof(timeout));
        uint64_t requestStart = nowNs();
        uint8_t header[4];
        if (!transfer(fd, header, 4, NO)) goto finish;
        uint32_t size = ReconDecodeLength(header);
        if (!ReconValidFrameLength(size)) {
            sendResponse(fd, ReconError(@"INVALID_ARGUMENT", @"framing", @"Request exceeds 64 KiB or is empty"));
            goto finish;
        }
        data = [NSMutableData dataWithLength:size];
        if (!transfer(fd, data.mutableBytes, size, NO)) goto finish;
        request = [NSJSONSerialization JSONObjectWithData:data options:0 error:NULL];
        if (![request isKindOfClass:NSDictionary.class]) {
            sendResponse(fd, ReconError(@"INVALID_ARGUMENT", @"json", @"Request must be one JSON object"));
            goto finish;
        }
        clientPid = request[@"clientPid"]; clientVersion = request[@"clientPidversion"]; clientUid = request[@"clientUid"];
        if (![clientPid isKindOfClass:NSNumber.class] || clientPid.intValue != audit_token_to_pid(token) ||
            ![clientVersion isKindOfClass:NSNumber.class] || clientVersion.unsignedIntValue != audit_token_to_pidversion(token) ||
            ![clientUid isKindOfClass:NSNumber.class] || clientUid.unsignedIntValue != audit_token_to_euid(token)) {
            sendResponse(fd, ReconError(@"STALE_SESSION", @"peer", @"Peer process incarnation changed"));
            goto finish;
        }
        sendResponse(fd, dispatchRequest(request, requestStart));
    finish:
        close(fd);
    }
    return NULL;
}
struct server_context { int listener; char path[sizeof(((struct sockaddr_un *)0)->sun_path)]; };
static void *acceptClients(void *arg) {
    struct server_context *context = arg;
    int listener = context->listener;
    @autoreleasepool {
        for (;;) {
            int client = accept(listener, NULL, NULL);
            if (client < 0) { if (errno == EINTR) continue; break; }
            int nosigpipe = 1;
            setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &nosigpipe, sizeof(nosigpipe));
            pthread_t thread;
            if (pthread_create(&thread, NULL, serveClient, (void *)(intptr_t)client) == 0) pthread_detach(thread);
            else close(client);
        }
    }
    close(listener);
    unlink(context->path);
    free(context);
    return NULL;
}
static int startupFailed(void) {
    atomic_store(&serverStarted, false);
    return -1;
}
int recon_server_main(const char *path) {
    if (!path || !*path || strlen(path) >= sizeof(((struct sockaddr_un *)0)->sun_path)) return -1;
    bool expected = false;
    if (!atomic_compare_exchange_strong(&serverStarted, &expected, true)) return -1;
    @autoreleasepool {
        NSString *directory = [[NSString stringWithUTF8String:path] stringByDeletingLastPathComponent];
        struct stat dir, endpoint;
        if (lstat(directory.fileSystemRepresentation, &dir) != 0 || !S_ISDIR(dir.st_mode) ||
            dir.st_uid != geteuid() || (dir.st_mode & 0777) != 0700 || lstat(path, &endpoint) == 0) return startupFailed();
        audit_token_t token;
        mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
        if (task_info(mach_task_self(), TASK_AUDIT_TOKEN, (task_info_t)&token, &count) != KERN_SUCCESS) return startupFailed();
        ownPidversion = audit_token_to_pidversion(token);
        stateLock = [NSLock new];
        int listener = socket(AF_UNIX, SOCK_STREAM, 0);
        if (listener < 0) return startupFailed();
        int nosigpipe = 1;
        setsockopt(listener, SOL_SOCKET, SO_NOSIGPIPE, &nosigpipe, sizeof(nosigpipe));
        struct sockaddr_un address = { .sun_family = AF_UNIX };
        strlcpy(address.sun_path, path, sizeof(address.sun_path));
        if (bind(listener, (struct sockaddr *)&address, sizeof(address)) != 0) {
            close(listener); return startupFailed();
        }
        if (lstat(path, &endpoint) || !S_ISSOCK(endpoint.st_mode) || endpoint.st_uid != geteuid() ||
            chmod(path, 0600) != 0 || listen(listener, 16) != 0) {
            close(listener); unlink(path); return startupFailed();
        }
        struct server_context *context = calloc(1, sizeof(*context));
        if (!context) { close(listener); unlink(path); return startupFailed(); }
        context->listener = listener;
        strlcpy(context->path, path, sizeof(context->path));
        pthread_t thread;
        if (pthread_create(&thread, NULL, acceptClients, context) != 0) {
            free(context); close(listener); unlink(path); return startupFailed();
        }
        pthread_detach(thread);
        return 0;
    }
}
