// Proof fixture for proposal.0006: a cooperative AppKit process built for arm64 and arm64e.
// It never orders a window in or activates; NSApp.windows still lists its window.
#import <AppKit/AppKit.h>
#import <signal.h>

@interface RaisingView : NSView
@end

@implementation RaisingView
- (NSString *)toolTip {
    [NSException raise:NSInternalInconsistencyException format:@"proof getter raised"];
    return nil;
}
@end

static dispatch_source_t blockOnSignal(int signalNumber, useconds_t microseconds) {
    signal(signalNumber, SIG_IGN);
    dispatch_source_t source =
        dispatch_source_create(DISPATCH_SOURCE_TYPE_SIGNAL, signalNumber, 0, dispatch_get_main_queue());
    dispatch_source_set_event_handler(source, ^{ usleep(microseconds); });
    dispatch_resume(source);
    return source;
}

int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 320, 200)
                                                       styleMask:NSWindowStyleMaskTitled
                                                         backing:NSBackingStoreBuffered
                                                           defer:YES];
        window.title = @"Proof — Main";
        NSView *plain = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 100, 100)];
        plain.identifier = @"plain";
        plain.toolTip = @"plain tip";
        RaisingView *raising = [[RaisingView alloc] initWithFrame:NSMakeRect(100, 0, 100, 100)];
        raising.identifier = @"raising";
        [window.contentView addSubview:plain];
        [window.contentView addSubview:raising];
        // SIGUSR1 holds the main thread past a read's deadline; SIGUSR2 holds it briefly.
        dispatch_source_t past = blockOnSignal(SIGUSR1, 3000000);
        dispatch_source_t brief = blockOnSignal(SIGUSR2, 1000000);
        (void)past;
        (void)brief;
        [NSApp run];
    }
    return 0;
}
