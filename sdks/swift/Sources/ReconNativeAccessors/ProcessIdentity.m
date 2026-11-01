#import "ReconAccessorsInternal.h"

#import <libproc.h>
#import <mach/mach.h>
#import <mach/machine.h>
#import <sys/sysctl.h>
#import <unistd.h>

/// The audit token's eighth word is the process's generation count, which separates this process
/// from a later one that reuses its pid.
static uint32_t processPidversion(void) {
    audit_token_t token;
    mach_msg_type_number_t count = TASK_AUDIT_TOKEN_COUNT;
    kern_return_t read = task_info(mach_task_self(), TASK_AUDIT_TOKEN, (task_info_t)&token, &count);
    return read == KERN_SUCCESS ? token.val[7] : 0;
}

static NSString *processArchitecture(void) {
    struct proc_archinfo architectureInfo = {0};
    int read = proc_pidinfo(getpid(), PROC_PIDARCHINFO, 0, &architectureInfo, sizeof(architectureInfo));
    if (read != sizeof(architectureInfo) || architectureInfo.p_cputype != CPU_TYPE_ARM64) {
        return @"unknown";
    }
    int subtype = architectureInfo.p_cpusubtype & ~CPU_SUBTYPE_MASK;
    return subtype == CPU_SUBTYPE_ARM64E || subtype == 12 ? @"arm64e" : @"arm64";
}

static NSString *operatingSystemBuild(void) {
    char build[128] = {0};
    size_t size = sizeof(build);
    if (sysctlbyname("kern.osversion", build, &size, NULL, 0) != 0) build[0] = 0;
    return @(build);
}

ReconProcessIdentityFacts *ReconReadProcessIdentity(void) {
    NSBundle *bundle = NSBundle.mainBundle;
    NSString *path = bundle.bundlePath;
    ReconProcessIdentityFacts *facts = [ReconProcessIdentityFacts new];
    facts.pid = getpid();
    facts.pidversion = processPidversion();
    facts.uid = geteuid();
    facts.bundleId = bundle.bundleIdentifier;
    facts.bundlePath = [path.pathExtension isEqualToString:@"app"] ? path : nil;
    facts.executablePath = bundle.executablePath;
    facts.architecture = processArchitecture();
    facts.osBuild = operatingSystemBuild();
    facts.osVersion = NSProcessInfo.processInfo.operatingSystemVersionString;
    return facts;
}

NSDictionary *ReconProcessIdentityDictionary(void) {
    ReconProcessIdentityFacts *facts = ReconReadProcessIdentity();
    return @{ @"pid": @(facts.pid), @"pidversion": @(facts.pidversion), @"uid": @(facts.uid),
              @"bundleId": facts.bundleId ?: NSNull.null,
              @"bundlePath": facts.bundlePath ?: NSNull.null,
              @"executablePath": facts.executablePath ?: NSNull.null,
              @"architecture": facts.architecture, @"osBuild": facts.osBuild,
              @"osVersion": facts.osVersion };
}
