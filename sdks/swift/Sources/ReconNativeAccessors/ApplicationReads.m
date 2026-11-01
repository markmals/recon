#import "ReconAccessorsInternal.h"

#import <CoreGraphics/CoreGraphics.h>

ReconReadStatus ReconReadApplicationWindows(NSArray **windows) {
    *windows = nil;
    @try {
        if (!NSApp) return ReconReadStatusUnavailable;
        *windows = [NSApp.windows copy] ?: @[];
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

id _Nullable ReconApplicationWindowWithNumber(NSInteger number) {
    @try {
        return [NSApp windowWithWindowNumber:number];
    } @catch (id exception) {
        return nil;
    }
}

static ReconWindowServerEntry *_Nullable inventoryEntry(NSDictionary *description) {
    NSNumber *number = description[(id)kCGWindowNumber];
    if (![number isKindOfClass:NSNumber.class]) return nil;
    ReconWindowServerEntry *entry = [ReconWindowServerEntry new];
    entry.number = number.unsignedIntValue;
    entry.ownerPid = [description[(id)kCGWindowOwnerPID] intValue];
    entry.onscreen = [description[(id)kCGWindowIsOnscreen] boolValue];
    return entry;
}

NSArray<ReconWindowServerEntry *> *ReconWindowServerInventory(void) {
    @try {
        NSArray *inventory = CFBridgingRelease(
            CGWindowListCopyWindowInfo(kCGWindowListOptionAll, kCGNullWindowID));
        NSMutableArray<ReconWindowServerEntry *> *entries = [NSMutableArray new];
        for (NSDictionary *description in inventory) {
            ReconWindowServerEntry *entry = inventoryEntry(description);
            if (entry) [entries addObject:entry];
        }
        return entries;
    } @catch (id exception) {
        return @[];
    }
}
