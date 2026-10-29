#import "ReconInspect.h"
#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <CoreGraphics/CoreGraphics.h>
#import <objc/runtime.h>
#import <mach/mach_time.h>
#import <sys/utsname.h>

static uint64_t nowNs(void) {
    static mach_timebase_info_data_t scale;
    if (!scale.denom) mach_timebase_info(&scale);
    return mach_continuous_time() * scale.numer / scale.denom;
}
@implementation ReconWork
- (instancetype)init {
    if ((self = [super init])) {
        _narrowed = [NSMutableArray new]; _truncated = [NSMutableArray new];
        _limit = 200; _depth = 3;
    }
    return self;
}
- (BOOL)expired { return nowNs() >= _deadline; }
- (BOOL)consume:(NSString *)reason {
    if ([self expired]) return NO;
    if (_visited >= 25000) {
        if (![_truncated containsObject:@"work:25000"]) [_truncated addObject:@"work:25000"];
        return NO;
    }
    _visited++;
    return YES;
}
- (NSString *)reference:(id)object {
    if (!object) return nil;
    NSString *handle = [_registry handleForObject:object];
    if (!handle && ![_truncated containsObject:@"registry:100000"]) [_truncated addObject:@"registry:100000"];
    return handle;
}
- (NSDictionary *)record:(id)object fields:(NSSet<NSString *> *)fields {
    NSString *kind = ReconKind(object);
    NSString *handle = [self reference:object];
    if (!kind || !handle) return nil;
    NSMutableDictionary *result = [@{ @"handle": handle, @"kind": kind } mutableCopy];
    @try {
        if ([object isKindOfClass:NSView.class]) {
            NSView *view = object;
            result[@"relationships"] = @{ @"superview": [self reference:view.superview] ?: ReconStatus(@"absent"),
                @"window": [self reference:view.window] ?: ReconStatus(@"absent") };
        } else if ([object isKindOfClass:CALayer.class]) {
            result[@"relationships"] = @{ @"superlayer": [self reference:((CALayer *)object).superlayer] ?: ReconStatus(@"absent") };
        } else if ([object isKindOfClass:NSWindow.class]) {
            NSWindow *window = object;
            result[@"relationships"] = @{ @"parentWindow": [self reference:window.parentWindow] ?: ReconStatus(@"absent"),
                @"contentView": [self reference:window.contentView] ?: ReconStatus(@"absent") };
        } else if ([object isKindOfClass:NSLayoutGuide.class]) {
            result[@"relationships"] = @{ @"owningView": [self reference:((NSLayoutGuide *)object).owningView] ?: ReconStatus(@"absent") };
        } else result[@"relationships"] = @{};
    } @catch (NSException *exception) {
        result[@"relationships"] = ReconStatus(@"failed");
    }
    NSDictionary *(^facet)(NSString *) = ^NSDictionary *(NSString *name) {
        @try {
            if ([name isEqual:@"basic"]) return ReconBasic(object, self);
            if ([name isEqual:@"appearance"]) return ReconAppearance(object, self);
            if ([name isEqual:@"font"]) return ReconFont(object, self);
            if ([name isEqual:@"layout"]) return ReconLayout(object, self);
            if ([name isEqual:@"layer"]) return ReconLayer(object, self);
            if ([name isEqual:@"cell"]) return ReconCell(object, self);
            return ReconStatus(@"unsupported");
        } @catch (NSException *exception) {
            return @{ @"status": @"failed", @"reason": @"target-getter-exception" };
        }
    };
    for (NSString *field in fields) {
        if ([self expired]) return nil;
        result[field] = facet(field);
    }
    return result;
}
- (NSDictionary *)complete:(NSDictionary *)payload {
    if ([_truncated containsObject:@"registry:100000"])
        return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
    NSMutableDictionary *result = [payload mutableCopy];
    result[@"ok"] = (NSNumber *)kCFBooleanTrue;
    result[@"coverage"] = @{ @"visited": @(_visited), @"returned": @(_emitted),
        @"narrowed": _narrowed, @"truncated": _truncated,
        @"complete": @(_truncated.count == 0) };
    return result;
}
@end

static NSArray<NSWindow *> *nativeWindows(ReconWork *work) {
    return NSApp ? [NSApp.windows copy] : @[];
}
static NSWindow *mappedWindow(NSNumber *windowId, NSArray<NSWindow *> *windows, NSString **failure) {
    if (!windowId || ![windowId isKindOfClass:NSNumber.class] || [windowId unsignedLongLongValue] > UINT32_MAX) {
        *failure = @"invalid-window-id";
        return nil;
    }
    CGWindowID number = (CGWindowID)[windowId unsignedIntValue];
    CFArrayRef rawDescription = CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID);
    NSArray *description = CFBridgingRelease(rawDescription);
    BOOL owned = NO;
    for (NSDictionary *info in description) {
        if ([info[(id)kCGWindowNumber] unsignedIntValue] == number &&
            [info[(id)kCGWindowOwnerPID] intValue] == getpid() &&
            [info[(id)kCGWindowIsOnscreen] boolValue]) owned = YES;
    }
    if (!owned) {
        *failure = @"window-server-owner";
        return nil;
    }
    NSWindow *resolved = [NSApp windowWithWindowNumber:number];
    if (!resolved) {
        *failure = @"appkit-window-number";
        return nil;
    }
    for (NSWindow *window in windows) if (window == resolved) return window;
    *failure = @"app-window-membership";
    return nil;
}
static id windowForObject(id object) {
    if ([object isKindOfClass:NSWindow.class]) return object;
    if ([object isKindOfClass:NSView.class]) return ((NSView *)object).window;
    if ([object isKindOfClass:NSLayoutGuide.class]) return ((NSLayoutGuide *)object).owningView.window;
    if ([object isKindOfClass:NSCell.class]) return nil;
    if ([object isKindOfClass:CALayer.class]) {
        CALayer *layer = object;
        for (CALayer *parent = layer; parent; parent = parent.superlayer) {
            id view = parent.delegate;
            if ([view isKindOfClass:NSView.class] && ((NSView *)view).layer == parent) return ((NSView *)view).window;
        }
    }
    return nil;
}
static NSArray *children(id object, BOOL layerKind) {
    if (layerKind) return [object isKindOfClass:CALayer.class] ? (((CALayer *)object).sublayers ?: @[]) : @[];
    return [object isKindOfClass:NSView.class] ? ((NSView *)object).subviews : @[];
}
static id traversalRoot(id root, BOOL layerKind) {
    if ([root isKindOfClass:NSWindow.class]) root = ((NSWindow *)root).contentView;
    if (layerKind && [root isKindOfClass:NSView.class]) root = ((NSView *)root).layer;
    return root;
}
static NSDictionary *observation(ReconWork *work, NSArray *fields, NSString *kind) {
    NSDictionary *request = work.request;
    return @{ @"observationId": ReconToken() ?: @"", @"source": @"native-appkit",
        @"sessionId": work.sessionId, @"helperBuild": ReconHelperBuild,
        @"protocol": @(ReconProtocolVersion), @"identity": ReconProcessIdentity(),
        @"scope": @{ @"root": request[@"root"] ?: NSNull.null,
            @"windowId": request[@"windowId"] ?: NSNull.null,
            @"depth": @(work.depth), @"limit": @(work.limit) },
        @"fields": fields, @"kind": kind, @"consistency": @"main-thread-best-effort",
        @"layerState": @"model", @"startTime": @(work.startedAt ?: NSDate.date.timeIntervalSince1970),
        @"endTime": @(NSDate.date.timeIntervalSince1970) };
}
static BOOL matches(id object, NSDictionary *request) {
    NSString *klass = request[@"class"], *identifier = request[@"identifier"];
    if (klass && ![NSStringFromClass(object_getClass(object)) isEqualToString:klass]) return NO;
    if (identifier) {
        NSString *actual = nil;
        if ([object isKindOfClass:NSView.class]) actual = ((NSView *)object).identifier;
        if ([object isKindOfClass:CALayer.class]) actual = ((CALayer *)object).name;
        if (![actual isEqualToString:identifier]) return NO;
    }
    return YES;
}

NSDictionary *ReconInspect(ReconWork *work) {
    NSDictionary *request = work.request;
    NSString *op = request[@"op"], *kind = request[@"kind"] ?: @"view";
    BOOL layerKind = [kind isEqual:@"layer"];
    NSSet *fields = [NSSet setWithArray:request[@"fields"] ?: @[ @"basic" ]];
    NSArray *windows = request[@"windowId"] || !request[@"root"] || [op isEqual:@"windows"] ? nativeWindows(work) : @[];
    NSNumber *windowId = request[@"windowId"];
    NSString *mappingFailure = nil;
    NSWindow *window = windowId ? mappedWindow(windowId, windows, &mappingFailure) : nil;
    if (windowId && !window) return ReconError(@"INSPECTION_UNAVAILABLE", mappingFailure ?: @"window-mapping", [NSString stringWithFormat:@"No live owned native window matches the WindowServer ID (%@)", mappingFailure ?: @"unknown"]);
    id root = nil;
    if (request[@"root"]) {
        if (!ReconValidHandle(request[@"root"])) return ReconError(@"INVALID_ARGUMENT", @"root", @"Malformed native handle");
        root = [work.registry objectForHandle:request[@"root"]];
        if (!root) return ReconError(@"STALE_NODE", @"root", @"Native handle is no longer live");
        if (window && windowForObject(root) != window) return ReconError(@"STALE_NODE", @"scope", @"Native object is no longer in the selected window");
    }
    if ([op isEqual:@"node"]) {
        if (!root) return ReconError(@"INVALID_ARGUMENT", @"node", @"A native handle is required");
        NSDictionary *record = [work record:root fields:fields];
        if (!record) return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
        work.emitted = 1;
        return [work complete:@{ @"node": record, @"observation": observation(work, fields.allObjects, ReconKind(root)) }];
    }
    if ([op isEqual:@"windows"]) {
        NSMutableArray *records = [NSMutableArray new];
        NSArray *inventory = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID));
        NSMutableArray *bridges = [NSMutableArray new];
        for (NSDictionary *info in inventory) {
            if (![work consume:@"windowserver-inventory"]) break;
            if ([info[(id)kCGWindowOwnerPID] intValue] != getpid() ||
                ![info[(id)kCGWindowIsOnscreen] boolValue]) continue;
            NSNumber *candidate = info[(id)kCGWindowNumber];
            if (![candidate isKindOfClass:NSNumber.class]) continue;
            NSWindow *resolved = [NSApp windowWithWindowNumber:candidate.unsignedIntValue];
            for (NSWindow *member in windows) {
                if (member == resolved) { [bridges addObject:@{ @"native": member, @"windowId": candidate }]; break; }
            }
        }
        for (NSWindow *item in windows) {
            if (![work consume:@"windows"]) break;
            NSMutableDictionary *record = [[work record:item fields:fields] mutableCopy];
            if (!record) return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
            record[@"windowId"] = ReconStatus(@"unavailable");
            for (NSDictionary *bridge in bridges) {
                if (bridge[@"native"] != item) continue;
                if ([record[@"windowId"] isKindOfClass:NSNumber.class]) {
                    record[@"windowId"] = ReconStatus(@"unavailable");
                    record[@"mapping"] = @{ @"status": @"ambiguous" };
                    break;
                }
                record[@"windowId"] = bridge[@"windowId"];
                record[@"mapping"] = @{ @"method": @"NSApplication.windowWithWindowNumber",
                    @"evidence": @"WindowServer-owner-pid-and-NSApp-windows-membership" };
            }
            [records addObject:record]; work.emitted++;
            if (records.count >= work.limit) { [work.truncated addObject:@"windows:limit"]; break; }
        }
        return [work complete:@{ @"windows": records, @"observation": observation(work, fields.allObjects, @"window") }];
    }
    if ([op isEqual:@"find"] && !request[@"class"] && !request[@"identifier"])
        return ReconError(@"INVALID_ARGUMENT", @"filters", @"Find requires an exact class or identifier");
    if (layerKind && !root && !window) return ReconError(@"INVALID_ARGUMENT", @"scope", @"Layer traversal requires a native root or WindowServer window");
    if (root && !(layerKind ? ([root isKindOfClass:NSWindow.class] || [root isKindOfClass:NSView.class] || [root isKindOfClass:CALayer.class]) :
                  ([root isKindOfClass:NSWindow.class] || [root isKindOfClass:NSView.class])))
        return ReconError(@"INVALID_ARGUMENT", @"root", @"Root kind is incompatible with traversal");
    NSMutableArray *roots = [NSMutableArray new];
    NSMutableArray *rootWindows = [NSMutableArray new];
    if (root || window) {
        NSWindow *scopeWindow = window ?: ([root isKindOfClass:NSWindow.class] ? root : nil);
        if (scopeWindow) {
            NSDictionary *metadata = [work record:scopeWindow fields:fields];
            if (!metadata) return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
            [rootWindows addObject:metadata];
        }
        id first = traversalRoot(root ?: window, layerKind);
        if (first) [roots addObject:first];
        else [work.narrowed addObject:@"selected-scope:absent-content-or-layer"];
    } else {
        for (NSWindow *candidate in windows) {
            if (![work consume:@"windows"]) break;
            NSDictionary *metadata = [work record:candidate fields:fields];
            if (!metadata) return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
            [rootWindows addObject:metadata];
            if (candidate.contentView) [roots addObject:candidate.contentView];
            if (![op isEqual:@"find"] && rootWindows.count >= work.limit) {
                if (rootWindows.count < windows.count) [work.truncated addObject:@"windows:limit"];
                break;
            }
        }
    }
    NSMutableArray *queue = [NSMutableArray new];
    for (id first in roots) [queue addObject:@{ @"object": first, @"depth": @0 }];
    NSMutableArray *records = [NSMutableArray new];
    NSMutableDictionary *fonts = [NSMutableDictionary new];
    NSHashTable *seen = [NSHashTable hashTableWithOptions:NSHashTableObjectPointerPersonality];
    NSUInteger index = 0;
    while (index < queue.count && ![work expired]) {
        NSDictionary *entry = queue[index++]; id object = entry[@"object"];
        NSUInteger depth = [entry[@"depth"] unsignedIntegerValue];
        if (![work consume:@"traversal"]) break;
        BOOL repeated = [seen containsObject:object];
        if (repeated) {
            if (![op isEqual:@"fonts"] && (![op isEqual:@"find"] || matches(object, request))) {
                NSString *reference = [work reference:object];
                if (!reference) return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
                [records addObject:@{ @"handle": reference, @"kind": ReconKind(object),
                    @"depth": @(depth), @"reference": (NSNumber *)kCFBooleanTrue }];
                work.emitted++;
                if (work.emitted >= work.limit) {
                    if (index < queue.count) [work.truncated addObject:@"returned:limit"];
                    break;
                }
            }
            continue;
        }
        [seen addObject:object];
        BOOL include = ![op isEqual:@"find"] || matches(object, request);
        if (include && ![op isEqual:@"fonts"]) {
            NSDictionary *record = [work record:object fields:fields];
            if (!record) return ReconError(@"INSPECTION_UNAVAILABLE", @"registry", @"Native handle capacity exhausted");
            NSMutableDictionary *row = [record mutableCopy]; row[@"depth"] = @(depth);
            [records addObject:row]; work.emitted++;
        }
        if ([op isEqual:@"fonts"]) {
            NSDictionary *facet = ReconFont(object, work);
            NSMutableArray *observed = [NSMutableArray new];
            if ([facet[@"carrier"][@"status"] isEqual:@"value"]) [observed addObject:@{ @"descriptor": facet[@"carrier"], @"source": facet[@"source"] }];
            for (NSDictionary *run in facet[@"runs"] ?: @[]) [observed addObject:@{ @"descriptor": run[@"font"], @"source": @"attributed-run", @"range": run[@"range"] }];
            for (NSDictionary *item in observed) {
                NSDictionary *desc = item[@"descriptor"];
                NSString *key = [NSString stringWithFormat:@"%@|%@|%@|%@", desc[@"postScriptName"], desc[@"pointSize"], desc[@"symbolicTraits"], desc[@"weight"]];
                NSMutableDictionary *group = fonts[key];
                if (!group) { group = [@{ @"descriptor": desc, @"sources": [NSMutableArray new] } mutableCopy]; fonts[key] = group; }
                NSMutableDictionary *source = [@{ @"handle": [work reference:object] ?: @"", @"kind": ReconKind(object), @"source": item[@"source"] } mutableCopy];
                if (item[@"range"]) source[@"range"] = item[@"range"];
                [group[@"sources"] addObject:source];
                work.emitted++;
                if (work.emitted >= work.limit) break;
            }
        }
        if (work.emitted >= work.limit) { if (index < queue.count || depth < work.depth) [work.truncated addObject:@"returned:limit"]; break; }
        if (depth < work.depth) {
            for (id child in children(object, layerKind)) {
                if ([op isEqual:@"tree"] && queue.count - index >= work.limit - work.emitted) {
                    [work.truncated addObject:@"returned:limit"];
                    [work.truncated addObject:@"unsearched-scope"];
                    break;
                }
                if (![work consume:@"relationships"]) break;
                [queue addObject:@{ @"object": child, @"depth": @(depth + 1) }];
            }
        } else if (children(object, layerKind).count && ![work.narrowed containsObject:@"depth"]) [work.narrowed addObject:@"depth"];
    }
    if (index < queue.count) [work.truncated addObject:@"unsearched-scope"];
    if ([work expired]) return ReconError(@"INSPECTION_TIMEOUT", @"target-execution", @"Native read exceeded its deadline");
    if ([op isEqual:@"fonts"]) return [work complete:@{ @"fonts": fonts.allValues, @"windowRoots": rootWindows,
        @"observation": observation(work, @[ @"font" ], kind) }];
    return [work complete:@{ @"nodes": records, @"windowRoots": rootWindows,
        @"observation": observation(work, fields.allObjects, kind) }];
}
